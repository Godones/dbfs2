use crate::file::{DBFS_DIR_FILE_OPS, DBFS_FILE_FILE_OPS, DBFS_SYMLINK_FILE_OPS};
use crate::{clone_db, u32, usize};
use alloc::borrow::ToOwned;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::cmp::min;
use core::sync::atomic::AtomicUsize;

use rvfs::dentry::{DirEntry, LookUpData};
use rvfs::file::{FileMode, FileOps};
use rvfs::inode::{create_tmp_inode_from_sb_blk, Inode, InodeMode, InodeOps};
use rvfs::{ddebug, StrResult};
use spin::Mutex;

pub static DBFS_INODE_NUMBER: AtomicUsize = AtomicUsize::new(0);

pub const DBFS_DIR_INODE_OPS: InodeOps = {
    let mut ops = InodeOps::empty();
    ops.create = dbfs_create;
    ops.mkdir = dbfs_mkdir;
    ops.link = dbfs_link;
    ops.unlink = dbfs_unlink;
    ops.symlink = dbfs_symlink;
    ops.lookup = dbfs_lookup;
    ops.rmdir = dbfs_rmdir;
    ops.set_attr = dbfs_setattr;
    ops.get_attr = dbfs_getattr;
    ops.list_attr = dbfs_listattr;
    ops.remove_attr = dbfs_removeattr;
    ops
};

pub const DBFS_FILE_INODE_OPS: InodeOps = {
    let mut ops = InodeOps::empty();
    ops.set_attr = dbfs_setattr;
    ops.get_attr = dbfs_getattr;
    ops.list_attr = dbfs_listattr;
    ops
};
pub const DBFS_SYMLINK_INODE_OPS: InodeOps = {
    let mut ops = InodeOps::empty();
    ops.set_attr = dbfs_setattr;
    ops.get_attr = dbfs_getattr;
    ops.list_attr = dbfs_listattr;
    ops.readlink = dbfs_readlink;
    ops.follow_link = dbfs_followlink;
    ops
};

fn dbfs_create(dir: Arc<Inode>, dentry: Arc<DirEntry>, mode: FileMode) -> StrResult<()> {
    dbfs_common_create(dir, dentry, mode, InodeMode::S_FILE, None)
}
fn dbfs_mkdir(dir: Arc<Inode>, dentry: Arc<DirEntry>, mode: FileMode) -> StrResult<()> {
    dbfs_common_create(dir, dentry, mode, InodeMode::S_DIR, None)
}

fn dbfs_link(
    old_dentry: Arc<DirEntry>,
    dir: Arc<Inode>,
    new_dentry: Arc<DirEntry>,
) -> StrResult<()> {
    let db = clone_db();
    // update new inode data in db
    let tx = db.tx(true).unwrap();
    let old_inode = old_dentry.access_inner().d_inode.clone();
    let number = dir.number;
    let bucket = tx.get_bucket(number.to_be_bytes()).unwrap();
    let next_number = bucket.next_int();
    // we use data0, data1, data2, ... to store data
    // but the number of data is not continuous,
    // we only neet make sure that the number is unique
    let key = format!("data{}", next_number);
    let value = format!(
        "{}:{}",
        new_dentry.access_inner().d_name.clone(),
        old_inode.number
    );
    bucket.put(key, value).unwrap();
    tx.commit().unwrap();
    // update old inode data in db
    let tx = db.tx(true).unwrap();
    let old_bucket = tx.get_bucket(old_inode.number.to_be_bytes()).unwrap();
    let hard_links = old_bucket.get_kv("hard_links".to_string()).unwrap();
    let mut value = u32!(hard_links.value());
    value += 1;
    old_bucket.put("hard_links", value.to_be_bytes()).unwrap();
    tx.commit().unwrap();

    // update old inode data in memory
    // update hard_links
    // set the new dentry's inode to old inode
    old_inode.access_inner().hard_links += 1;
    new_dentry.access_inner().d_inode = old_inode;
    Ok(())
}
fn dbfs_unlink(dir: Arc<Inode>, dentry: Arc<DirEntry>) -> StrResult<()> {
    let inode = dentry.access_inner().d_inode.clone();
    let number = dir.number;
    let db = clone_db();

    // delete dentry in db
    let tx = db.tx(true).unwrap();
    let bucket = tx.get_bucket(number.to_be_bytes()).unwrap();
    // find the dentry in db
    let value = bucket.kv_pairs().find(|kv| {
        kv.value()
            .starts_with(dentry.access_inner().d_name.as_bytes())
    });
    bucket.delete(value.unwrap().key()).unwrap();
    tx.commit().unwrap();

    // update inode data in db
    let tx = db.tx(true).unwrap();
    let bucket = tx.get_bucket(inode.number.to_be_bytes()).unwrap();
    let hard_links = bucket.get_kv("hard_links".to_string()).unwrap();
    let mut value = u32!(hard_links.value());
    value -= 1;
    bucket.put("hard_links", value.to_be_bytes()).unwrap();
    tx.commit().unwrap();

    let mut inner = inode.access_inner();
    inner.hard_links -= 1;

    if inner.hard_links == 0 {
        // delete inode in db
        let tx = db.tx(true).unwrap();
        tx.delete_bucket(inode.number.to_be_bytes()).unwrap();
        tx.commit().unwrap();
    }
    Ok(())
}

