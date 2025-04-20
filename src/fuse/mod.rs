pub mod attr;
pub mod file;
pub mod inode;
pub mod link;
pub mod mkfs;
pub mod sblk;

extern crate std;

use alloc::{sync::Arc, vec};
use std::{alloc::Layout, ffi::OsStr, time::Duration};

use downcast::_std::{path::Path, time::SystemTime};
use fuser::{
    consts::FOPEN_DIRECT_IO, fuse_forget_one, FileAttr, Filesystem, KernelConfig, ReplyAttr,
    ReplyCreate, ReplyData, ReplyDirectory, ReplyDirectoryPlus, ReplyEmpty, ReplyEntry, ReplyOpen,
    ReplyStatfs, ReplyWrite, ReplyXattr, Request, TimeOrNow,
};
use jammdb::DB;
use libc::{c_int, ENOENT};
use log::{error, info, trace};
pub use mkfs::init_dbfs_fuse;

use crate::{
    common::DbfsTimeSpec,
    fs_type::dbfs_common_root_inode,
    fuse::{
        attr::{
            dbfs_fuse_access, dbfs_fuse_chmod, dbfs_fuse_chown, dbfs_fuse_getattr,
            dbfs_fuse_getxattr, dbfs_fuse_listxattr, dbfs_fuse_removexattr, dbfs_fuse_setxattr,
            dbfs_fuse_statfs, dbfs_fuse_utimens,
        },
        file::{
            dbfs_fuse_copy_file_range, dbfs_fuse_open, dbfs_fuse_opendir, dbfs_fuse_read,
            dbfs_fuse_readdir, dbfs_fuse_readdirplus, dbfs_fuse_releasedir, dbfs_fuse_write,
        },
        inode::{
            dbfs_fuse_create, dbfs_fuse_fallocate, dbfs_fuse_lookup, dbfs_fuse_mkdir,
            dbfs_fuse_mknod, dbfs_fuse_rename, dbfs_fuse_rmdir, dbfs_fuse_truncate,
        },
        link::{dbfs_fuse_link, dbfs_fuse_readlink, dbfs_fuse_symlink, dbfs_fuse_unlink},
        mkfs::{init_db, FakeMMap, FakePath, MyOpenOptions},
        sblk::dbfs_fuse_destroy,
    },
    init_cache, init_dbfs, BUDDY_ALLOCATOR,
};

const TTL: Duration = Duration::from_secs(1); // 1 second
                                              // const FILE_SIZE: u64 = 1024 * 1024 * 1024; // 1 GiB
                                              // const FILE_SIZE: u64 = 9999999999999999;
const FILE_SIZE: usize = 1024 * 1024 * 1024 * 20; // 6GB

pub struct DbfsFuse {
    direct_io: bool,
    _suid_support: bool,
}

impl DbfsFuse {
    pub fn new(direct_io: bool, _suid_support: bool) -> Self {
        {
            Self {
                direct_io,
                _suid_support: false,
            }
        }
    }
}

