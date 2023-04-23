use crate::common::{DbfsFsStat, DbfsResult};
use crate::fs_type::dbfs_common_statfs;
use crate::inode::{dbfs_common_access, dbfs_common_attr};
use downcast::_std::time::SystemTime;
use fuser::{FileAttr, Request, TimeOrNow};
use log::warn;

pub fn dbfs_fuse_getattr(ino: u64) -> Result<FileAttr, ()> {
    warn!("dbfs_fuse_getattr(ino:{})", ino);
    dbfs_common_attr(ino as usize).map(|x| x.into())
}

pub fn dbfs_fuse_setattr(
    ino: u64,
    _size: Option<u64>,
    _atime: Option<TimeOrNow>,
    _mtime: Option<TimeOrNow>,
    _fh: Option<u64>,
    _crtime: Option<SystemTime>,
    _flags: Option<u32>,
) -> Result<FileAttr, ()> {
    warn!("dbfs_fuse_setattr(ino:{})", ino);
    let attr = dbfs_common_attr(ino as usize)?;
    Ok(attr.into())
}

pub fn dbfs_fuse_statfs() -> Result<DbfsFsStat, ()> {
    warn!("dbfs_fuse_statfs)");
    dbfs_common_statfs(None, None, None)
}


pub fn dbfs_fuse_access(req:&Request<'_>,ino: u64, mask: i32) -> DbfsResult<bool> {
    warn!("dbfs_fuse_access(ino:{})", ino);
    dbfs_common_access(req.uid(),req.gid(),ino as usize, mask)
}