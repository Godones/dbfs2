use crate::{clone_db, SLICE_SIZE, u16, u32, usize};
use alloc::borrow::ToOwned;

use alloc::string::ToString;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::cmp::{max, min};
use core::ops::Range;
use jammdb::Data;
use log::{debug, error, warn};

use crate::common::{
    generate_data_key, DbfsDirEntry, DbfsError, DbfsFileType, DbfsPermission, DbfsResult,
    DbfsTimeSpec,
};
use crate::inode::{checkout_access, dbfs_common_attr};
use rvfs::dentry::DirContext;
use rvfs::file::{File, FileOps};
use rvfs::StrResult;

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
        "dbfs_common_read ino: {}, offset: {}, buf.len: {}",
        number,
        offset,
        buf.len()
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
    //     let key = generate_data_key(start_num as u32);
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
    // let mut offset = offset % SLICE_SIZE as u64;
    let mut buf_offset = 0;
    let end_num = (offset + buf.len() as u64) / SLICE_SIZE as u64 + 1;
    let f_end_num = size / SLICE_SIZE + 1;

    let end_num = min(end_num, f_end_num as u64);

    warn!(
        "start_num: {:?}, end_num: {:?}, file_end_num:{:?}",start_num,end_num,f_end_num
    );

    let start_key = generate_data_key(start_num as u32);
    let end_key = generate_data_key(end_num as u32);


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
                debug!("key: {:?}", index);
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
                debug!( "read len: {}", len);
            }
        }
        if buf_offset == buf.len() {
            break;
        }
    }
    Ok(buf_offset)


   // Ok(count)
}

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
    loop {
        let key = generate_data_key(num as u32);
        let kv = bucket.get_kv(key.as_slice());
        let mut data = if kv.is_some() {
            // the existed data
            kv.unwrap().value().to_owned()
        } else {
            // the new data
            [0; SLICE_SIZE].to_vec()
        };
        // if offset as usize > data.len() {
        //     data.resize(offset as usize, 0);
        // }
        let len = min(buf.len() - count, SLICE_SIZE - offset as usize);
        data[offset as usize..offset as usize + len].copy_from_slice(&buf[count..count + len]);
        count += len;
        offset = (offset + len as u64) % SLICE_SIZE as u64;
        num += 1;
        bucket.put(key, data).unwrap();
        if count == buf.len() {
            break;
        }
    }
    let new_size = max(size, (o_offset as usize + count) as usize);
    bucket.put("size", new_size.to_be_bytes()).unwrap();
    tx.commit()?;
    Ok(count)
}

fn dbfs_readdir(file: Arc<File>) -> StrResult<DirContext> {
    let dentry = file.f_dentry.clone();
    let inode = dentry.access_inner().d_inode.clone();
    let numer = inode.number;
    let db = clone_db();
    let tx = db.tx(false).unwrap();
    let bucket = tx.get_bucket(numer.to_be_bytes()).unwrap();
    let mut data = vec![];
    bucket.kv_pairs().for_each(|x| {
        if x.key().starts_with("data:".as_bytes()) {
            let value = x.value();
            let str = core::str::from_utf8(value).unwrap();
            let name = str.rsplitn(2, ':').collect::<Vec<&str>>();
            data.extend_from_slice(name[1].as_bytes());
            data.push(0);
        }
    });
    Ok(DirContext::new(data))
}

pub fn dbfs_common_readdir(
    number: usize,
    buf: &mut Vec<DbfsDirEntry>,
    offset: u64,
    is_readdir_plus:bool
) -> DbfsResult<usize> {
    let db = clone_db();
    let tx = db.tx(false)?;
    let bucket = tx.get_bucket(number.to_be_bytes())?;
    buf.clear();
    let next_number = bucket.get_kv("next_number").unwrap();
    let next_number = u32!(next_number.value());
    let mut count = 0;

    let start_key = generate_data_key(offset as u32);
    let end_key = generate_data_key(next_number);
    let range = Range {
        start: start_key.as_slice(),
        end: end_key.as_slice(),
    };

    bucket.range(range).for_each(|x| {
        if let Data::KeyValue(kv) = x {
            let key = kv.key();
            let key = key.splitn(2, |x| *x == b':').collect::<Vec<&[u8]>>();
            let key = key[1];
            let offset = u32!(key);
            let value = kv.value();
            let str = core::str::from_utf8(value).unwrap();
            let name = str.rsplitn(2, ':').collect::<Vec<&str>>();
            let inode_number = name[0].parse::<usize>().unwrap();
            let mut entry = DbfsDirEntry::default();
            entry.name = name[1].to_string();
            entry.ino = inode_number as u64;
            entry.offset = offset as u64;

            if !is_readdir_plus{
                let inode = tx.get_bucket(inode_number.to_be_bytes()).unwrap();
                let mode = inode.get_kv("mode").unwrap();
                let mode = u16!(mode.value());
                let perm = DbfsPermission::from_bits_truncate(mode);
                entry.kind = DbfsFileType::from(perm);
            }else {
                let attr = dbfs_common_attr(inode_number).unwrap();
                entry.kind = attr.kind;
                entry.attr = Some(attr);
            }

            buf.push(entry);

            count += 1;
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
