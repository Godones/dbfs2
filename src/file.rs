use alloc::{format, string::ToString, sync::Arc, vec, vec::Vec};
use core::{
    alloc::Layout,
    cmp::{max, min},
    ops::Range,
    ptr::NonNull,
    sync::atomic::AtomicBool,
};

use jammdb::Data;
use log::{error, trace, warn};
use rvfs::{
    dentry::{Dirent64, DirentType},
    file::{File, FileOps},
    StrResult,
};

use crate::{
    clone_db,
    common::{
        generate_data_key_with_number, get_readdir_table, pop_readdir_table, push_readdir_table,
        DbfsDirEntry, DbfsError, DbfsFileType, DbfsPermission, DbfsResult, DbfsTimeSpec,
        ReadDirInfo,
    },
    copy_data,
    inode::{checkout_access, dbfs_common_attr},
    u16, u32, usize, BUDDY_ALLOCATOR, SLICE_SIZE,
};

pub const DBFS_DIR_FILE_OPS: FileOps = {
    let mut ops = FileOps::empty();
    ops.readdir = dbfs_readdir;
    ops.open = |_| Ok(());
    ops
};
pub const DBFS_FILE_FILE_OPS: FileOps = {
    let mut ops = FileOps::empty();
    ops.write = dbfs_file_write;
    ops.read = dbfs_file_read;
    ops.open = |_| Ok(());
    ops
};
pub const DBFS_SYMLINK_FILE_OPS: FileOps = {
    let mut ops = FileOps::empty();
    ops.open = |_| Ok(());
    ops
};

fn dbfs_file_write(file: Arc<File>, buf: &[u8], offset: u64) -> StrResult<usize> {
    warn!(
        "dbfs_file_write ino: {}, offset: {}, buf.len: {}, slice_size:{}",
        file.f_dentry.access_inner().d_inode.number,
        offset,
        buf.len(),
        SLICE_SIZE
    );
    let dentry = file.f_dentry.clone();
    let inode = dentry.access_inner().d_inode.clone();
    let numer = inode.number;
    dbfs_common_write(numer, buf, offset).map_err(|_| "dbfs_common_write error")
}
fn dbfs_file_read(file: Arc<File>, buf: &mut [u8], offset: u64) -> StrResult<usize> {
    let dentry = file.f_dentry.clone();
    let inode = dentry.access_inner().d_inode.clone();
    let numer = inode.number;
    dbfs_common_read(numer, buf, offset).map_err(|_| "dbfs_common_read error")
}

