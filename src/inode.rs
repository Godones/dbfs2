use crate::file::{DBFS_DIR_FILE_OPS, DBFS_FILE_FILE_OPS, DBFS_SYMLINK_FILE_OPS};
use crate::{clone_db, u16, u32, u64, usize};
use alloc::borrow::ToOwned;
use alloc::format;
use alloc::string::ToString;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::cmp::min;
use core::sync::atomic::AtomicUsize;
use log::error;
use rvfs::dentry::{DirEntry, LookUpData};
use rvfs::file::{FileMode, FileOps};
use rvfs::inode::{create_tmp_inode_from_sb_blk, Inode, InodeMode, InodeOps};
use rvfs::{ddebug, warn, StrResult};
use crate::attr::clear_suid_sgid;

use crate::common::{ACCESS_W_OK, DbfsAttr, DbfsError, DbfsFileType, DbfsPermission, DbfsResult, DbfsTimeSpec};
use crate::link::{dbfs_common_readlink, dbfs_common_unlink};

pub static DBFS_INODE_NUMBER: AtomicUsize = AtomicUsize::new(1);

pub const DBFS_DIR_INODE_OPS: InodeOps = {
    let mut ops = InodeOps::empty();
    ops.create = dbfs_create;
    ops.mkdir = dbfs_mkdir;
    ops.link = dbfs_link;
    ops.unlink = dbfs_unlink;
    ops.symlink = dbfs_symlink;
    ops.lookup = dbfs_lookup;
    ops.rmdir = dbfs_rmdir;
    ops.set_attr = dbfs_setattr;
    ops.get_attr = dbfs_getattr;
    ops.list_attr = dbfs_listattr;
    ops.remove_attr = dbfs_removeattr;
    ops.rename = dbfs_rename;
    ops
};

pub const DBFS_FILE_INODE_OPS: InodeOps = {
    let mut ops = InodeOps::empty();
    ops.set_attr = dbfs_setattr;
    ops.get_attr = dbfs_getattr;
    ops.list_attr = dbfs_listattr;
    ops.truncate = dbfs_truncate;
    ops
};
pub const DBFS_SYMLINK_INODE_OPS: InodeOps = {
    let mut ops = InodeOps::empty();
    ops.set_attr = dbfs_setattr;
    ops.get_attr = dbfs_getattr;
    ops.list_attr = dbfs_listattr;
    ops.readlink = dbfs_readlink;
    ops.follow_link = dbfs_followlink;
    ops
};

fn dbfs_create(dir: Arc<Inode>, dentry: Arc<DirEntry>, mode: FileMode) -> StrResult<()> {
    dbfs_rvfs_create(dir, dentry, mode, InodeMode::S_FILE, None)
}
fn dbfs_mkdir(dir: Arc<Inode>, dentry: Arc<DirEntry>, mode: FileMode) -> StrResult<()> {
    dbfs_rvfs_create(dir, dentry, mode, InodeMode::S_DIR, None)
}

fn dbfs_link(
    old_dentry: Arc<DirEntry>,
    dir: Arc<Inode>,
    new_dentry: Arc<DirEntry>,
) -> StrResult<()> {
    let old_inode = old_dentry.access_inner().d_inode.clone();
    let ino = old_inode.number;
    let name = new_dentry.access_inner().d_name.clone();
    let new_ino = dir.number;

    let _ = dbfs_common_link(0, 0, ino, new_ino, &name, 0).map_err(|_| "DbfsError::NotFound")?;

    // update old inode data in memory
    // update hard_links
    // set the new dentry's inode to old inode
    old_inode.access_inner().hard_links += 1;
    dir.access_inner().file_size += 1;
    new_dentry.access_inner().d_inode = old_inode;
    Ok(())
}

