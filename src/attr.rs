use log::error;

use crate::{
    clone_db,
    common::{
        DbfsAttr, DbfsError, DbfsPermission, DbfsResult, DbfsTimeSpec, XattrNamespace, ACCESS_R_OK,
        ACCESS_W_OK,
    },
    inode::{checkout_access, dbfs_common_attr},
    u16, u32,
};

pub fn dbfs_common_setxattr(
    r_uid: u32,
    r_gid: u32,
    ino: usize,
    key: &str,
    value: &[u8],
    ctime: DbfsTimeSpec,
) -> DbfsResult<()> {
    let db = clone_db();
    let tx = db.tx(true).unwrap();
    let bucket = tx.get_bucket(ino.to_be_bytes())?;
    // checkout access
    let uid = bucket.get_kv("uid").unwrap();
    let uid = u32!(uid.value());
    let gid = bucket.get_kv("gid").unwrap();
    let gid = u32!(gid.value());
    let mode = bucket.get_kv("mode").unwrap();
    let mode = u16!(mode.value()) & 0o777;
    xattr_access_check(key, ACCESS_R_OK, r_uid, r_gid, uid, gid, mode)?;
    bucket.put(key, value)?;
    // update ctime
    bucket.put("ctime", ctime.to_be_bytes())?;
    tx.commit()?;
    Ok(())
}

pub fn dbfs_common_getxattr(
    r_uid: u32,
    r_gid: u32,
    ino: usize,
    key: &str,
    buf: &mut [u8],
) -> DbfsResult<usize> {
    let db = clone_db();
    let tx = db.tx(false).unwrap();
    let bucket = tx.get_bucket(ino.to_be_bytes())?;
    // checkout access
    let uid = bucket.get_kv("uid").unwrap();
    let uid = u32!(uid.value());
    let gid = bucket.get_kv("gid").unwrap();
    let gid = u32!(gid.value());
    let mode = bucket.get_kv("mode").unwrap();
    let mode = u16!(mode.value()) & 0o777;
    xattr_access_check(key, ACCESS_R_OK, r_uid, r_gid, uid, gid, mode)?;
    let value = bucket.get_kv(key);
    if value.is_none() {
        return Err(DbfsError::NoData);
    }
    let value = value.unwrap();
    if buf.len() == 0 {
        return Ok(value.value().len());
    }
    let val_len = value.value().len();
    if buf.len() < val_len {
        return Err(DbfsError::RangeError);
    }
    buf[..val_len].copy_from_slice(value.value());

    Ok(val_len)
}

pub fn dbfs_common_listxattr(
    _r_uid: u32,
    _r_gid: u32,
    ino: usize,
    buf: &mut [u8],
) -> DbfsResult<usize> {
    let db = clone_db();
    let tx = db.tx(false).unwrap();
    let bucket = tx.get_bucket(ino.to_be_bytes())?;
    // TODO! checkout access
    let mut size = 0;
    // find all xattr
    let buf = &mut buf[..];
    bucket.kv_pairs().for_each(|x| {
        if x.key().starts_with(b"user.")
            || x.key().starts_with(b"system.")
            || x.key().starts_with(b"trusted.")
            || x.key().starts_with(b"security.")
        {
            let tmp = size;
            size += x.key().len() + 1;
            if buf.len() >= size {
                buf[tmp..size - 1].copy_from_slice(x.key());
                buf[size - 1] = 0;
            } else {
                size = tmp;
                return;
            }
        }
    });
    Ok(size)
}

pub fn dbfs_common_removexattr(
    r_uid: u32,
    r_gid: u32,
    ino: usize,
    key: &str,
    ctime: DbfsTimeSpec,
) -> DbfsResult<()> {
    let db = clone_db();
    let tx = db.tx(true).unwrap();
    let bucket = tx.get_bucket(ino.to_be_bytes())?;
    // checkout access
    let uid = bucket.get_kv("uid").unwrap();
    let uid = u32!(uid.value());
    let gid = bucket.get_kv("gid").unwrap();
    let gid = u32!(gid.value());
    let mode = bucket.get_kv("mode").unwrap();
    let mode = u16!(mode.value()) & 0o777;
    xattr_access_check(key, ACCESS_W_OK, r_uid, r_gid, uid, gid, mode)?;
    bucket.delete(key)?;
    //update ctime
    bucket.put("ctime", ctime.to_be_bytes())?;
    Ok(())
}

pub fn dbfs_common_chmod(
    r_uid: u32,
    r_gid: u32,
    ino: usize,
    mode: u16,
    ctime: DbfsTimeSpec,
) -> DbfsResult<DbfsAttr> {
    let mut attr = dbfs_common_attr(ino).map_err(|_| DbfsError::Other)?;
    // checkout access
    let uid = attr.uid;
    let gid = attr.gid;

    let mut i_mode = attr.perm;

    if r_uid != 0 && r_uid != uid {
        return Err(DbfsError::PermissionDenied);
    }
    if r_uid != 0 && r_gid != gid {
        return Err(DbfsError::PermissionDenied);
    }
    //update mode, the i_mode include file type but mode not include file type
    i_mode = (i_mode & 0o170000) | (mode & 0o777);

    if i_mode != attr.perm {
        let db = clone_db();
        let tx = db.tx(true).unwrap();
        let bucket = tx.get_bucket(ino.to_be_bytes())?;
        bucket.put("mode", i_mode.to_be_bytes())?;
        //update ctime
        bucket.put("ctime", ctime.to_be_bytes())?;
        tx.commit()?;
        attr.perm = i_mode;
        attr.ctime = DbfsTimeSpec::from(ctime);
    }
    Ok(attr)
}

