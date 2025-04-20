use alloc::vec;
use std::{cmp::min, io::IoSlice, println};

use downcast::_std::time::SystemTime;
use fuser::{ReplyData, ReplyDirectory, ReplyDirectoryPlus, Request};
use log::error;
use rvfs::warn;
use smallvec::{smallvec, SmallVec};

use crate::{
    clone_db,
    common::{
        generate_data_key_with_number, pop_readdir_table, push_readdir_table, DbfsDirEntry,
        DbfsError, DbfsResult, DbfsTimeSpec, ReadDirInfo, FMODE_EXEC,
    },
    file::{
        dbfs_common_copy_file_range, dbfs_common_open, dbfs_common_read, dbfs_common_readdir,
        dbfs_common_write,
    },
    fuse::TTL,
    usize, SLICE_SIZE,
};

pub fn dbfs_fuse_read(ino: u64, offset: i64, buf: &mut [u8]) -> DbfsResult<usize> {
    assert!(offset >= 0);
    dbfs_common_read(ino as usize, buf, offset as u64)
}

//
pub fn dbfs_fuse_special_read(
    ino: usize,
    old_offset: i64,
    need_size: usize,
    _repl: ReplyData,
) -> DbfsResult<usize> {
    assert!(old_offset >= 0);
    let offset = old_offset as u64;
    let db = clone_db();
    let tx = db.tx(false)?;
    let bucket = tx.get_bucket(ino.to_be_bytes())?;
    let size = bucket.get_kv("size").unwrap();
    let size = usize!(size.value());
    if offset >= size as u64 {
        return Ok(0);
    }
    let mut res_slice: SmallVec<[IoSlice<'_>; 1024 * 1024 / SLICE_SIZE]> = smallvec![];

    let tmp = [0u8; SLICE_SIZE];
    let mut start_num = offset / SLICE_SIZE as u64;
    let mut offset = offset % SLICE_SIZE as u64;

    let old_start = start_num;
    let mut count = 0;
    loop {
        let key = generate_data_key_with_number(start_num as u32);
        let value = bucket.get_kv(key);
        let real_size = min(size - start_num as usize * SLICE_SIZE, SLICE_SIZE);
        if value.is_none() {
            // copy tmp buf to buf
            let len = min(need_size - count, real_size.saturating_sub(offset as usize));
            let ptr = tmp.as_ptr();
            let data = unsafe { std::slice::from_raw_parts(ptr, SLICE_SIZE) };
            res_slice.push(IoSlice::new(&data[offset as usize..offset as usize + len]));

            count += len;
            offset = (offset + len as u64) % SLICE_SIZE as u64;
        } else {
            let value = value.unwrap();
            let value = value.value();
            let len = min(need_size - count, real_size.saturating_sub(offset as usize));
            let ptr = value.as_ptr();
            let data = unsafe { std::slice::from_raw_parts(ptr, SLICE_SIZE) };
            res_slice.push(IoSlice::new(&data[offset as usize..offset as usize + len]));

            count += len;
            offset = (offset + len as u64) % SLICE_SIZE as u64;
        }
        if count == size || count == need_size {
            break;
        }
        start_num += 1;
    }
    error!("IoSlice len :{}", res_slice.len());
    if count != need_size {
        for _i in 0..(need_size - count) / SLICE_SIZE {
            let ptr = tmp.as_ptr();
            let data = unsafe { std::slice::from_raw_parts(ptr, SLICE_SIZE) };
            res_slice.push(IoSlice::new(data));
        }
        let len = (need_size - count) % SLICE_SIZE;
        let ptr = tmp.as_ptr();
        let data = unsafe { std::slice::from_raw_parts(ptr, SLICE_SIZE) };
        res_slice.push(IoSlice::new(&data[..len]));
    }

    let total = res_slice.iter().fold(0, |acc, x| acc + x.len());
    println!(
        "read_num:{},offset:{},count:{},need:{},total:{}",
        start_num - old_start,
        old_offset,
        count,
        need_size,
        total
    );

    // repl.data2(&res_slice);
    Ok(count)
}

pub fn dbfs_fuse_write(ino: u64, offset: i64, buf: &[u8]) -> DbfsResult<usize> {
    assert!(offset >= 0);
    let res = dbfs_common_write(ino as usize, buf, offset as u64);
    error!("dbfs write res:{:?}", res);
    res
}

pub fn dbfs_fuse_releasedir(ino: u64) -> DbfsResult<()> {
    pop_readdir_table(ino as usize);
    Ok(())
}

pub fn dbfs_fuse_readdir(ino: u64, mut offset: i64, mut repl: ReplyDirectory) {
    warn!("dbfs_fuse_readdir(ino:{},offset:{})", ino, offset);
    assert!(offset >= 0);
    let mut entries = vec![DbfsDirEntry::default(); 16]; // we read 16 entries at a time
    loop {
        let res = dbfs_common_readdir(ino as usize, &mut entries, offset as u64, false);
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
                // TODO! update GLOBAL_READDIR_TABLE
                push_readdir_table(
                    ino as usize,
                    ReadDirInfo::new(x.offset as usize, x.name.clone()),
                );
                warn!(
                    "push_readdir_table(ino:{},offset:{},name:{})",
                    ino, x.offset, x.name
                );
                return;
            }
            offset = x.offset as i64 + 1;
        }
        let x = &entries[res - 1];
        push_readdir_table(
            ino as usize,
            ReadDirInfo::new(x.offset as usize, x.name.clone()),
        );
    }
}

pub fn dbfs_fuse_readdirplus(ino: u64, mut offset: i64, mut repl: ReplyDirectoryPlus) {
    // panic!("dbfs_fuse_readdirplus(ino:{},offset:{})",ino,offset);
    assert!(offset >= 0);
    let mut entries = vec![DbfsDirEntry::default(); 16]; // we read 16 entries at a time
    loop {
        let res = dbfs_common_readdir(ino as usize, &mut entries, offset as u64, true);
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
            let attr = x.attr.as_ref().unwrap();
            if repl.add(
                x.ino,
                x.offset as i64 + 1,
                x.name.as_str(),
                &TTL,
                &attr.into(),
                0,
            ) {
                // buf full
                repl.ok();

                // TODO! update GLOBAL_READDIR_TABLE
                push_readdir_table(
                    ino as usize,
                    ReadDirInfo::new(x.offset as usize, x.name.clone()),
                );
                return;
            }
            offset = x.offset as i64 + 1;
        }
        let x = &entries[res - 1];
        push_readdir_table(
            ino as usize,
            ReadDirInfo::new(x.offset as usize, x.name.clone()),
        );
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

pub fn dbfs_fuse_copy_file_range(
    req: &Request<'_>,
    ino_in: u64,
    offset_in: u64,
    ino_out: u64,
    offset_out: u64,
    len: u64,
) -> DbfsResult<usize> {
    warn!(
        "dbfs_fuse_copy_file_range(ino_in:{},offset_in:{},ino_out:{},offset_out:{},len:{})",
        ino_in, offset_in, ino_out, offset_out, len
    );
    let time = DbfsTimeSpec::from(SystemTime::now());
    let uid = req.uid();
    let gid = req.gid();
    dbfs_common_copy_file_range(
        uid,
        gid,
        ino_in as usize,
        offset_in as usize,
        ino_out as usize,
        offset_out as usize,
        len as usize,
        time,
    )
}