pub fn dbfs_common_link(
    uid: u32,
    gid: u32,
    ino: usize,
    new_ino: usize,
    name: &str,
    ctime: usize,
) -> DbfsResult<DbfsAttr> {
    // checkout permission
    let attr = dbfs_common_attr(new_ino).map_err(|_| DbfsError::NotFound)?;
    if !checkout_access(
        attr.uid,
        attr.gid,
        attr.perm & 0o777,
        uid,
        gid,
        2, //libc::W_OK,
    ) {
        return Err(DbfsError::AccessError);
    }

    let db = clone_db();
    // update new inode data in db
    let tx = db.tx(true).unwrap();
    let bucket = tx.get_bucket(new_ino.to_be_bytes()).unwrap();
    let next_number = bucket.get_kv("size".to_string()).unwrap();
    let next_number = usize!(next_number.value());

    let key = format!("data{}", next_number);
    let value = format!("{}:{}", name, ino);
    bucket.put(key, value).unwrap();
    bucket.put("size", (next_number + 1).to_be_bytes()).unwrap();

    // update ctime/mtime
    bucket.put("ctime", ctime.to_be_bytes()).unwrap();
    bucket.put("mtime", ctime.to_be_bytes()).unwrap();

    // update old inode data in memory
    // update hard_links
    // set the new dentry's inode to old inode

    let old_bucket = tx.get_bucket(ino.to_be_bytes()).unwrap();
    let hard_links = old_bucket.get_kv("hard_links".to_string()).unwrap();
    let mut value = u32!(hard_links.value());
    value += 1;
    old_bucket.put("hard_links", value.to_be_bytes()).unwrap();
    // update ctime: last change time
    old_bucket.put("ctime", ctime.to_be_bytes()).unwrap();

    tx.commit().unwrap();
    let dbfs_attr = dbfs_common_attr(ino).map_err(|_| DbfsError::NotFound)?;
    Ok(dbfs_attr)
}

fn dbfs_unlink(dir: Arc<Inode>, dentry: Arc<DirEntry>) -> StrResult<()> {
    let inode = dentry.access_inner().d_inode.clone();
    let number = dir.number;
    let name = &dentry.access_inner().d_name;

    warn!("dbfs_unlink: dir.number={}, name={}", number, name);
    dbfs_common_unlink(0, 0, number, name, Some(inode.number), 0)
        .map_err(|_| "dbfs_common_unlink failed")?;
    let mut inner = inode.access_inner();
    inner.hard_links -= 1;
    Ok(())
}

fn dbfs_symlink(dir: Arc<Inode>, dentry: Arc<DirEntry>, target: &str) -> StrResult<()> {
    dbfs_rvfs_create(
        dir,
        dentry,
        FileMode::FMODE_READ,
        InodeMode::S_SYMLINK,
        Some(target),
    )
}

fn dbfs_lookup(dir: Arc<Inode>, dentry: Arc<DirEntry>) -> StrResult<()> {
    let number = dir.number;
    let name = &dentry.access_inner().d_name;
    let res = dbfs_common_lookup(number, name).map_err(|_| "dbfs_common_lookup failed")?;
    let inode_mode = InodeMode::from(res.kind);
    // create a inode according to the data in db
    let n_inode = create_tmp_inode_from_sb_blk(
        dir.super_blk.upgrade().unwrap().clone(),
        number,
        inode_mode,
        0,
        inode_ops_from_inode_mode(inode_mode),
        file_ops_from_inode_mode(inode_mode),
        None,
    )?;
    n_inode.access_inner().file_size = res.size;
    n_inode.access_inner().hard_links = res.nlink;
    n_inode.access_inner().uid = res.uid;
    n_inode.access_inner().gid = res.gid;
    dentry.access_inner().d_inode = n_inode;
    Ok(())
}

pub fn dbfs_common_lookup(dir: usize, name: &str) -> Result<DbfsAttr, ()> {
    let db = clone_db();
    let tx = db.tx(false).unwrap();
    let bucket = tx.get_bucket(dir.to_be_bytes()).unwrap();
    let value = bucket.kv_pairs().find(|kv| {
        kv.key().starts_with("data".as_bytes()) && kv.value().starts_with(name.as_bytes())
    });
    if value.is_none() {
        return Err(());
    }
    let value = value.unwrap();
    let value = value.value();
    let str = core::str::from_utf8(value).unwrap();
    let data = str.rsplitn(2, ':').collect::<Vec<&str>>();
    let number = data[0].parse::<usize>().unwrap();
    dbfs_common_attr(number)
}

