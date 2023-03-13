use crate::{clone_db, usize};
use alloc::borrow::ToOwned;
use alloc::string::ToString;
use alloc::sync::Arc;
use alloc::{format, vec};
use alloc::vec::Vec;
use core::cmp::min;
use core::fmt::write;
use rvfs::dentry::DirContext;
use rvfs::file::{File, FileOps};
use rvfs::{info, StrResult};
use spin::Mutex;

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
pub const DBFS_SYMLINK_FILE_OPS: FileOps = FileOps::empty();

fn dbfs_file_write(file: Arc<File>, buf: &[u8], offset: u64) -> StrResult<usize> {
    let dentry = file.f_dentry.clone();
    let inode = dentry.access_inner().d_inode.clone();
    let numer = inode.number;
    dbfs_file_write_inner(numer, buf, offset)
}
fn dbfs_file_read(file: Arc<File>, buf: &mut [u8], offset: u64) -> StrResult<usize> {
    let dentry = file.f_dentry.clone();
    let inode = dentry.access_inner().d_inode.clone();
    let numer = inode.number;
    dbfs_file_read_inner(numer, buf, offset)
}

/// the file data in dbfs is stored as a set of key-value pairs
/// * data1: \[u8;512]
/// * data2: \[u8;512]
/// * ....
/// * datai: \[u8;512]
fn dbfs_file_read_inner(number: usize, buf: &mut [u8], offset: u64) -> StrResult<usize> {
    let db = clone_db();
    let tx = db.tx(false).unwrap();
    let bucket = tx.get_bucket(number.to_be_bytes()).unwrap();
    let size = bucket.get_kv("size").unwrap();
    let size = usize!(size.value());
    if offset > size as u64 {
        return Ok(0);
    }
    let mut num = offset / 512;
    let mut offset = offset % 512;
    let mut buf_offset = 0;
    loop {
        let key = format!("data{:04x}", num as u32);
        let kv = bucket.get_kv(key.as_bytes());
        if kv.is_none() {
            break;
        }
        let kv = kv.unwrap();
        let value = kv.value();
        let len = min(buf.len() - buf_offset, 512 - offset as usize);
        buf[buf_offset..buf_offset + len].copy_from_slice(&value[offset as usize..offset as usize + len]);
        buf_offset += len;
        offset = 0;
        num += 1;
        if buf_offset == buf.len() {
            break;
        }
    }
    tx.commit();
    Ok(buf_offset)
}

/// we need think about how to write data to dbfs
/// * data1: \[u8;512]
/// * data2: \[u8;512]
/// * ....
/// * datai: \[u8;512]
/// the i should be u32, because we can store 2^32 * 512 bytes in dbfs, == 2048 GB
/// u32 == 4 bytes, 0x00000000 - 0xffffffff
fn dbfs_file_write_inner(number: usize, buf: &[u8], offset: u64) -> StrResult<usize> {
    let db = clone_db();
    let tx = db.tx(true).unwrap();
    let bucket = tx.get_bucket(number.to_be_bytes()).unwrap();
    let size = bucket.get_kv("size").unwrap();
    let size = usize!(size.value());
    if offset > size as u64 {
        return Err("offset > size");
    }
    let mut num = offset / 512;
    let mut offset = offset % 512;
    let mut count = 0;
    loop {
        let key = format!("data{:04x}", num as u32);
        let kv = bucket.get_kv(key.as_bytes());
        let mut data = if kv.is_some(){
            // the existed data
            kv.unwrap().value().to_owned()
        }else {
            // the new data
            vec![0;512]
        };
        if offset as usize > data.len(){
            data.resize(offset as usize, 0);
        }
        let len = min(buf.len() - count, 512 - offset as usize);
        data[offset as usize..offset as usize + len].copy_from_slice(&buf[count..count + len]);
        count += len;
        offset = 0;
        num += 1;
        bucket.put(key, data).unwrap();
        if count == buf.len() {
            break;
        }
    }
    tx.commit();
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
