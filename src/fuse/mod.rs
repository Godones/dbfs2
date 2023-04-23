pub mod attr;
pub mod file;
pub mod inode;
pub mod link;
mod mkfs;

extern crate std;

use alloc::sync::Arc;
use alloc::vec;
use downcast::_std::path::Path;
use downcast::_std::time::SystemTime;
use fuser::consts::FOPEN_DIRECT_IO;
use fuser::{
    Filesystem, KernelConfig, ReplyAttr, ReplyCreate, ReplyData, ReplyDirectory, ReplyEmpty,
    ReplyEntry, ReplyOpen, ReplyStatfs, ReplyWrite, Request, TimeOrNow,
};
use jammdb::DB;
use libc::{c_int, ENOENT};
use log::{error, info, warn};
use std::ffi::OsStr;
use std::time::Duration;

use crate::fuse::file::{
    dbfs_fuse_open, dbfs_fuse_opendir, dbfs_fuse_read, dbfs_fuse_readdir, dbfs_fuse_write,
};
use crate::fuse::inode::{dbfs_fuse_create, dbfs_fuse_lookup, dbfs_fuse_mkdir};

use crate::common::DbfsTimeSpec;
use crate::fs_type::dbfs_common_root_inode;
use crate::fuse::attr::{dbfs_fuse_access, dbfs_fuse_getattr, dbfs_fuse_setattr, dbfs_fuse_statfs};
use crate::fuse::link::{dbfs_fuse_link, dbfs_fuse_readlink, dbfs_fuse_symlink, dbfs_fuse_unlink};
use crate::fuse::mkfs::{init_db, test_dbfs, FakeMMap, MyOpenOptions};
use crate::init_dbfs;
pub use mkfs::init_dbfs_fuse;

const TTL: Duration = Duration::from_secs(1); // 1 second
const FILE_SIZE: u64 = 1024 * 1024 * 128; // 1 GiB

pub struct DbfsFuse {
    direct_io: bool,
    suid_support: bool,
}

impl DbfsFuse {
    pub fn new(direct_io: bool, _suid_support: bool) -> Self {
        #[cfg(feature = "abi-7-26")]
        {
            SimpleFS {
                data_dir,
                next_file_handle: AtomicU64::new(1),
                direct_io,
                suid_support,
            }
        }
        #[cfg(not(feature = "abi-7-26"))]
        {
            Self {
                direct_io,
                suid_support: false,
            }
        }
    }
}