pub fn dbfs_common_attr(number: usize) -> Result<DbfsAttr, ()> {
    let db = clone_db();
    let tx = db.tx(false).unwrap();
    let bucket = tx.get_bucket(number.to_be_bytes()).unwrap();
    let size = bucket.get_kv("size").unwrap();
    let size = usize!(size.value());

    let mode = bucket.get_kv("mode").unwrap();
    let mode = u16!(mode.value());
    let mode = DbfsPermission::from_bits(mode).unwrap();
    let file_type = DbfsFileType::from(mode);

    let n_links = bucket.get_kv("hard_links").unwrap();
    let n_links = u32!(n_links.value());

    let uid = bucket.get_kv("uid").unwrap();
    let uid = u32!(uid.value());
    let gid = bucket.get_kv("gid").unwrap();
    let gid = u32!(gid.value());

    let blksize = bucket.get_kv("block_size").unwrap();
    let blksize = u32!(blksize.value());
    let blocks = (size + blksize as usize - 1) / blksize as usize;

    let atime = bucket.get_kv("atime").unwrap();
    let atime = usize!(atime.value());
    let mtime = bucket.get_kv("mtime").unwrap();
    let mtime = usize!(mtime.value());
    let ctime = bucket.get_kv("ctime").unwrap();
    let ctime = usize!(ctime.value());

    // fill dbfs_attr
    let dbfs_attr = DbfsAttr {
        ino: number,
        size,
        blocks,
        atime: DbfsTimeSpec::from(atime),
        mtime: DbfsTimeSpec::from(mtime),
        ctime: DbfsTimeSpec::from(ctime),
        crtime: DbfsTimeSpec::default(),
        kind: file_type,
        perm: mode.bits(),
        nlink: n_links,
        uid,
        gid,
        rdev: 0,
        blksize,
        padding: 0,
        flags: 0,
    };
    Ok(dbfs_attr)
}

fn dbfs_rmdir(dir: Arc<Inode>, dentry: Arc<DirEntry>) -> StrResult<()> {
    let number = dir.number;
    let name = &dentry.access_inner().d_name;
    dbfs_common_rmdir(0,0,number,name,0).map_err(|x|{
        warn!("dbfs_common_rmdir failed: {:?}",x);
        "dbfs_common_rmdir failed"
    })?;
    dir.access_inner().file_size -= 1;
    Ok(())
}

/// create a new attribute for a dentry
/// if the key is already exist, it will be overwrite
/// if the key is not exist, it will be created
fn dbfs_setattr(dentry: Arc<DirEntry>, key: &str, val: &[u8]) -> StrResult<()> {
    let db = clone_db();
    let tx = db.tx(true).unwrap();
    let number = dentry.access_inner().d_inode.number;
    let bucket = tx.get_bucket(number.to_be_bytes()).unwrap();
    let key = format!("attr:{}", key);
    bucket.put(key, val).unwrap();
    tx.commit().unwrap();
    Ok(())
}
fn dbfs_removeattr(dentry: Arc<DirEntry>, key: &str) -> StrResult<()> {
    let db = clone_db();
    let tx = db.tx(true).unwrap();
    let number = dentry.access_inner().d_inode.number;
    let bucket = tx.get_bucket(number.to_be_bytes()).unwrap();
    let key = format!("attr:{}", key);
    let res = bucket.delete(key);
    let res = if res.is_err() {
        Err("not found")
    } else {
        Ok(())
    };
    tx.commit().unwrap();
    res
}
fn dbfs_getattr(dentry: Arc<DirEntry>, key: &str, buf: &mut [u8]) -> StrResult<usize> {
    let db = clone_db();
    let tx = db.tx(false).unwrap();
    let number = dentry.access_inner().d_inode.number;
    let bucket = tx.get_bucket(number.to_be_bytes()).unwrap();
    let key = format!("attr:{}", key);
    let value = bucket.get_kv(key);
    let value = if value.is_none() {
        return Err("not found");
    } else {
        value.unwrap()
    };
    let value = value.value();
    let len = min(value.len(), buf.len());
    buf[..len].copy_from_slice(&value[..len]);
    Ok(value.len())
}

