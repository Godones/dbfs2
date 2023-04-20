pub mod file;
pub mod inode;
mod mkfs;
mod attr;

extern crate std;

use alloc::vec;
use downcast::_std::time::SystemTime;
use fuser::{
    Filesystem, ReplyAttr, ReplyCreate, ReplyData, ReplyDirectory, ReplyEmpty, ReplyEntry,
    ReplyWrite, Request, TimeOrNow,
};
use libc::ENOENT;
use std::ffi::OsStr;
use std::time::Duration;

use crate::fuse::file::{dbfs_fuse_read, dbfs_fuse_readdir, dbfs_fuse_write};
use crate::fuse::inode::{dbfs_fuse_create, dbfs_fuse_lookup};

pub use mkfs::init_dbfs_fuse;
use crate::fuse::attr::{dbfs_fuse_getattr, dbfs_fuse_setattr};

const TTL: Duration = Duration::from_secs(1); // 1 second

pub struct DbfsFuse;

impl Filesystem for DbfsFuse {
    /// The lookup() method is called when the kernel wants to know about a file.
    ///
    /// Parameters:
    /// * req: The request that triggered this operation.
    /// * parent: The inode number of the parent directory of the file.
    /// * name: The name of the file.
    /// * reply: The reply to send back to the kernel.
    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        let res = dbfs_fuse_lookup(parent, name.to_str().unwrap());
        match res {
            Ok(attr) => reply.entry(&TTL, &attr, 0),
            Err(_) => reply.error(ENOENT),
        }
    }
    fn getattr(&mut self, _req: &Request<'_>, ino: u64, reply: ReplyAttr) {
        let res = dbfs_fuse_getattr(ino);
        match res {
            Ok(attr) => reply.attr(&TTL, &attr),
            Err(_) => reply.error(ENOENT),
        }
    }
    fn read(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        _fh: u64,
        offset: i64,
        size: u32,
        _flags: i32,
        _lock_owner: Option<u64>,
        reply: ReplyData,
    ) {
        let mut data = vec![0u8; size as usize];
        let res = dbfs_fuse_read(ino, offset, data.as_mut_slice());
        match res {
            Ok(_) => reply.data(data.as_slice()),
            Err(_) => reply.error(ENOENT),
        }
    }
    fn write(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        _fh: u64,
        offset: i64,
        data: &[u8],
        _write_flags: u32,
        _flags: i32,
        _lock_owner: Option<u64>,
        reply: ReplyWrite,
    ) {
        let res = dbfs_fuse_write(ino, offset, data);
        match res {
            Ok(_) => reply.written(res.unwrap() as u32),
            Err(_) => reply.error(ENOENT),
        }
    }

    fn readdir(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        _fh: u64,
        offset: i64,
        reply: ReplyDirectory,
    ) {
        dbfs_fuse_readdir(ino, offset, reply)
    }

    fn create(
        &mut self,
        req: &Request<'_>,
        parent: u64,
        name: &OsStr,
        mode: u32,
        _umask: u32,
        flags: i32,
        reply: ReplyCreate,
    ) {
        let res = dbfs_fuse_create(req, parent, name.to_str().unwrap(), mode, flags);
        match res {
            Ok(attr) => reply.created(&TTL, &attr, 0, 0, 0),
            Err(_) => reply.error(ENOENT),
        }
    }
    fn flush(
        &mut self,
        _req: &Request<'_>,
        _ino: u64,
        _fh: u64,
        _lock_owner: u64,
        reply: ReplyEmpty,
    ) {
        reply.ok();
    }

    fn setattr(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        mode: Option<u32>,
        uid: Option<u32>,
        gid: Option<u32>,
        size: Option<u64>,
        atime: Option<TimeOrNow>,
        mtime: Option<TimeOrNow>,
        ctime: Option<SystemTime>,
        _fh: Option<u64>,
        crtime: Option<SystemTime>,
        chgtime: Option<SystemTime>,
        bkuptime: Option<SystemTime>,
        _flags: Option<u32>,
        reply: ReplyAttr,
    ) {
        let res = dbfs_fuse_setattr(ino,size,atime,mtime,_fh,ctime,_flags);
        reply.attr(&TTL, &res.unwrap());
    }
}