/// the file data in dbfs is stored as a set of key-value pairs
/// * data1: \[u8;SLICE_SIZE]
/// * data2: \[u8;SLICE_SIZE]
/// * ....
/// * datai: \[u8;SLICE_SIZE]
pub fn dbfs_common_read(number: usize, buf: &mut [u8], offset: u64) -> DbfsResult<usize> {
    warn!(
        "dbfs_common_read ino: {}, offset: {}, buf.len: {}, slice_size:{}",
        number,
        offset,
        buf.len(),
        SLICE_SIZE
    );
    let db = clone_db();
    let tx = db.tx(false)?;
    let bucket = tx.get_bucket(number.to_be_bytes())?;
    let size = bucket.get_kv("size").unwrap();
    let size = usize!(size.value());
    if offset >= size as u64 {
        return Ok(0);
    }
    // TODO! second version
    let tmp = [0u8; SLICE_SIZE];
    let mut start_num = offset / SLICE_SIZE as u64;
    // let mut offset = offset % SLICE_SIZE as u64;
    // let mut count = 0;
    // loop {
    //     let key = generate_data_key_with_number(start_num as u32);
    //     let value = bucket.get_kv(key);
    //     let real_size = min(size - start_num as usize * SLICE_SIZE, SLICE_SIZE);
    //     if value.is_none(){
    //         // copy tmp buf to buf
    //         let len = min(buf.len() - count, real_size.saturating_sub(offset as usize));
    //         buf[count..count + len].copy_from_slice(&tmp[offset as usize..offset as usize + len]);
    //         count += len;
    //         offset  = (offset + len as u64) % SLICE_SIZE as u64;
    //     }else {
    //         let value = value.unwrap();
    //         let value = value.value();
    //         let len = min(buf.len() - count, real_size.saturating_sub(offset as usize));
    //         buf[count..count + len].copy_from_slice(&value[offset as usize..offset as usize + len]);
    //         count += len;
    //         offset  = (offset + len as u64) % SLICE_SIZE as u64;
    //     }
    //     if count == buf.len() || count == size {
    //         break;
    //     }
    //     start_num += 1;
    // }

    // TODO! first version
    let end_num = (offset + buf.len() as u64) / SLICE_SIZE as u64 + 1;
    let offset = offset % SLICE_SIZE as u64;
    let mut buf_offset = 0;

    let f_end_num = size / SLICE_SIZE + 1;

    let end_num = min(end_num, f_end_num as u64);

    warn!(
        "start_num: {:?}, end_num: {:?}, file_end_num:{:?}",
        start_num, end_num, f_end_num
    );

    let start_key = generate_data_key_with_number(start_num as u32);
    let end_key = generate_data_key_with_number(end_num as u32);

    let range = Range {
        start: start_key.as_slice(),
        end: end_key.as_slice(),
    };
    let iter = bucket.range(range);
    for data in iter {
        match data {
            Data::Bucket(_) => {
                panic!("bucket in bucket")
            }
            Data::KeyValue(kv) => {
                let value = kv.value();
                let key = kv.key();

                let index = key.splitn(2, |c| *c == b':').nth(1).unwrap();
                let index = u32!(index);
                // debug!("key: {:?}", index);
                if index as u64 != start_num {
                    for i in start_num as u32..index {
                        let current_size = i as usize * SLICE_SIZE; // offset = 1000 ,current_size >= SLICE_SIZE,1024 => offset= 1000 - SLICE_SIZE = 488
                        let value_offset = offset.saturating_sub(current_size as u64) as usize; // 一定位于(0,SLICE_SIZE)范围
                        let real_size = min(size - current_size, SLICE_SIZE);
                        let len = min(
                            buf.len() - buf_offset,
                            real_size.saturating_sub(value_offset),
                        );
                        buf[buf_offset..buf_offset + len]
                            .copy_from_slice(&tmp[value_offset..value_offset + len]);
                        buf_offset += len;
                    }
                    start_num = index as u64 + 1;
                }
                let current_size = index as usize * SLICE_SIZE; // offset = 1000 ,current_size >= SLICE_SIZE,1024 => offset= 1000 - SLICE_SIZE = 488
                let value_offset = offset.saturating_sub(current_size as u64) as usize; // 一定位于(0,SLICE_SIZE)范围
                let real_size = min(size - current_size, SLICE_SIZE);
                let len = min(
                    buf.len() - buf_offset,
                    real_size.saturating_sub(value_offset),
                );
                buf[buf_offset..buf_offset + len]
                    .copy_from_slice(&value[value_offset..value_offset + len]);
                buf_offset += len;
                start_num += 1;
                warn!("read len: {}", len);
            }
        }
        if buf_offset == buf.len() {
            break;
        }
    }
    Ok(buf_offset)

    // Ok(count)
}
#[cfg(feature = "fuse")]
pub static FLAG: AtomicBool = AtomicBool::new(false);
/// we need think about how to write data to dbfs
/// * data1: \[u8;SLICE_SIZE]
/// * data2: \[u8;SLICE_SIZE]
/// * ....
/// * datai: \[u8;SLICE_SIZE]
/// the i should be u32, because we can store 2^32 * SLICE_SIZE bytes in dbfs, == 2048 GB
/// u32 == 4 bytes, 0x00000000 - 0xffffffff
pub fn dbfs_common_write(number: usize, buf: &[u8], offset: u64) -> DbfsResult<usize> {
    warn!(
        "dbfs_common_write ino: {}, offset: {}, buf.len: {}",
        number,
        offset,
        buf.len()
    );
    let db = clone_db();
    let tx = db.tx(true)?;
    let bucket = tx.get_bucket(number.to_be_bytes())?;
    let size = bucket.get_kv("size").unwrap();
    let size = usize!(size.value());
    let o_offset = offset;
    let mut num = offset / SLICE_SIZE as u64;
    let mut offset = offset % SLICE_SIZE as u64;
    let mut count = 0;

    let mut ptrs = vec![];
    loop {
        let key = generate_data_key_with_number(num as u32);
        let len = min(buf.len() - count, SLICE_SIZE - offset as usize);
        let data = if len == SLICE_SIZE && offset == 0 {
            unsafe { buf.as_ptr().add(count) }
        } else {
            #[cfg(feature = "fuse")]
            let start = std::time::SystemTime::now();
            let kv = bucket.get_kv(key.as_slice());
            #[cfg(feature = "fuse")]
            {
                let end = std::time::SystemTime::now();
                let duration = end.duration_since(start).unwrap();
                if FLAG.load(core::sync::atomic::Ordering::SeqCst) {
                    std::println!("get_kv:{} cost {:?}", num, duration);
                }
            }
            if kv.is_none() {
                let ptr = unsafe {
                    let ptr = BUDDY_ALLOCATOR
                        .lock()
                        .alloc(Layout::from_size_align_unchecked(SLICE_SIZE, 8));
                    ptr.unwrap().as_ptr()
                };
                unsafe {
                    copy_data(buf.as_ptr().add(count), ptr.add(offset as usize), len);
                }
                ptrs.push(ptr);
                ptr as *const u8
            } else {
                let value = kv.as_ref().unwrap().value();
                let ptr = unsafe {
                    let ptr = BUDDY_ALLOCATOR
                        .lock()
                        .alloc(Layout::from_size_align_unchecked(SLICE_SIZE, 8));
                    ptr.unwrap().as_ptr()
                };
                unsafe {
                    copy_data(value.as_ptr(), ptr, offset as usize);
                    copy_data(buf.as_ptr().add(count), ptr.add(offset as usize), len);
                    copy_data(
                        value.as_ptr().add(offset as usize + len),
                        ptr.add(offset as usize + len),
                        SLICE_SIZE - offset as usize - len,
                    );
                }
                ptrs.push(ptr);
                ptr as *const u8
            }
        };

        let data = unsafe { core::slice::from_raw_parts(data, SLICE_SIZE) };

        bucket.put(key, data)?;
        count += len;
        offset = (offset + len as u64) % SLICE_SIZE as u64;
        num += 1;
        if count == buf.len() {
            break;
        }
    }

    let new_size = max(size, (o_offset as usize + count) as usize);
    if new_size > size {
        bucket.put("size", new_size.to_be_bytes())?;
    }
    tx.commit()?;
    ptrs.into_iter().for_each(|ptr| unsafe {
        BUDDY_ALLOCATOR.lock().dealloc(
            NonNull::new(ptr).unwrap(),
            Layout::from_size_align_unchecked(SLICE_SIZE, 8),
        )
    });
    Ok(count)
}