fn dbfs_listattr(dentry: Arc<DirEntry>, buf: &mut [u8]) -> StrResult<usize> {
    let db = clone_db();
    let tx = db.tx(false).unwrap();
    let number = dentry.access_inner().d_inode.number;
    let bucket = tx.get_bucket(number.to_be_bytes()).unwrap();
    let mut len = 0;
    let mut total_attr_buf = 0;
    for kv in bucket.kv_pairs() {
        let key = kv.key();
        if key.starts_with("attr:".as_bytes()) {
            let key = key.splitn(2, |c| *c == b':').collect::<Vec<&[u8]>>();
            let key = key[1];
            let key_len = key.len();
            total_attr_buf += key_len + 1;
            if len + key_len >= buf.len() {
                continue;
            }
            buf[len..len + key_len].copy_from_slice(key);
            buf[len + key_len] = 0;
            len += key_len + 1;
        }
    }
    Ok(total_attr_buf)
}

fn dbfs_readlink(dentry: Arc<DirEntry>, buf: &mut [u8]) -> StrResult<usize> {
    let number = dentry.access_inner().d_inode.number;
    dbfs_common_readlink(number, buf).map_err(|_| "not a symlink")
}

fn dbfs_followlink(dentry: Arc<DirEntry>, lookup_data: &mut LookUpData) -> StrResult<()> {
    let db = clone_db();
    let tx = db.tx(false).unwrap();
    let number = dentry.access_inner().d_inode.number;
    let bucket = tx.get_bucket(number.to_be_bytes()).unwrap();
    let value = bucket.get_kv("data").unwrap();
    let value = value.value();
    let str = core::str::from_utf8(value).unwrap();
    lookup_data.symlink_names.push(str.to_string());
    Ok(())
}

fn dbfs_rename(
    old_dir: Arc<Inode>,
    old_dentry: Arc<DirEntry>,
    new_dir: Arc<Inode>,
    new_dentry: Arc<DirEntry>,
) -> StrResult<()> {
    let db = clone_db();
    let tx = db.tx(false).unwrap();
    let old_number = old_dir.number;

    let old_bucket = tx.get_bucket(old_number.to_be_bytes()).unwrap();
    let old_name = old_dentry.access_inner().d_name.clone();
    let kv = old_bucket.kv_pairs().find(|kv| {
        kv.key().starts_with("data".as_bytes()) && kv.value().starts_with(old_name.as_bytes())
    });
    let new_number = new_dir.number;
    if let Some(kv) = kv {
        let key = kv.key();
        let value = kv.value();
        let str = core::str::from_utf8(value).unwrap();
        let data = str.rsplitn(2, ':').collect::<Vec<&str>>();
        let _number = data[0].parse::<usize>().unwrap();

        let new_name = new_dentry.access_inner().d_name.clone();
        let new_value = format!("{}:{}", new_name, new_number);
        let tx = db.tx(true).unwrap();
        let old_bucket = tx.get_bucket(old_number.to_be_bytes()).unwrap();
        if new_number == old_number {
            // in the same bucket
            // update old bucket
            old_bucket.put(key, new_value).unwrap();
        } else {
            // in different bucket
            let new_bucket = tx.get_bucket(new_number.to_be_bytes()).unwrap();
            // update old bucket
            old_bucket.delete(key).unwrap();
            // update new bucket
            let next_number = new_bucket.get_kv("size").unwrap();
            let next_number = usize!(next_number.value());
            let new_key = format!("data:{}", next_number);
            new_bucket.put(new_key, new_value).unwrap();
            // update size
            new_bucket
                .put("size", (next_number + 1).to_string())
                .unwrap();

            old_dir.access_inner().file_size -= 1;
            new_dir.access_inner().file_size += 1;
        }
        tx.commit().unwrap();
    } else {
        return Err("dbfs_rename: old_dentry not found");
    }
    Ok(())
}