pub fn dbfs_common_chown(
    r_uid: u32,
    r_gid: u32,
    ino: usize,
    uid: Option<u32>,
    gid: Option<u32>,
    c_time: DbfsTimeSpec,
) -> DbfsResult<DbfsAttr> {
    let mut attr = dbfs_common_attr(ino).map_err(|_| DbfsError::Other)?;
    if let Some(gid) = gid {
        // Non-root users can only change gid to a group they're in
        if r_uid != 0 && r_gid != gid {
            return Err(DbfsError::PermissionDenied);
        }
    }
    if let Some(uid) = uid {
        // but no-op changes by the owner are not an error
        if r_uid != 0 && !(uid == attr.uid && r_uid == attr.uid) {
            return Err(DbfsError::PermissionDenied);
        }
    }
    // Only owner may change the group
    if gid.is_some() && r_uid != 0 && r_uid != attr.uid {
        return Err(DbfsError::PermissionDenied);
    }
    let mut perm = DbfsPermission::from_bits_truncate(attr.perm);
    if perm.contains(DbfsPermission::S_IXUSR)
        || perm.contains(DbfsPermission::S_IXGRP)
        || perm.contains(DbfsPermission::S_IXOTH)
    {
        perm = clear_suid_sgid(perm);
    }
    if let Some(uid) = uid {
        attr.uid = uid;
        perm -= DbfsPermission::S_ISUID;
    }
    if let Some(gid) = gid {
        attr.gid = gid;
        perm -= DbfsPermission::S_ISGID;
    }
    attr.perm = perm.bits();
    // we need update the uid and gid and ctime
    let db = clone_db();
    let tx = db.tx(true).unwrap();
    let bucket = tx.get_bucket(ino.to_be_bytes())?;
    bucket.put("uid", attr.uid.to_be_bytes())?;
    bucket.put("gid", attr.gid.to_be_bytes())?;
    bucket.put("mode", attr.perm.to_be_bytes())?;
    bucket.put("ctime", c_time.to_be_bytes())?;
    tx.commit()?;
    attr.ctime = DbfsTimeSpec::from(c_time);
    Ok(attr)
}

pub fn dbfs_common_utimens(
    r_uid: u32,
    r_gid: u32,
    ino: usize,
    atime: Option<DbfsTimeSpec>,
    mtime: Option<DbfsTimeSpec>,
    c_time: DbfsTimeSpec,
) -> DbfsResult<DbfsAttr> {
    let mut attr = dbfs_common_attr(ino)?;
    // checkout access
    let _uid = attr.uid;
    let _gid = attr.gid;
    let mode = attr.perm;

    if attr.uid != r_uid && attr.uid != 0 {
        return Err(DbfsError::PermissionDenied);
    }
    if attr.uid != r_uid
        && !checkout_access(attr.uid, attr.gid, mode & 0o777, r_uid, r_gid, ACCESS_W_OK)
    {
        return Err(DbfsError::AccessError);
    }
    // update atime / mtime / ctime
    let db = clone_db();
    let tx = db.tx(true).unwrap();
    let bucket = tx.get_bucket(ino.to_be_bytes())?;
    if let Some(atime) = atime {
        bucket.put("atime", atime.to_be_bytes())?;
        attr.atime = DbfsTimeSpec::from(atime);
    }
    if let Some(mtime) = mtime {
        bucket.put("mtime", mtime.to_be_bytes())?;
        attr.mtime = DbfsTimeSpec::from(mtime);
    }
    bucket.put("ctime", c_time.to_be_bytes())?;
    tx.commit()?;

    attr.ctime = DbfsTimeSpec::from(c_time);

    error!(
        "utimens attr: {:?} {:?} {:?}",
        attr.atime, attr.mtime, attr.ctime
    );

    Ok(attr)
}

pub fn clear_suid_sgid(mut perm: DbfsPermission) -> DbfsPermission {
    perm -= DbfsPermission::S_ISUID;
    if perm.contains(DbfsPermission::S_IXGRP) {
        perm -= DbfsPermission::S_ISGID;
    }
    perm
}

fn parse_xattr_namespace(key: &str) -> DbfsResult<XattrNamespace> {
    let user = "user.";
    let system = "system.";
    let trusted = "trusted.";
    let security = "security.";
    if key.starts_with(user) {
        return Ok(XattrNamespace::User);
    }
    if key.starts_with(system) {
        return Ok(XattrNamespace::System);
    }

    if key.starts_with(trusted) {
        return Ok(XattrNamespace::Trusted);
    }
    if key.starts_with(security) {
        return Ok(XattrNamespace::Security);
    }
    return Err(DbfsError::NotSupported);
}

fn xattr_access_check(
    key: &str,
    access_mask: u16,
    r_uid: u32,
    r_gid: u32,
    uid: u32,
    gid: u32,
    mode: u16,
) -> DbfsResult<()> {
    match parse_xattr_namespace(key)? {
        XattrNamespace::Security => {
            if access_mask != ACCESS_R_OK && r_uid != 0 {
                return Err(DbfsError::PermissionDenied);
            }
        }
        XattrNamespace::Trusted => {
            if r_uid != 0 {
                return Err(DbfsError::PermissionDenied);
            }
        }
        XattrNamespace::System => {
            if key.eq("system.posix_acl_access") {
                let bool = checkout_access(uid, gid, mode, r_uid, r_gid, access_mask);
                if !bool {
                    return Err(DbfsError::PermissionDenied);
                }
            } else if r_uid != 0 {
                return Err(DbfsError::PermissionDenied);
            }
        }
        XattrNamespace::User => {
            if !checkout_access(uid, gid, mode, r_uid, r_gid, access_mask) {
                return Err(DbfsError::PermissionDenied);
            }
        }
    }
    Ok(())
}
