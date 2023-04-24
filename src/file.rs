use crate::{clone_db, u16, usize};
use alloc::borrow::ToOwned;

use alloc::string::ToString;
use alloc::sync::Arc;
use alloc::vec::Vec;
use alloc::{format, vec};
use core::cmp::{max, min};
use core::ops::Range;
use jammdb::Data;
use log::{debug, error};

use crate::common::{DbfsDirEntry, DbfsError, DbfsFileType, DbfsPermission, DbfsResult};
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
    dbfs_common_write(numer, buf, offset).map_err(|_|"dbfs_common_write error")
}
fn dbfs_file_read(file: Arc<File>, buf: &mut [u8], offset: u64) -> StrResult<usize> {
    let dentry = file.f_dentry.clone();
    let inode = dentry.access_inner().d_inode.clone();
    let numer = inode.number;
    dbfs_common_read(numer, buf, offset).map_err(|_|"dbfs_common_read error")
}

/// the file data in dbfs is stored as a set of key-value pairs
/// * data1: \[u8;512]
/// * data2: \[u8;512]
/// * ....
/// * datai: \[u8;512]
pub fn dbfs_common_read(number: usize, buf: &mut [u8], offset: u64) -> DbfsResult<usize> {
    debug!("dbfs_common_read ino: {}, offset: {}, buf.len: {}", number, offset, buf.len());
    let db = clone_db();
    let tx = db.tx(false)?;
    let bucket = tx.get_bucket(number.to_be_bytes())?;
    let size = bucket.get_kv("size").unwrap();
    let size = usize!(size.value());
    if offset >= size as u64 {
        return Ok(0);
    }
    let num = offset / 512;
    // let mut offset = offset % 512;
    let mut buf_offset = 0;
    let _total = 0;
    let end_num = (offset + buf.len() as u64) / 512 + 1;

    let start_key = format!("data{:04x}", num as u32);
    let end_key = format!("data{:04x}", end_num as u32);
    let range = Range {
        start: start_key.as_bytes(),
        end: end_key.as_bytes(),
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
                let key = core::str::from_utf8(key).unwrap();
                let index = key.splitn(2, "data").nth(1).unwrap();
                let index = u32::from_str_radix(index, 16).unwrap();
                let current_size = index as usize * 512; // offset = 1000 ,current_size >= 512,1024 => offset= 1000 - 512 = 488
                let value_offset = offset.saturating_sub(current_size as u64) as usize; // 一定位于(0,512)范围
                let real_size = min(size - current_size, 512);
                let len = min(buf.len() - buf_offset, real_size - value_offset);
                buf[buf_offset..buf_offset + len]
                    .copy_from_slice(&value[value_offset..value_offset + len]);

                buf_offset += len;
            }
        }
        if buf_offset == buf.len() {
            break;
        }
    }
    Ok(buf_offset)
}

/// we need think about how to write data to dbfs
/// * data1: \[u8;512]
/// * data2: \[u8;512]
/// * ....
/// * datai: \[u8;512]
/// the i should be u32, because we can store 2^32 * 512 bytes in dbfs, == 2048 GB
/// u32 == 4 bytes, 0x00000000 - 0xffffffff
pub fn dbfs_common_write(number: usize, buf: &[u8], offset: u64) -> DbfsResult<usize> {
    let db = clone_db();
    let tx = db.tx(true)?;
    let bucket = tx.get_bucket(number.to_be_bytes())?;
    let size = bucket.get_kv("size").unwrap();
    let size = usize!(size.value());
    let o_offset = offset;
    let mut num = offset / 512;
    let mut offset = offset % 512;
    let mut count = 0;
    loop {
        let key = format!("data{:04x}", num as u32);
        let kv = bucket.get_kv(key.as_bytes());
        let mut data = if kv.is_some() {
            // the existed data
            kv.unwrap().value().to_owned()
        } else {
            // the new data
            [0; 512].to_vec()
        };
        // if offset as usize > data.len() {
        //     data.resize(offset as usize, 0);
        // }
        let len = min(buf.len() - count, 512 - offset as usize);
        data[offset as usize..offset as usize + len].copy_from_slice(&buf[count..count + len]);
        count += len;
        offset  = (offset + len as u64) % 512;
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
        if x.key().starts_with("data".as_bytes()) {
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
) -> Result<usize, ()> {
    let db = clone_db();
    let tx = db.tx(false).unwrap();
    let bucket = tx.get_bucket(number.to_be_bytes()).unwrap();
    let mut count = 0;
    for i in offset as usize..buf.len() + offset as usize {
        let mut x = &mut buf[i - offset as usize];
        let key = format!("data{}", i);
        let value = bucket.get_kv(key.as_bytes());
        if value.is_none() {
            continue;
        }
        let value = value.unwrap();
        count += 1;
        let str = core::str::from_utf8(value.value()).unwrap();
        let name = str.rsplitn(2, ':').collect::<Vec<&str>>();
        x.name = name[1].to_string();
        let inode_number = name[0].parse::<usize>().unwrap();
        x.ino = inode_number as u64;
        x.offset = i as u64;
        // x.kind
        let inode = tx.get_bucket(inode_number.to_be_bytes()).unwrap();
        let mode = inode.get_kv("mode").unwrap();
        let mode = u16!(mode.value());
        let perm = DbfsPermission::from_bits_truncate(mode);
        x.kind = DbfsFileType::from(perm);
    }
    error!("dbfs_common_readdir: count: {},buf:{:?}", count,&buf[0..count]);
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