fn dbfs_truncate(inode: Arc<Inode>) -> StrResult<()> {
    let number = inode.number;
    let inode_inner = inode.access_inner();
    let f_size = inode_inner.file_size;
    let _res = dbfs_common_truncate(0,0,number,0,f_size)
        .map_err(|_| "dbfs_truncate: truncate failed")?;
    Ok(())
}

pub fn permission_from_mode(_mode: FileMode, inode_mode: InodeMode) -> DbfsPermission {
    // we don't use mode now,make all permission to true
    let mut permission = DbfsPermission::from_bits_truncate(0o777);
    match inode_mode {
        InodeMode::S_FILE => permission |= DbfsPermission::S_IFREG,
        InodeMode::S_DIR => permission |= DbfsPermission::S_IFDIR,
        InodeMode::S_SYMLINK => permission |= DbfsPermission::S_IFLNK,
        _ => {}
    }
    permission
}

fn dbfs_rvfs_create(
    dir: Arc<Inode>,
    dentry: Arc<DirEntry>,
    mode: FileMode,
    inode_mode: InodeMode,
    target_path: Option<&str>,
) -> StrResult<()> {
    let dir_number = dir.number;
    let name = dentry.access_inner().d_name.to_owned();
    let permission = permission_from_mode(mode, inode_mode);

    let attr = dbfs_common_create(dir_number, &name, 0, 0, 0, permission, target_path)
        .map_err(|_| "dbfs_rvfs_create: dbfs_common_create failed")?;

    let n_inode = create_tmp_inode_from_sb_blk(
        dir.super_blk.upgrade().unwrap().clone(),
        attr.ino,
        inode_mode,
        0,
        inode_ops_from_inode_mode(inode_mode),
        file_ops_from_inode_mode(inode_mode),
        None,
    )?;

    // update the parent size of the directory
    dir.access_inner().file_size += 1;
    // set dentry with inode
    dentry.access_inner().d_inode = n_inode;
    ddebug!("dbfs_common_create end");
    Ok(())
}

/// checkout the permission
pub fn checkout_access(
    p_uid: u32,
    p_gid: u32,
    mode: u16,
    uid: u32,
    gid: u32,
    access_mask: u16,
) -> bool {
    if access_mask == 0 {
        return true;
    }
    let permission = mode;
    let mut access_mask = access_mask;
    // root is allowed to read & write anything
    if uid == 0 {
        // root only allowed to exec if one of the X bits is set
        access_mask &= 0o1;
        access_mask -= access_mask & (permission >> 6);
        access_mask -= access_mask & (permission >> 3);
        access_mask -= access_mask & permission;
        return access_mask == 0;
    }
    // check user
    if p_uid == uid {
        access_mask -= access_mask & (permission >> 6);
    } else if p_gid == gid {
        access_mask -= access_mask & (permission >> 3);
    } else {
        // check other
        access_mask -= access_mask & permission;
    }

    access_mask == 0
}

fn creation_gid(p_gid: u32, p_mode: DbfsPermission, gid: u32) -> u32 {
    if p_mode.contains(DbfsPermission::S_ISGID) {
        return p_gid;
    }
    gid
}