fn dbfs_readdir(file: Arc<File>, dirents: &mut [u8]) -> StrResult<usize> {
    let dentry = file.f_dentry.clone();
    let inode = dentry.access_inner().d_inode.clone();
    let numer = inode.number;
    let db = clone_db();
    let tx = db.tx(false).unwrap();
    let bucket = tx.get_bucket(numer.to_be_bytes()).unwrap();

    let res: usize = if dirents.is_empty() {
        bucket
            .kv_pairs()
            .map(|x| {
                if x.key().starts_with("data:".as_bytes()) {
                    let key = x.key();
                    let str = core::str::from_utf8(key).unwrap();
                    let name = str.rsplitn(2, ':').collect::<Vec<&str>>();
                    let fake_dirent = Dirent64::new(name[0], 1, 0, DirentType::empty());
                    fake_dirent.len()
                } else {
                    0
                }
            })
            .sum()
    } else {
        pop_readdir_table(numer);
        let mut count = 0;
        let buf_len = dirents.len();
        let mut ptr = dirents.as_mut_ptr();
        let mut offset = 0;
        loop {
            let mut entries = vec![DbfsDirEntry::default(); 16]; // we read 16 entries at a time
            let res = dbfs_common_readdir(numer as usize, &mut entries, offset as u64, false);
            if res.is_err() {
                return Err("dbfs_common_readdir error");
            }
            let res = res.unwrap();
            if res == 0 {
                trace!("There is no entry in the directory.");
                return Ok(count);
            }
            for i in 0..res {
                let x = &entries[i];
                let dirent = Dirent64::new(&x.name, x.ino, x.offset as i64, x.kind.into());
                offset = x.offset as i64 + 1;
                if count + dirent.len() <= buf_len {
                    let dirent_ptr = unsafe { &mut *(ptr as *mut Dirent64) };
                    *dirent_ptr = dirent;
                    let name_ptr = dirent_ptr.name.as_mut_ptr();
                    unsafe {
                        let mut name = x.name.clone();
                        name.push('\0');
                        let len = name.len();
                        name_ptr.copy_from(name.as_ptr(), len);
                        ptr = ptr.add(dirent_ptr.len());
                    }
                    count += dirent_ptr.len();
                } else {
                    return Ok(count); // return
                }
            }
            if res < 16 {
                break;
            }
            let x = &entries[res - 1];
            push_readdir_table(numer, ReadDirInfo::new(x.offset as usize, x.name.clone()));
        }
        count
    };
    Ok(res)
}