impl Filesystem for DbfsFuse {
    fn init(&mut self, _req: &Request<'_>, _config: &mut KernelConfig) -> Result<(), c_int> {
        let path = "./my-database.db";
        let db =
            DB::open::<MyOpenOptions<FILE_SIZE>, FakePath>(Arc::new(FakeMMap), FakePath::new(path))
                .map_err(|_| -1)?; // TODO: error handling
        init_db(&db, FILE_SIZE as u64);
        init_dbfs(db);
        init_cache();
        let uid = unsafe { libc::getuid() };
        let gid = unsafe { libc::getgid() };
        let time = DbfsTimeSpec::from(SystemTime::now());
        dbfs_common_root_inode(uid, gid, time).map_err(|_| -1)?;
        Ok(())
    }
    /// Clean up filesystem
    fn destroy(&mut self) {
        // we need write back the metadata
        // 1. continue_number to super_block
        dbfs_fuse_destroy();
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
            Err(x) => {
                reply.error(x as i32);
            }
        }
    }
    fn forget(&mut self, _req: &Request<'_>, _ino: u64, _nlookup: u64) {
        info!("forget");
    }

    fn batch_forget(&mut self, _req: &Request<'_>, nodes: &[fuse_forget_one]) {
        for node in nodes {
            trace!("batch_forget: {}", node.nodeid);
        }
    }
    fn getattr(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        fh: Option<u64>, // 新增参数
        reply: ReplyAttr,
    ) {
        // 如果不需要使用 `fh` 参数，可以直接忽略它：
        let res = dbfs_fuse_getattr(ino);
        match res {
            Ok(attr) => reply.attr(&TTL, &attr),
            Err(x) => {
                reply.error(x as i32);
            }
        }
    }
    /// It include truncate/chown/chmod/utimens function
    fn setattr(
        &mut self,
        req: &Request<'_>,
        ino: u64,
        mode: Option<u32>,
        uid: Option<u32>,
        gid: Option<u32>,
        size: Option<u64>,
        atime: Option<TimeOrNow>,
        mtime: Option<TimeOrNow>,
        _ctime: Option<SystemTime>,
        _fh: Option<u64>,
        _crtime: Option<SystemTime>,
        _chgtime: Option<SystemTime>,
        _bkuptime: Option<SystemTime>,
        _flags: Option<u32>,
        reply: ReplyAttr,
    ) {
        if let Some(mode) = mode {
            let res = dbfs_fuse_chmod(req, ino, mode);
            match res {
                Ok(attr) => reply.attr(&TTL, &attr.into()),
                Err(x) => reply.error(x as i32),
            }
            return;
        }
        if uid.is_some() || gid.is_some() {
            let res = dbfs_fuse_chown(req, ino, uid, gid);
            match res {
                Ok(attr) => reply.attr(&TTL, &attr.into()),
                Err(x) => reply.error(x as i32),
            }
            return;
        }
        if let Some(size) = size {
            let res = dbfs_fuse_truncate(req, ino, size);
            match res {
                Ok(attr) => reply.attr(&TTL, &attr.into()),
                Err(x) => reply.error(x as i32),
            }
            return;
        }

        if atime.is_some() || mtime.is_some() {
            let res = dbfs_fuse_utimens(req, ino, atime, mtime);
            match res {
                Ok(attr) => {
                    let attr: FileAttr = attr.into();
                    reply.attr(&TTL, &attr)
                }
                Err(x) => reply.error(x as i32),
            }
            return;
        }
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

    /// Make a special (device) file, FIFO, or socket. See mknod(2) for details.
    /// This function is rarely needed, since it's uncommon to make these objects inside special-purpose filesystems.
    fn mknod(
        &mut self,
        req: &Request<'_>,
        parent: u64,
        name: &OsStr,
        mode: u32,
        _umask: u32,
        rdev: u32,
        reply: ReplyEntry,
    ) {
        let res = dbfs_fuse_mknod(req, parent, name.to_str().unwrap(), mode, rdev);
        match res {
            Ok(attr) => reply.entry(&TTL, &attr.into(), 0),
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
            Err(x) => {
                // panic!("unlink panic");
                reply.error(x as i32)
            }
        }
    }
    /// Remove the given directory. This should succeed only if the directory is empty (except for "." and "..").
    fn rmdir(&mut self, req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEmpty) {
        let res = dbfs_fuse_rmdir(req, parent, name.to_str().unwrap());
        match res {
            Ok(_) => reply.ok(),
            Err(x) => reply.error(x as i32),
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
            Err(x) => reply.error(x as i32),
        }
    }

    /// Rename a file
    //
    // flags may be RENAME_EXCHANGE or RENAME_NOREPLACE.
    // If RENAME_NOREPLACE is specified, the filesystem must not overwrite newname if it exists
    // and return an error instead. If RENAME_EXCHANGE is specified, the filesystem must
    // atomically exchange the two files, i.e. both must exist and neither may be deleted.
    fn rename(
        &mut self,
        req: &Request<'_>,
        parent: u64,
        name: &OsStr,
        newparent: u64,
        newname: &OsStr,
        flags: u32,
        reply: ReplyEmpty,
    ) {
        let res = dbfs_fuse_rename(
            req,
            parent,
            name.to_str().unwrap(),
            newparent,
            newname.to_str().unwrap(),
            flags,
        );
        match res {
            Ok(_) => reply.ok(),
            Err(x) => reply.error(x as i32),
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
            Err(e) => {
                error!("link error: {:?}", e);
                reply.error(e as i32)
            }
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
        let _data = vec![0u8; size as usize];
        let ptr = BUDDY_ALLOCATOR
            .lock()
            .alloc(Layout::from_size_align(size as usize, 8).unwrap())
            .unwrap();
        let data =
            unsafe { std::slice::from_raw_parts_mut(ptr.as_ptr() as *mut u8, size as usize) };
        let res = dbfs_fuse_read(ino, offset, data);
        match res {
            Ok(x) => reply.data(&data[..x]),
            Err(_) => reply.error(ENOENT),
        }
        BUDDY_ALLOCATOR
            .lock()
            .dealloc(ptr, Layout::from_size_align(size as usize, 8).unwrap());
        // dbfs_fuse_special_read(ino as usize, offset, size as usize, reply).unwrap();
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
            Ok(x) => reply.written(x as u32),
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
        reply: ReplyEmpty,
    ) {
        reply.ok();
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
            Err(x) => reply.error(x as i32),
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

    fn readdirplus(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        _fh: u64,
        offset: i64,
        reply: ReplyDirectoryPlus,
    ) {
        dbfs_fuse_readdirplus(ino, offset, reply)
    }

    /// Release directory
    ///
    /// If the directory has been removed after the call to opendir, the path parameter will be NULL.
    fn releasedir(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        _fh: u64,
        _flags: i32,
        reply: ReplyEmpty,
    ) {
        dbfs_fuse_releasedir(ino).unwrap();
        reply.ok()
    }
    fn fsyncdir(
        &mut self,
        _req: &Request<'_>,
        _ino: u64,
        _fh: u64,
        _datasync: bool,
        reply: ReplyEmpty,
    ) {
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
    /// Set extended attributes
    fn setxattr(
        &mut self,
        req: &Request<'_>,
        ino: u64,
        name: &OsStr,
        value: &[u8],
        flags: i32,
        position: u32,
        reply: ReplyEmpty,
    ) {
        let res = dbfs_fuse_setxattr(req, ino, name.to_str().unwrap(), value, flags, position);
        match res {
            Ok(_) => reply.ok(),
            Err(x) => reply.error(x as i32),
        }
    }

    /// Get extended attributes
    fn getxattr(
        &mut self,
        req: &Request<'_>,
        ino: u64,
        name: &OsStr,
        size: u32,
        reply: ReplyXattr,
    ) {
        let mut buf = vec![0u8; size as usize];
        let res = dbfs_fuse_getxattr(req, ino, name.to_str().unwrap(), buf.as_mut_slice());
        match res {
            Ok(x) => {
                if size == 0 {
                    reply.size(x as u32);
                } else {
                    reply.data(&buf[..x]);
                }
            }
            Err(x) => reply.error(x as i32),
        }
    }

    /// List extended attributes
    fn listxattr(&mut self, req: &Request<'_>, ino: u64, size: u32, reply: ReplyXattr) {
        let mut buf = vec![0u8; size as usize];
        let res = dbfs_fuse_listxattr(req, ino, buf.as_mut_slice());
        match res {
            Ok(x) => {
                if size == 0 {
                    reply.size(x as u32);
                } else {
                    reply.data(&buf);
                }
            }
            Err(x) => reply.error(x as i32),
        }
    }
    /// Remove extended attributes
    fn removexattr(&mut self, req: &Request<'_>, ino: u64, name: &OsStr, reply: ReplyEmpty) {
        let res = dbfs_fuse_removexattr(req, ino, name.to_str().unwrap());
        match res {
            Ok(_) => reply.ok(),
            Err(x) => reply.error(x as i32),
        }
    }

    fn access(&mut self, req: &Request<'_>, ino: u64, mask: i32, reply: ReplyEmpty) {
        let res = dbfs_fuse_access(req, ino, mask);
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
    // fn bmap(&mut self, _req: &Request<'_>, _ino: u64, _blocksize: u32, _idx: u64, reply: ReplyBmap) {
    //     todo!()
    // }
    // fn getlk(&mut self, _req: &Request<'_>, _ino: u64, _fh: u64, _lock_owner: u64, _start: u64, _end: u64, _typ: i32, _pid: u32, reply: ReplyLock) {
    //     todo!()
    // }
    // fn setlk(&mut self, _req: &Request<'_>, _ino: u64, _fh: u64, _lock_owner: u64, _start: u64, _end: u64, _typ: i32, _pid: u32, _sleep: bool, reply: ReplyEmpty) {
    //     todo!()
    // }

    // macos
    // fn exchange(&mut self, _req: &Request<'_>, _parent: u64, _name: &OsStr, _newparent: u64, _newname: &OsStr, _options: u64, reply: ReplyEmpty) {
    //
    // }

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
            Err(x) => reply.error(x as i32),
        }
    }

    // macos
    // fn getxtimes(&mut self, _req: &Request<'_>, _ino: u64, reply: ReplyXTimes) {
    //
    // }

    // fn ioctl(
    //     &mut self,
    //     _req: &Request<'_>,
    //     _ino: u64,
    //     _fh: u64,
    //     _flags: u32,
    //     _cmd: u32,
    //     _in_data: &[u8],
    //     _out_size: u32,
    //     reply: ReplyIoctl,
    // ) {
    //     todo!()
    // }

    // fn lseek(&mut self, _req: &Request<'_>, _ino: u64, _fh: u64, _offset: i64, _whence: i32, reply: ReplyLseek) {
    //     todo!()
    // }

    // macos
    // fn setvolname(&mut self, _req: &Request<'_>, _name: &OsStr, reply: ReplyEmpty) {
    //
    // }

    /// Allocates space for an open file
    ///
    /// This function ensures that required space is allocated for specified file.
    /// If this function returns success then any subsequent write request to specified range
    /// is guaranteed not to fail because of lack of space on the file system media.
    fn fallocate(
        &mut self,
        req: &Request<'_>,
        ino: u64,
        _fh: u64,
        offset: i64,
        length: i64,
        mode: i32,
        reply: ReplyEmpty,
    ) {
        let res = dbfs_fuse_fallocate(req, ino, offset as u64, length as u64, mode as u32);
        match res {
            Ok(_) => reply.ok(),
            Err(x) => reply.error(x as i32),
        }
    }

    /// Copy a range of data from one file to another
    /// Performs an optimized copy between two file descriptors without the additional cost of transferring data through the FUSE kernel module to user space (glibc) and then back into the FUSE filesystem again.
    /// In case this method is not implemented, applications are expected to fall back to a regular file copy.
    /// (Some glibc versions did this emulation automatically, but the emulation has been removed from all glibc release branches.)
    fn copy_file_range(
        &mut self,
        req: &Request<'_>,
        ino_in: u64,
        _fh_in: u64,
        offset_in: i64,
        ino_out: u64,
        _fh_out: u64,
        offset_out: i64,
        len: u64,
        _flags: u32,
        reply: ReplyWrite,
    ) {
        let res = dbfs_fuse_copy_file_range(
            req,
            ino_in,
            offset_in as u64,
            ino_out,
            offset_out as u64,
            len,
        );
        match res {
            Ok(x) => reply.written(x as u32),
            Err(x) => reply.error(x as i32),
        }
    }
}