///
pub fn dbfs_common_create(
    dir: usize,
    name: &str,
    uid: u32,
    gid: u32,
    c_time: usize,
    permission: DbfsPermission,
    target_path: Option<&str>,
) -> Result<DbfsAttr, ()> {
    ddebug!("dbfs_common_create");
    let new_number = DBFS_INODE_NUMBER.fetch_add(1, core::sync::atomic::Ordering::SeqCst);
    let db = clone_db();
    let tx = db.tx(true).unwrap();

    // find the dir
    let parent = tx.get_bucket(dir.to_be_bytes()).unwrap();

    // check the permission
    let p_uid = parent.get_kv("uid").unwrap();
    let p_uid = u32!(p_uid.value());
    let p_gid = parent.get_kv("gid").unwrap();
    let p_gid = u32!(p_gid.value());
    let p_mode = parent.get_kv("mode").unwrap();
    let p_mode = u16!(p_mode.value());
    let bool = checkout_access(p_uid, p_gid, p_mode & 0o777, uid, gid, 0o2);
    if !bool {
        return Err(());
    }

    let next_number = parent.get_kv("size").unwrap();
    let next_number = usize!(next_number.value());
    // update the size of the dir
    parent.put("size", (next_number + 1).to_be_bytes()).unwrap();

    let key = format!("data{}", next_number);
    let value = format!("{}:{}", name, new_number);
    parent.put(key, value).unwrap(); // add a new entry to the dir

    // update dir ctime/mtime
    parent.put("ctime", c_time.to_be_bytes()).unwrap();
    parent.put("mtime", c_time.to_be_bytes()).unwrap();

    let mut mode = permission;
    if uid != 0 {
        mode -= DbfsPermission::S_ISUID;
        mode -= DbfsPermission::S_ISGID;
    }

    let p_mode = DbfsPermission::from_bits_truncate(p_mode);

    {
        if permission.contains(DbfsPermission::S_IFDIR) {
            // for dir, set the S_ISGID bit if the parent dir has the S_ISGID bit set
            if p_mode.contains(DbfsPermission::S_IFDIR) {
                mode |= DbfsPermission::S_ISGID;
            }
        }
    }

    // set the gid of inode
    let gid = creation_gid(p_gid, permission, gid);

    // create a new inode
    warn!("dbfs_common_create: create a new inode {}", new_number);
    let new_inode = tx.create_bucket(new_number.to_be_bytes()).unwrap();

    // set the mode of inode
    new_inode.put("mode", mode.bits().to_be_bytes()).unwrap();
    // set the size of inode to 0

    let (hard_link, file_size) = if permission.contains(DbfsPermission::S_IFDIR) {
        (2u32, 2usize)
    } else if permission.contains(DbfsPermission::S_IFLNK) {
        assert!(target_path.is_some());
        (1u32, target_path.as_ref().unwrap().len())
    } else {
        (1u32, 0usize)
    };
    if permission.contains(DbfsPermission::S_IFDIR) {
        new_inode.put("next_number", 0usize.to_be_bytes()).unwrap();
        new_inode.put("hard_links", 2u32.to_be_bytes()).unwrap();
        let dot = format!("data{}", 0);
        let dot_value = format!("{}:{}", ".", new_number);
        new_inode.put(dot, dot_value).unwrap();
        let dotdot = format!("data{}", 1);
        let dotdot_value = format!("{}:{}", "..", dir);
        new_inode.put(dotdot, dotdot_value).unwrap();
    }
    new_inode.put("size", file_size.to_be_bytes()).unwrap();
    new_inode
        .put("hard_links", hard_link.to_be_bytes())
        .unwrap();
    new_inode.put("uid", uid.to_be_bytes()).unwrap();
    new_inode.put("gid", gid.to_be_bytes()).unwrap();
    // set time
    new_inode.put("atime", c_time.to_be_bytes()).unwrap();
    new_inode.put("mtime", c_time.to_be_bytes()).unwrap();
    new_inode.put("ctime", c_time.to_be_bytes()).unwrap();

    new_inode.put("block_size", 512u32.to_be_bytes()).unwrap();
    if permission.contains(DbfsPermission::S_IFLNK) {
        new_inode.put("data", target_path.unwrap()).unwrap();
    }
    tx.commit().map_err(|_| ())?;

    let dbfs_attr = DbfsAttr {
        ino: new_number,
        size: file_size,
        blocks: 0,
        atime: DbfsTimeSpec::from(c_time),
        mtime: DbfsTimeSpec::from(c_time),
        ctime: DbfsTimeSpec::from(c_time),
        crtime: DbfsTimeSpec::from(0),
        kind: DbfsFileType::from(permission),
        perm: mode.bits(),
        nlink: hard_link,
        uid,
        gid,
        rdev: 0,
        blksize: 512,
        padding: 0,
        flags: 0,
    };

    ddebug!("dbfs_common_create end");
    Ok(dbfs_attr)
}