pub fn dbfs_common_readdir(
    ino: usize,
    buf: &mut Vec<DbfsDirEntry>,
    offset: u64,
    is_readdir_plus: bool,
) -> DbfsResult<usize> {
    let db = clone_db();
    let tx = db.tx(false)?;
    let bucket = tx.get_bucket(ino.to_be_bytes())?;
    buf.clear();
    let mut count = 0;

    let buf_len = buf.len();

    let mut cursor = bucket.cursor();
    let readdir_info = get_readdir_table(ino);
    if readdir_info.is_some() {
        let info = readdir_info.unwrap();
        let name = info.key;
        let save_offset = info.offset;
        assert_eq!(offset, (save_offset as u64 + 1));
        let key = format!("data:{}", name);
        let res = cursor.seek(key);
        assert_eq!(res, true);
        let val = cursor.next();
        assert!(val.is_some());
    }
    let mut offset = offset;
    cursor.for_each(|x| {
        if let Data::KeyValue(kv) = x {
            let key = kv.key();
            if key.starts_with(b"data:") {
                let key = key.splitn(2, |x| *x == b':').collect::<Vec<&[u8]>>();
                let name = key[1];
                let name = core::str::from_utf8(name).unwrap();
                let value = kv.value();
                let ino = core::str::from_utf8(value).unwrap();
                let inode_number = ino.parse::<usize>().unwrap();
                let mut entry = DbfsDirEntry::default();
                entry.name = name.to_string();
                entry.ino = inode_number as u64;
                entry.offset = offset;

                offset += 1;
                if !is_readdir_plus {
                    let inode = tx.get_bucket(inode_number.to_be_bytes()).unwrap();
                    let mode = inode.get_kv("mode").unwrap();
                    let mode = u16!(mode.value());
                    let perm = DbfsPermission::from_bits_truncate(mode);
                    entry.kind = DbfsFileType::from(perm);
                } else {
                    let attr = dbfs_common_attr(inode_number).unwrap();
                    entry.kind = attr.kind;
                    entry.attr = Some(attr);
                }

                buf.push(entry);
                count += 1;

                if buf.len() == buf_len {
                    return;
                }
            } else {
                return;
            }
        }
    });
    error!(
        "dbfs_common_readdir: offset: {}, count: {}, buf:{:?}",
        offset,
        count,
        &buf[0..count]
    );
    Ok(count)
}

pub fn dbfs_common_open(ino: usize, uid: u32, gid: u32, access_mask: u16) -> Result<(), DbfsError> {
    let attr = dbfs_common_attr(ino as usize).map_err(|_| DbfsError::NotFound)?;
    let bool = checkout_access(attr.uid, attr.gid, attr.perm, uid, gid, access_mask);
    if bool {
        Ok(())
    } else {
        Err(DbfsError::AccessError)
    }
}

pub fn dbfs_common_copy_file_range(
    _uid: u32,
    _gid: u32,
    src: usize,
    offset_src: usize,
    dest: usize,
    offset_dest: usize,
    len: usize,
    ctime: DbfsTimeSpec,
) -> DbfsResult<usize> {
    // now we ignore the uid and gid
    let db = clone_db();
    let src_size = {
        let tx = db.tx(false)?;
        let bucket = tx.get_bucket(src.to_be_bytes())?;
        let size = bucket.get_kv("size").unwrap();
        let size = usize!(size.value());
        size
    };
    let read_size = min(src_size.saturating_sub(offset_src), len);
    let mut buf = vec![0; read_size];
    let read_size = { dbfs_common_read(src, &mut buf, offset_src as u64)? };

    let write_size = { dbfs_common_write(dest, &buf[..read_size], offset_dest as u64)? };

    // update dest ctime/mtime
    {
        let tx = db.tx(true)?;
        let bucket = tx.get_bucket(dest.to_be_bytes())?;
        bucket.put("ctime", ctime.to_be_bytes())?;
        bucket.put("mtime", ctime.to_be_bytes())?;
        tx.commit()?;
    }
    Ok(write_size)
}
