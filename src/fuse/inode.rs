use downcast::_std::{println, time::SystemTime};
use fuser::{FileAttr, Request};
use rvfs::warn;

use crate::{
    common::{DbfsAttr, DbfsError, DbfsPermission, DbfsResult, DbfsTimeSpec, MAX_PATH_LEN},
    inode::{
        dbfs_common_create, dbfs_common_fallocate, dbfs_common_lookup, dbfs_common_rename,
        dbfs_common_rmdir, dbfs_common_truncate,
    },
};

pub fn dbfs_fuse_lookup(parent: u64, name: &str) -> DbfsResult<FileAttr> {
    warn!("dbfs_fuse_lookup(parent:{},name:{})", parent, name);
    if name.len() > MAX_PATH_LEN {
        return Err(DbfsError::NameTooLong);
    }
    let res = dbfs_common_lookup(parent as usize, name);
    res.map(|attr| attr.into())
}

pub fn dbfs_fuse_create(
    req: &Request<'_>,
    parent: u64,
    name: &str,
    mode: u32,
    flags: i32,
) -> DbfsResult<FileAttr> {
    warn!(
        "dbfs_fuse_create(parent:{},name:{},mode:{})",
        parent, name, mode
    );
    // checkout the open flags
    let (_read, _write) = match flags & libc::O_ACCMODE {
        libc::O_RDONLY => (true, false),
        libc::O_WRONLY => (false, true),
        libc::O_RDWR => (true, true),
        // Exactly one access mode flag must be specified
        _ => {
            return Err(DbfsError::InvalidArgument);
        }
    };

    let permission = DbfsPermission::from_bits_truncate(mode as u16);
    let uid = req.uid();
    let gid = req.gid();
    let ctime = DbfsTimeSpec::from(SystemTime::now());
    let res = dbfs_common_create(
        parent as usize,
        name,
        uid,
        gid,
        ctime,
        permission,
        None,
        None,
    );
    res.map(|attr| attr.into())
}

/// Create a directory
///
/// Note that the mode argument may not have the type specification bits set, i.e. S_ISDIR(mode) can be false. To obtain the correct directory type bits use mode|S_IFDIR
pub fn dbfs_fuse_mkdir(
    req: &Request<'_>,
    parent: u64,
    name: &str,
    mode: u32,
) -> Result<FileAttr, ()> {
    warn!(
        "dbfs_fuse_mkdir(parent:{},name:{},mode:{})",
        parent, name, mode
    );
    let mut permission = DbfsPermission::from_bits_truncate(mode as u16);
    permission |= DbfsPermission::S_IFDIR;
    let uid = req.uid();
    let gid = req.gid();
    let ctime = DbfsTimeSpec::from(SystemTime::now());
    let res = dbfs_common_create(
        parent as usize,
        name,
        uid,
        gid,
        ctime,
        permission,
        None,
        None,
    );
    if res.is_err() {
        return Err(());
    }
    Ok(res.unwrap().into())
}

pub fn dbfs_fuse_truncate(req: &Request<'_>, ino: u64, size: u64) -> DbfsResult<DbfsAttr> {
    warn!("dbfs_fuse_truncate(ino:{},size:{})", ino, size);
    let uid = req.uid();
    let gid = req.gid();
    let ctime = DbfsTimeSpec::from(SystemTime::now());
    dbfs_common_truncate(uid, gid, ino as usize, ctime, size as usize)
}

pub fn dbfs_fuse_rmdir(req: &Request<'_>, parent: u64, name: &str) -> DbfsResult<()> {
    warn!("dbfs_fuse_rmdir(parent:{},name:{})", parent, name);
    let uid = req.uid();
    let gid = req.gid();
    let ctime = DbfsTimeSpec::from(SystemTime::now());
    dbfs_common_rmdir(uid, gid, parent as usize, name, ctime)
}

pub fn dbfs_fuse_fallocate(
    req: &Request<'_>,
    ino: u64,
    offset: u64,
    size: u64,
    mode: u32,
) -> DbfsResult<()> {
    warn!(
        "dbfs_fuse_fallocate(ino:{},offset:{},size:{},mode:{})",
        ino, offset, size, mode
    );
    let uid = req.uid();
    let gid = req.gid();
    let ctime = DbfsTimeSpec::from(SystemTime::now());
    dbfs_common_fallocate(
        uid,
        gid,
        ino as usize,
        offset as usize,
        size as usize,
        mode,
        ctime,
    )
}

pub fn dbfs_fuse_rename(
    req: &Request<'_>,
    parent: u64,
    name: &str,
    newparent: u64,
    newname: &str,
    flags: u32,
) -> DbfsResult<()> {
    warn!(
        "dbfs_fuse_rename(parent:{},name:{},newparent:{},newname:{})",
        parent, name, newparent, newname
    );
    let uid = req.uid();
    let gid = req.gid();
    let ctime = DbfsTimeSpec::from(SystemTime::now());
    dbfs_common_rename(
        uid,
        gid,
        parent as usize,
        name,
        newparent as usize,
        newname,
        flags,
        ctime,
    )
}

pub fn dbfs_fuse_mknod(
    req: &Request<'_>,
    parent: u64,
    name: &str,
    mode: u32,
    dev: u32,
) -> DbfsResult<DbfsAttr> {
    warn!(
        "dbfs_fuse_mknod(parent:{},name:{},mode:{},dev:{})",
        parent, name, mode, dev
    );
    let permission = DbfsPermission::from_bits_truncate(mode as u16);
    // if !permission.contains(DbfsPermission::S_IFDIR)
    //     && !permission.contains(DbfsPermission::S_IFREG)
    //     && !permission.contains(DbfsPermission::S_IFLNK){
    //         return Err(DbfsError::NoSys);
    //     }
    println!("permission:{:?}", permission);
    let uid = req.uid();
    let gid = req.gid();
    let ctime = DbfsTimeSpec::from(SystemTime::now());
    dbfs_common_create(
        parent as usize,
        name,
        uid,
        gid,
        ctime,
        permission,
        None,
        Some(dev),
    )
}
