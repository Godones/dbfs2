use crate::clone_db;
use alloc::borrow::ToOwned;
use alloc::string::ToString;
use alloc::sync::Arc;
use core::cmp::min;
use core::fmt::write;
use rvfs::{File, FileOps, StrResult};
use spin::Mutex;

pub const DBFS_DIR_FILE_OPS: FileOps = FileOps::empty();
pub const DBFS_FILE_FILE_OPS: FileOps = {
    let mut ops = FileOps::empty();
    ops.write = dbfs_file_write;
    ops.read = dbfs_file_read;
    ops.open = |_| Ok(());
    ops
};
pub const DBFS_SYMLINK_FILE_OPS: FileOps = FileOps::empty();

fn dbfs_file_write(file: Arc<Mutex<File>>, buf: &[u8], offset: u64) -> StrResult<usize> {
    let file = file.lock();
    let dentry = file.f_dentry.lock();
    let inode = dentry.d_inode.lock();
    let numer = inode.number;
    dbfs_file_write_inner(numer, buf, offset)
}
fn dbfs_file_read(file: Arc<Mutex<File>>, buf: &mut [u8], offset: u64) -> StrResult<usize> {
    let file = file.lock();
    let dentry = file.f_dentry.lock();
    let inode = dentry.d_inode.lock();
    let numer = inode.number;
    dbfs_file_read_inner(numer, buf, offset)
}

fn dbfs_file_read_inner(number: usize, buf: &mut [u8], offset: u64) -> StrResult<usize> {
    let db = clone_db();
    let tx = db.tx(true).unwrap();
    let bucket = tx.get_bucket(number.to_be_bytes()).unwrap();
    let data = bucket.get_kv("data").unwrap();
    let mut data = data.value();
    let len = min(buf.len(), data.len().saturating_sub(offset as usize));
    buf[..len].copy_from_slice(&data[offset as usize..offset as usize + len]);
    tx.commit();
    Ok(buf.len())
}

fn dbfs_file_write_inner(number: usize, buf: &[u8], offset: u64) -> StrResult<usize> {
    let db = clone_db();
    let tx = db.tx(true).unwrap();
    let bucket = tx.get_bucket(number.to_be_bytes()).unwrap();
    let data = bucket.get_kv("data").unwrap();
    let mut data = data.value().to_owned();
    if data.len() < offset as usize + buf.len() {
        data.resize(offset as usize + buf.len(), 0);
    }
    data[offset as usize..offset as usize + buf.len()].copy_from_slice(buf);
    bucket.put("data".to_string(), data).unwrap();
    tx.commit();
    Ok(buf.len())
}