fn dbfs_symlink(dir: Arc<Inode>, dentry: Arc<DirEntry>, target: &str) -> StrResult<()> {
    dbfs_common_create(
        dir,
        dentry,
        FileMode::FMODE_READ,
        InodeMode::S_SYMLINK,
        Some(target),
    )
}

fn dbfs_lookup(dir: Arc<Inode>, dentry: Arc<DirEntry>) -> StrResult<()>{
    let db = clone_db();
    let tx = db.tx(false).unwrap();
    let number = dir.number;
    let bucket = tx.get_bucket(number.to_be_bytes()).unwrap();
    let name = &dentry.access_inner().d_name;
    let value = bucket.kv_pairs().find(|kv| {
        kv.key().starts_with("data".as_bytes())&&kv.value().starts_with(name.as_bytes())
    });
    if value.is_none(){
        return Err("not found");
    }
    let value = value.unwrap();
    let value = value.value();
    let str = core::str::from_utf8(value).unwrap();
    let data = str.rsplitn(2, ':').collect::<Vec<&str>>();
    let number  = data[0].parse::<usize>().unwrap();
    let bucket = tx.get_bucket(number.to_be_bytes()).unwrap();
    let mode = bucket.get_kv("mode").unwrap();
    let inode_mode = InodeMode::from(mode.value());
    let size = bucket.get_kv("size").unwrap();
    let size = usize!(size.value());
    // create a inode according to the data in db
    let n_inode = create_tmp_inode_from_sb_blk(
        dir.super_blk.upgrade().unwrap().clone(),
        number,
        inode_mode,
        0,
        inode_ops_from_inode_mode(inode_mode),
        file_ops_from_inode_mode(inode_mode),
        None,
    )?;
    n_inode.access_inner().file_size = size;
    dentry.access_inner().d_inode = n_inode;
    Ok(())
}

fn dbfs_rmdir(dir: Arc<Inode>, dentry: Arc<DirEntry>) -> StrResult<()>{
    todo!()
}

/// create a new attribute for a dentry
/// if the key is already exist, it will be overwrite
/// if the key is not exist, it will be created
fn dbfs_setattr(dentry: Arc<DirEntry>, key: &str, val: &[u8]) -> StrResult<()>{
    let db = clone_db();
    let tx = db.tx(true).unwrap();
    let number= dentry.access_inner().d_inode.number;
    let bucket = tx.get_bucket(number.to_be_bytes()).unwrap();
    let key = format!("attr:{}", key);
    bucket.put(key, val).unwrap();
    tx.commit().unwrap();
    Ok(())
}
fn dbfs_removeattr(dentry: Arc<DirEntry>, key: &str) -> StrResult<()>{
    let db = clone_db();
    let tx = db.tx(true).unwrap();
    let number= dentry.access_inner().d_inode.number;
    let bucket = tx.get_bucket(number.to_be_bytes()).unwrap();
    let key = format!("attr:{}", key);
    bucket.delete(key).unwrap();
    tx.commit().unwrap();
    Ok(())
}
fn dbfs_getattr(dentry: Arc<DirEntry>, key: &str,buf:&mut [u8]) -> StrResult<usize>{
    let db = clone_db();
    let tx = db.tx(false).unwrap();
    let number= dentry.access_inner().d_inode.number;
    let bucket = tx.get_bucket(number.to_be_bytes()).unwrap();
    let key = format!("attr:{}", key);
    let value = bucket.get_kv(key).unwrap();
    let value = value.value();
    let len = min(value.len(), buf.len());
    buf[..len].copy_from_slice(value);
    Ok(len)
}

