use crate::common::{DbfsAttr, DbfsPermission, DbfsResult, DbfsTimeSpec};
use downcast::_std::time::SystemTime;
use fuser::{FileAttr, Request};

use rvfs::warn;

use crate::inode::{dbfs_common_create, dbfs_common_lookup, dbfs_common_truncate};

pub fn dbfs_fuse_lookup(parent: u64, name: &str) -> Result<FileAttr, ()> {
    warn!("dbfs_fuse_lookup(parent:{},name:{})", parent, name);
    dbfs_common_lookup(parent as usize, name).map(|x| x.into())
}

pub fn dbfs_fuse_create(
    req: &Request<'_>,
    parent: u64,
    name: &str,
    mode: u32,
    flags: i32,
) -> Result<FileAttr, ()> {
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
            return Err(());
        }
    };

    let permission = DbfsPermission::from_bits_truncate(mode as u16);
    let uid = req.uid();
    let gid = req.gid();
    let ctime = DbfsTimeSpec::from(SystemTime::now()).into();
    let res = dbfs_common_create(parent as usize, name, uid, gid, ctime, permission, None);
    if res.is_err() {
        return Err(());
    }
    Ok(res.unwrap().into())
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
    let ctime = DbfsTimeSpec::from(SystemTime::now()).into();
    let res = dbfs_common_create(parent as usize, name, uid, gid, ctime, permission, None);
    if res.is_err() {
        return Err(());
    }
    Ok(res.unwrap().into())
}



pub fn dbfs_fuse_truncate(req:&Request<'_>,ino: u64, size: u64) -> DbfsResult<DbfsAttr> {
    warn!("dbfs_fuse_truncate(ino:{},size:{})", ino, size);
    let uid = req.uid();
    let gid = req.gid();
    let ctime = DbfsTimeSpec::from(SystemTime::now()).into();
    dbfs_common_truncate(uid, gid, ino as usize, ctime, size as usize)
}