use crate::common::{DbfsDirEntry, DbfsError, DbfsResult, FMODE_EXEC};
use crate::file::{dbfs_common_open, dbfs_common_read, dbfs_common_readdir, dbfs_common_write};
use alloc::vec;
use fuser::{ReplyDirectory, Request};
use log::error;

use rvfs::warn;

pub fn dbfs_fuse_read(ino: u64, offset: i64, buf: &mut [u8]) -> Result<usize, ()> {
    assert!(offset >= 0);
    dbfs_common_read(ino as usize, buf, offset as u64).map_err(|_| ())
}

pub fn dbfs_fuse_write(ino: u64, offset: i64, buf: &[u8]) -> Result<usize, ()> {
    assert!(offset >= 0);
    dbfs_common_write(ino as usize, buf, offset as u64).map_err(|_| ())
}

pub fn dbfs_fuse_readdir(ino: u64, mut offset: i64, mut repl: ReplyDirectory) {
    warn!("dbfs_fuse_readdir(ino:{},offset:{})", ino, offset);
    assert!(offset >= 0);
    let mut entries = vec![DbfsDirEntry::default(); 16]; // we read 16 entries at a time
    loop {
        let res = dbfs_common_readdir(ino as usize, &mut entries, offset as u64);
        if res.is_err() {
            repl.error(libc::ENOENT);
            return;
        }
        let res = res.unwrap();
        if res == 0 {
            repl.ok();
            return;
        }
        for i in 0..res {
            let x = &entries[i];
            if repl.add(x.ino, x.offset as i64 + 1, x.kind.into(), x.name.as_str()) {
                // buf full
                repl.ok();
                return;
            }
        }
        offset += res as i64;
    }
}

pub fn dbfs_fuse_open(req: &Request<'_>, ino: u64, flags: i32) -> Result<(), i32> {
    warn!("dbfs_fuse_open(ino:{},flag:{})", ino, flags);
    let (access_mask, _read, _write) = match flags & libc::O_ACCMODE {
        libc::O_RDONLY => {
            // Behavior is undefined, but most filesystems return EACCES
            if flags & libc::O_TRUNC != 0 {
                return Err(libc::EACCES);
            }
            if flags & FMODE_EXEC != 0 {
                // Open is from internal exec syscall
                (libc::X_OK, true, false)
            } else {
                (libc::R_OK, true, false)
            }
        }
        libc::O_WRONLY => (libc::W_OK, false, true),
        libc::O_RDWR => (libc::R_OK | libc::W_OK, true, true),
        // Exactly one access mode flag must be specified
        _ => {
            return Err(libc::EINVAL);
        }
    };

    // checkout the permission
    dbfs_common_open(ino as usize, req.uid(), req.gid(), access_mask as u16)
        .map_err(|x| x as i32)?;

    Ok(())
}

pub fn dbfs_fuse_opendir(req: &Request<'_>, ino: u64, flags: i32) -> DbfsResult<()> {
    error!("dbfs_fuse_opendir(ino:{},flag:{})", ino, flags);
    let (access_mask, _read, _write) = match flags & libc::O_ACCMODE {
        libc::O_RDONLY => {
            // Behavior is undefined, but most filesystems return EACCES
            if flags & libc::O_TRUNC != 0 {
                return Err(DbfsError::AccessError);
            }
            (libc::R_OK, true, false)
        }
        libc::O_WRONLY => (libc::W_OK, false, true),
        libc::O_RDWR => (libc::R_OK | libc::W_OK, true, true),
        // Exactly one access mode flag must be specified
        _ => return Err(DbfsError::InvalidArgument),
    };

    // checkout the permission
    dbfs_common_open(ino as usize, req.uid(), req.gid(), access_mask as u16)
}
