use downcast::_std::time::SystemTime;
use fuser::{FileAttr, Request, TimeOrNow};
use log::warn;

use crate::{
    attr::{
        dbfs_common_chmod, dbfs_common_chown, dbfs_common_getxattr, dbfs_common_listxattr,
        dbfs_common_removexattr, dbfs_common_setxattr, dbfs_common_utimens,
    },
    common::{DbfsAttr, DbfsFsStat, DbfsResult, DbfsTimeSpec},
    fs_type::dbfs_common_statfs,
    inode::{dbfs_common_access, dbfs_common_attr},
};

pub fn dbfs_fuse_getattr(ino: u64) -> DbfsResult<FileAttr> {
    warn!("dbfs_fuse_getattr(ino:{})", ino);
    dbfs_common_attr(ino as usize).map(|x| x.into())
}

pub fn dbfs_fuse_statfs() -> DbfsResult<DbfsFsStat> {
    warn!("dbfs_fuse_statfs)");
    dbfs_common_statfs(None, None, None)
}

pub fn dbfs_fuse_access(req: &Request<'_>, ino: u64, mask: i32) -> DbfsResult<bool> {
    warn!("dbfs_fuse_access(ino:{})", ino);
    dbfs_common_access(req.uid(), req.gid(), ino as usize, mask)
}

pub fn dbfs_fuse_setxattr(
    req: &Request<'_>,
    ino: u64,
    name: &str,
    value: &[u8],
    _flags: i32,
    _position: u32,
) -> DbfsResult<()> {
    warn!(
        "dbfs_fuse_setxattr(ino:{},name:{:?},value:{:?})",
        ino, name, value
    );
    let time = DbfsTimeSpec::from(SystemTime::now());
    dbfs_common_setxattr(req.uid(), req.gid(), ino as usize, name, value, time)
}

pub fn dbfs_fuse_getxattr(
    req: &Request<'_>,
    ino: u64,
    name: &str,
    buf: &mut [u8],
) -> DbfsResult<usize> {
    warn!("dbfs_fuse_getxattr(ino:{},name:{:?})", ino, name);
    dbfs_common_getxattr(req.uid(), req.gid(), ino as usize, name, buf)
}

pub fn dbfs_fuse_listxattr(req: &Request<'_>, ino: u64, buf: &mut [u8]) -> DbfsResult<usize> {
    warn!("dbfs_fuse_listxattr(ino:{})", ino);
    dbfs_common_listxattr(req.uid(), req.gid(), ino as usize, buf)
}

pub fn dbfs_fuse_removexattr(req: &Request<'_>, ino: u64, name: &str) -> DbfsResult<()> {
    warn!("dbfs_fuse_removexattr(ino:{},name:{:?})", ino, name);
    let time = DbfsTimeSpec::from(SystemTime::now());
    dbfs_common_removexattr(req.uid(), req.gid(), ino as usize, name, time)
}

/// Change the permission bits of a file
///
/// fi will always be NULL if the file is not currently open, but may also be NULL if the file is open.
pub fn dbfs_fuse_chmod(req: &Request<'_>, ino: u64, mode: u32) -> DbfsResult<DbfsAttr> {
    warn!("dbfs_fuse_chmod(ino:{},mode:{})", ino, mode);
    let time = DbfsTimeSpec::from(SystemTime::now());
    dbfs_common_chmod(req.uid(), req.gid(), ino as usize, mode as u16, time)
}

///
pub fn dbfs_fuse_chown(
    req: &Request<'_>,
    ino: u64,
    uid: Option<u32>,
    gid: Option<u32>,
) -> DbfsResult<DbfsAttr> {
    warn!("dbfs_fuse_chown(ino:{},uid:{:?},gid:{:?})", ino, uid, gid);
    let time = DbfsTimeSpec::from(SystemTime::now());
    dbfs_common_chown(req.uid(), req.gid(), ino as usize, uid, gid, time)
}

pub fn dbfs_fuse_utimens(
    req: &Request<'_>,
    ino: u64,
    atime: Option<TimeOrNow>,
    mtime: Option<TimeOrNow>,
) -> DbfsResult<DbfsAttr> {
    warn!(
        "dbfs_fuse_utimens(ino:{},atime:{:?},mtime:{:?})",
        ino, atime, mtime
    );
    let atime = atime.map(|t| match t {
        TimeOrNow::Now => DbfsTimeSpec::from(SystemTime::now()),
        TimeOrNow::SpecificTime(t) => DbfsTimeSpec::from(t),
    });
    let mtime = mtime.map(|t| match t {
        TimeOrNow::Now => DbfsTimeSpec::from(SystemTime::now()),
        TimeOrNow::SpecificTime(t) => DbfsTimeSpec::from(t),
    });

    let ctime = DbfsTimeSpec::from(SystemTime::now());
    warn!(
        "dbfs_fuse_utimens(ino:{},atime:{:?},mtime:{:?},ctime:{:?})",
        ino, atime, mtime, ctime
    );
    dbfs_common_utimens(req.uid(), req.gid(), ino as usize, atime, mtime, ctime)
}