pub fn dbfs_common_access(p_uid:u32,p_gid:u32,ino:usize,mask:i32)->DbfsResult<bool>{
    let db = clone_db();
    let tx = db.tx(false)?;
    let inode = tx.get_bucket(ino.to_be_bytes())?;

    let mode = inode.get_kv("mode").unwrap();
    let mode = u16!(mode.value());
    let uid = inode.get_kv("uid").unwrap();
    let uid = u32!(uid.value());
    let gid = inode.get_kv("gid").unwrap();
    let gid = u32!(gid.value());
    let res = checkout_access(
        p_uid,
        p_gid,
        mode,
        uid,
        gid,
        mask as u16,
    );
    Ok(res)
}



pub fn dbfs_common_truncate(r_uid:u32,r_gid:u32,ino:usize,ctime:usize,f_size:usize)->DbfsResult<DbfsAttr>{
    warn!("dbfs_truncate: set size to {}", f_size);
    let mut attr = dbfs_common_attr(ino).map_err(|_| DbfsError::NotFound)?;
    // checkout permission
    if !checkout_access(attr.uid,attr.gid,attr.perm,r_uid,r_gid,ACCESS_W_OK){
        return Err(DbfsError::AccessError);
    }

    let db = clone_db();
    let tx = db.tx(true)?;
    let bucket = tx.get_bucket(ino.to_be_bytes()).unwrap();
    let start = f_size / 512;
    let offset = f_size % 512;

    let current_size = attr.size;
    // if current file size < f_size, allocate new blocks
    // if current file size > f_size, free blocks

    let current_block = current_size / 512;
    if current_block < start {
        // We don't need to allocate new blocks
        // When write or read occurs, it will allocate new blocks or ignore
        // We need set the size of the file
        let sb_blk = tx.get_bucket("super_blk".as_bytes()).unwrap();
        let disk_size = sb_blk.get_kv("disk_size").unwrap();
        let disk_size = u64!(disk_size.value());
        let gap = f_size.saturating_sub(current_size); // newsize - oldsize
        if disk_size < gap as u64 {
            return Err(DbfsError::NoSpace);
        }
        let new_disk_size = disk_size - gap as u64;
        sb_blk
            .put("disk_size", new_disk_size.to_be_bytes())?;
    } else if current_block >= start {
        // we need to free blocks
        for i in start + 1..=current_block {
            let key = format!("data{:04x}", i);
            if bucket.get_kv(&key).is_some() {
                bucket.delete(&key)?;
            }
        }
        // fill the first data to zero
        let start_key = format!("data{:04x}", start);
        let value = bucket.get_kv(&start_key);
        if value.is_some() {
            let value = value.unwrap();
            let mut value = value.value().to_vec();
            // set the data in offset to 0
            for i in offset..512 {
                value[i] = 0;
            }
            bucket.put(start_key, value).unwrap();
        }
        let sb_blk = tx.get_bucket("super_blk".as_bytes()).unwrap();
        let disk_size = sb_blk.get_kv("disk_size").unwrap();
        let disk_size = u64!(disk_size.value());
        let additional_size = (current_block - start) * 512; // 1 - 0
        let new_disk_size = disk_size + additional_size as u64;
        sb_blk
            .put("disk_size", new_disk_size.to_be_bytes())?;
    }
    // update inode size
    bucket.put("size", f_size.to_be_bytes())?;
    // update ctime/mtime
    bucket.put("ctime", ctime.to_be_bytes())?;
    bucket.put("mtime", ctime.to_be_bytes())?;
    //Clear SETUID & SETGID on truncate
    let perm = attr.perm;
    let new_perm = clear_suid_sgid(DbfsPermission::from_bits_truncate(perm));
    bucket.put("mode", new_perm.bits().to_be_bytes())?;

    attr.size = f_size;
    attr.ctime = ctime.into();
    attr.mtime = ctime.into();
    attr.perm = new_perm.bits();

    tx.commit()?;
    Ok(attr)
}