impl Filesystem for DbfsFuse {
    fn init(&mut self, _req: &Request<'_>, _config: &mut KernelConfig) -> Result<(), c_int> {
        let path = "./test.dbfs";
        let db = DB::open::<MyOpenOptions, _>(Arc::new(FakeMMap), path).map_err(|_| -1)?; // TODO: error handling
        init_db(&db, FILE_SIZE);
        test_dbfs(&db);
        init_dbfs(db);
        let uid = unsafe { libc::getuid() };
        let gid = unsafe { libc::getgid() };
        let time = DbfsTimeSpec::from(SystemTime::now());
        dbfs_common_root_inode(uid, gid, time.into()).map_err(|_| -1)?;
        Ok(())
    }
    /// Clean up filesystem
    ///
    /// Called on filesystem exit.
    fn destroy(&mut self, _req: &Request<'_>) {
        // TODO: close db
        // we need write back the metadata
        // 1. continue_number to super_block
        // 2. disk_size to super_blk
        error!("filesystem exit");
    }
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
            Err(_) => {
                if name == "." || name == ".."{
                    panic!("lookup panic");
                }
                reply.error(ENOENT)
            }
        }
    }
    fn getattr(&mut self, _req: &Request<'_>, ino: u64, reply: ReplyAttr) {
        let res = dbfs_fuse_getattr(ino);
        match res {
            Ok(attr) => reply.attr(&TTL, &attr),
            Err(_) => {
                panic!("getattr error");
                reply.error(ENOENT)
            }
        }
    }
    fn setattr(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        _mode: Option<u32>,
        _uid: Option<u32>,
        _gid: Option<u32>,
        size: Option<u64>,
        atime: Option<TimeOrNow>,
        mtime: Option<TimeOrNow>,
        ctime: Option<SystemTime>,
        _fh: Option<u64>,
        _crtime: Option<SystemTime>,
        _chgtime: Option<SystemTime>,
        _bkuptime: Option<SystemTime>,
        _flags: Option<u32>,
        reply: ReplyAttr,
    ) {
        let res = dbfs_fuse_setattr(ino, size, atime, mtime, _fh, ctime, _flags);
        reply.attr(&TTL, &res.unwrap());
    }

    /// Read the target of a symbolic link
    ///
    /// The buffer should be filled with a null terminated string. The buffer size argument includes the space for the terminating null character.
    /// If the linkname is too long to fit in the buffer, it should be truncated. The return value should be 0 for success.
    fn readlink(&mut self, _req: &Request<'_>, ino: u64, reply: ReplyData) {
        let res = dbfs_fuse_readlink(ino);
        match res {
            Ok(data) => reply.data(&data),
            Err(x) => reply.error(x as i32),
        }
    }

    /// Create a directory
    ///
    /// Note that the mode argument may not have the type specification bits set, i.e. S_ISDIR(mode) can be false. To obtain the correct directory type bits use mode|S_IFDIR
    fn mkdir(
        &mut self,
        req: &Request<'_>,
        parent: u64,
        name: &OsStr,
        mode: u32,
        _umask: u32,
        reply: ReplyEntry,
    ) {
        let res = dbfs_fuse_mkdir(req, parent, name.to_str().unwrap(), mode);
        match res {
            Ok(attr) => reply.entry(&TTL, &attr, 0),
            Err(_) => reply.error(ENOENT),
        }
    }
    /// Remove a file
    fn unlink(&mut self, req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEmpty) {
        let res = dbfs_fuse_unlink(req, parent, name.to_str().unwrap());
        match res {
            Ok(_) => reply.ok(),
            Err(_) => {
                panic!("unlink panic");
                reply.error(ENOENT)
            }
        }
    }

    /// Create a symbolic link
    fn symlink(
        &mut self,
        req: &Request<'_>,
        parent: u64,
        name: &OsStr,
        link: &Path,
        reply: ReplyEntry,
    ) {
        let res = dbfs_fuse_symlink(req, parent, name.to_str().unwrap(), link.to_str().unwrap());
        match res {
            Ok(attr) => reply.entry(&TTL, &attr.into(), 0),
            Err(_) => reply.error(ENOENT),
        }
    }

    /// Create a hard link to a file
    fn link(
        &mut self,
        req: &Request<'_>,
        ino: u64,
        newparent: u64,
        newname: &OsStr,
        reply: ReplyEntry,
    ) {
        let res = dbfs_fuse_link(req, ino, newparent, newname.to_str().unwrap());
        match res {
            Ok(attr) => reply.entry(&TTL, &attr.into(), 0),
            Err(e) => reply.error(e as i32),
        }
    }

    /// Open flags are available in fi->flags. The following rules apply.
    ///
    /// * Creation (O_CREAT, O_EXCL, O_NOCTTY) flags will be filtered out / handled by the kernel.
    /// * Access modes (O_RDONLY, O_WRONLY, O_RDWR, O_EXEC, O_SEARCH) should be used by the filesystem to check if the operation is permitted.
    /// If the -o default_permissions mount option is given, this check is already done by the kernel before calling open() and may thus be omitted by the filesystem.
    /// * When writeback caching is enabled, the kernel may send read requests even for files opened with O_WRONLY. The filesystem should be prepared to handle this.
    /// * When writeback caching is disabled, the filesystem is expected to properly handle the O_APPEND flag and ensure that each write is appending to the end of the file.
    /// * When writeback caching is enabled, the kernel will handle O_APPEND. However, unless all changes to the file come through the kernel this will not work reliably.
    /// The filesystem should thus either ignore the O_APPEND flag (and let the kernel handle it), or return an error (indicating that reliably O_APPEND is not available).
    fn open(&mut self, req: &Request<'_>, ino: u64, flags: i32, reply: ReplyOpen) {
        let res = dbfs_fuse_open(req, ino, flags);
        match res {
            Ok(_) => {
                let open_flags = if self.direct_io { FOPEN_DIRECT_IO } else { 0 };
                reply.opened(0, open_flags);
            }
            Err(_) => reply.error(res.err().unwrap()),
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
    fn release(
        &mut self,
        _req: &Request<'_>,
        _ino: u64,
        _fh: u64,
        _flags: i32,
        _lock_owner: Option<u64>,
        _flush: bool,
        reply: ReplyEmpty,
    ) {
        error!("release not implemented");
        reply.ok();
    }

    ///Synchronize file contents
    ///
    /// If the datasync parameter is non-zero, then only the user data should be flushed, not the meta data.
    fn fsync(
        &mut self,
        _req: &Request<'_>,
        _ino: u64,
        _fh: u64,
        _datasync: bool,
        _reply: ReplyEmpty,
    ) {
        error!("fsync not implemented");
    }

    /// Open directory
    ///
    /// Unless the 'default_permissions' mount option is given, this method should check if opendir is permitted for this directory.
    /// Optionally opendir may also return an arbitrary filehandle in the fuse_file_info structure, which will be passed to readdir,
    /// releasedir and fsyncdir.
    fn opendir(&mut self, req: &Request<'_>, ino: u64, flags: i32, reply: ReplyOpen) {
        let res = dbfs_fuse_opendir(req, ino, flags);
        match res {
            Ok(_) => {
                let open_flags = if self.direct_io { FOPEN_DIRECT_IO } else { 0 };
                reply.opened(0, open_flags);
            }
            Err(_) => {
                panic!("opendir error");
                reply.error(res.err().unwrap() as i32)
            }
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

    /// Release directory
    ///
    /// If the directory has been removed after the call to opendir, the path parameter will be NULL.
    fn releasedir(&mut self, _req: &Request<'_>, _ino: u64, _fh: u64, _flags: i32, reply: ReplyEmpty) {
        warn!("releasedir always ok");
        reply.ok()
    }


    /// Get file system statistics
    //
    // The 'f_favail', 'f_fsid' and 'f_flag' fields are ignored
    fn statfs(&mut self, _req: &Request<'_>, _ino: u64, reply: ReplyStatfs) {
        let res = dbfs_fuse_statfs();
        match res {
            Ok(stat) => {
                reply.statfs(
                    stat.f_blocks,         // total blocks
                    stat.f_bfree,          // free blocks
                    stat.f_bavail,         // available blocks
                    stat.f_files,          // total inodes
                    stat.f_ffree,          // free inodes
                    stat.f_bsize as u32,   // block size
                    stat.f_namemax as u32, // name length
                    stat.f_frsize as u32,  // fragment size
                );
            }
            Err(_) => reply.error(ENOENT),
        }
    }
    fn access(&mut self, req: &Request<'_>, ino: u64, mask: i32, reply: ReplyEmpty) {
        let res = dbfs_fuse_access(req,ino,mask);
        match res {
            Ok(bool) => {
                if bool {
                    reply.ok();
                } else {
                    reply.error(ENOENT);
                }
            }
            Err(x) => reply.error(x as i32),
        }
    }

    /// Create and open a file
    ///
    /// If the file does not exist, first create it with the specified mode, and then open it.
    ///
    /// If this method is not implemented or under Linux kernel versions earlier than 2.6.15, the mknod() and open() methods will be called instead.
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
}