fn dbfs_listattr(dentry: Arc<DirEntry>, buf: &mut [u8]) -> StrResult<usize>{
    let db = clone_db();
    let tx = db.tx(false).unwrap();
    let number= dentry.access_inner().d_inode.number;
    let bucket = tx.get_bucket(number.to_be_bytes()).unwrap();
    let mut len = 0;
    for kv in bucket.kv_pairs(){
        let key = kv.key();
        if key.starts_with("attr:".as_bytes()){
            let key = key.splitn(2, |c|*c==b':').collect::<Vec<&[u8]>>();
            let key = key[1];
            let key_len = key.len();
            if len + key_len > buf.len(){
                break;
            }
            buf[len..len+key_len].copy_from_slice(key);
            buf[len+key_len] = 0;
            len += key_len+1;
        }
    }
    Ok(len)
}
fn dbfs_readlink(dentry: Arc<DirEntry>, buf: &mut [u8]) -> StrResult<usize>{
    let db = clone_db();
    let tx = db.tx(false).unwrap();
    let number= dentry.access_inner().d_inode.number;
    let bucket = tx.get_bucket(number.to_be_bytes()).unwrap();
    let value = bucket.get_kv("target").unwrap();
    let value = value.value();
    let len = min(value.len(), buf.len());
    buf[..len].copy_from_slice(value);
    Ok(len)
}
fn dbfs_followlink(dentry: Arc<DirEntry>, lookup_data: &mut LookUpData)->StrResult<()>{
    todo!()
}
fn dbfs_common_create(
    dir: Arc<Inode>,
    dentry: Arc<DirEntry>,
    mode: FileMode,
    inode_mode: InodeMode,
    target_path: Option<&str>,
) -> StrResult<()> {
    ddebug!("dbfs_common_create");
    let new_number = DBFS_INODE_NUMBER.fetch_add(1, core::sync::atomic::Ordering::SeqCst);

    let n_inode = create_tmp_inode_from_sb_blk(
        dir.super_blk.upgrade().unwrap().clone(),
        new_number,
        inode_mode,
        0,
        inode_ops_from_inode_mode(inode_mode),
        file_ops_from_inode_mode(inode_mode),
        None,
    )?;

    let inode_number = dir.number;
    let db = clone_db();
    let tx = db.tx(true).unwrap();
    let parent = tx.get_bucket(inode_number.to_be_bytes()).unwrap();
    let name = dentry.access_inner().d_name.to_owned();

    let next_number = parent.next_int();
    let key = format!("data{}", next_number);
    let value = format!("{}:{}", name, new_number);
    parent.put(key, value).unwrap();

    let new_inode = tx.create_bucket(new_number.to_be_bytes()).unwrap();
    new_inode
        .put("mode", inode_mode_from_file_mode(mode))
        .unwrap();
    new_inode
        .put("type", inode_type_from_inode_mode(inode_mode))
        .unwrap();
    new_inode.put("size", 0usize.to_be_bytes()).unwrap();
    if inode_mode == InodeMode::S_DIR {
        new_inode.put("hard_links", 2u32.to_be_bytes()).unwrap();
    } else {
        new_inode.put("hard_links", 1u32.to_be_bytes()).unwrap();
    }
    new_inode.put("uid", 0usize.to_be_bytes()).unwrap();
    new_inode.put("gid", 0usize.to_be_bytes()).unwrap();
    new_inode.put("atime", 0usize.to_be_bytes()).unwrap();
    new_inode.put("mtime", 0usize.to_be_bytes()).unwrap();
    new_inode.put("ctime", 0usize.to_be_bytes()).unwrap();
    if inode_mode == InodeMode::S_SYMLINK {
        new_inode.put("data", target_path.unwrap()).unwrap();
    }
    tx.commit().unwrap();

    // set dentry with inode
    dentry.access_inner().d_inode = n_inode;
    ddebug!("dbfs_common_create end");
    Ok(())
}

const INODE_MODE: [&str; 4] = ["r", "w", "x", "-"];
const INODE_TYPE: [&str; 4] = ["f", "d", "l", "-"];

fn inode_mode_from_file_mode(file_mode: FileMode) -> String {
    let mut mode = String::new();
    if file_mode.contains(FileMode::FMODE_READ) {
        mode.push_str(INODE_MODE[0]);
    } else if file_mode.contains(FileMode::FMODE_WRITE) {
        mode.push_str(INODE_MODE[1]);
    } else if file_mode.contains(FileMode::FMODE_EXEC) {
        mode.push_str(INODE_MODE[2]);
    } else {
        mode.push_str(INODE_MODE[3]);
    }
    mode
}


fn inode_type_from_inode_mode(inode_mode: InodeMode) -> &'static str {
    match inode_mode {
        InodeMode::S_FILE => INODE_TYPE[0],
        InodeMode::S_DIR => INODE_TYPE[1],
        InodeMode::S_SYMLINK => INODE_TYPE[2],
        _ => INODE_TYPE[3],
    }
}
fn inode_ops_from_inode_mode(inode_mode: InodeMode) -> InodeOps {
    match inode_mode {
        InodeMode::S_FILE => DBFS_FILE_INODE_OPS,
        InodeMode::S_DIR => DBFS_DIR_INODE_OPS,
        InodeMode::S_SYMLINK => DBFS_SYMLINK_INODE_OPS,
        _ => InodeOps::empty(),
    }
}

fn file_ops_from_inode_mode(inode_mode: InodeMode) -> FileOps {
    match inode_mode {
        InodeMode::S_FILE => DBFS_FILE_FILE_OPS,
        InodeMode::S_DIR => DBFS_DIR_FILE_OPS,
        InodeMode::S_SYMLINK => DBFS_SYMLINK_FILE_OPS,
        _ => FileOps::empty(),
    }
}