pub fn dbfs_common_rmdir(r_uid:u32,r_gid:u32,p_ino:usize,name:&str,c_time:usize)->DbfsResult<()>{
    let db = clone_db();
    let tx = db.tx(true)?;
    let p_bucket = tx.get_bucket(p_ino.to_be_bytes())?;
    // find the inode for the name
    let value = p_bucket.kv_pairs().find(|kv| {
        kv.key().starts_with("data".as_bytes()) && kv.value().starts_with(name.as_bytes())
    });
    if value.is_none() {
        return Err(DbfsError::NotFound);
    }
    let value = value.unwrap();
    let v_value = value.value();
    let str = core::str::from_utf8(v_value).unwrap();
    let data = str.rsplitn(2, ':').collect::<Vec<&str>>();
    let number = data[0].parse::<usize>().unwrap();
    let bucket = tx.get_bucket(number.to_be_bytes()).unwrap();

    // checkout the directory is empty
    let size = bucket.get_kv("size").unwrap();
    let size = usize!(size.value());
    // if size > 2, it means the directory is not empty
    //  Directories always have a self and parent link
    if size > 2{
        return Err(DbfsError::NotEmpty);
    }
    let p_uid = p_bucket.get_kv("uid").unwrap();
    let p_uid = u32!(p_uid.value());
    let p_gid = p_bucket.get_kv("gid").unwrap();
    let p_gid = u32!(p_gid.value());
    let p_mode = p_bucket.get_kv("mode").unwrap();
    let p_mode = u16!(p_mode.value());
    let p_size = p_bucket.get_kv("size").unwrap();
    let p_size = usize!(p_size.value());
    if !checkout_access(
        p_uid,
        p_gid,
        p_mode&0o777,
        r_uid,
        r_gid,
        ACCESS_W_OK
    ){
        return Err(DbfsError::AccessError);
    }
    // "Sticky bit" handling
    let uid = bucket.get_kv("uid").unwrap();
    let uid = u32!(uid.value());
    let p_perm = DbfsPermission::from_bits_truncate(p_mode);
    if p_perm.contains(DbfsPermission::S_ISVTX)
        && r_uid !=0 && r_uid != p_uid && r_uid != uid{
        return Err(DbfsError::AccessError);
    }
    // update the parent directory
    p_bucket.put("mtime", c_time.to_be_bytes())?;
    p_bucket.put("ctime", c_time.to_be_bytes())?;
    // delete the directory
    p_bucket.delete(value.key())?;
    p_bucket.put("size", (p_size - 1).to_be_bytes())?;
    // delete the inode
    tx.delete_bucket(number.to_be_bytes())?;
    tx.commit()?;
    Ok(())
}

fn inode_ops_from_inode_mode(inode_mode: InodeMode) -> InodeOps {
    match inode_mode {
        InodeMode::S_FILE => DBFS_FILE_INODE_OPS,
        InodeMode::S_DIR => DBFS_DIR_INODE_OPS,
        InodeMode::S_SYMLINK => DBFS_SYMLINK_INODE_OPS,
        _ => InodeOps::empty(),
    }
}

fn file_ops_from_inode_mode(inode_mode: InodeMode) -> FileOps {
    match inode_mode {
        InodeMode::S_FILE => DBFS_FILE_FILE_OPS,
        InodeMode::S_DIR => DBFS_DIR_FILE_OPS,
        InodeMode::S_SYMLINK => DBFS_SYMLINK_FILE_OPS,
        _ => FileOps::empty(),
    }
}
