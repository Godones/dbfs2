use downcast::_std::time::SystemTime;
use fuser::{FileAttr, TimeOrNow};
use log::{error, warn};
use crate::inode::dbfs_common_attr;


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
    error!("dbfs_fuse_setattr(ino:{})", ino);
    let attr = dbfs_common_attr(ino as usize)?;
    Ok(attr.into())
}