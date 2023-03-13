use crate::file::{DBFS_DIR_FILE_OPS, DBFS_FILE_FILE_OPS, DBFS_SYMLINK_FILE_OPS};
use crate::{clone_db, u32};
use alloc::borrow::ToOwned;
use alloc::format;
use alloc::string::ToString;
use alloc::sync::Arc;
use core::sync::atomic::AtomicUsize;

use rvfs::dentry::DirEntry;
use rvfs::file::FileMode;
use rvfs::inode::{create_tmp_inode_from_sb_blk, Inode, InodeMode, InodeOps};
use rvfs::{ddebug, StrResult};
use spin::Mutex;

pub static DBFS_INODE_NUMBER: AtomicUsize = AtomicUsize::new(0);

pub const DBFS_DIR_INODE_OPS: InodeOps = {
    let mut ops = InodeOps::empty();
    ops.create = dbfs_create;
    ops.create = dbfs_create;
    ops.link = dbfs_link;
    ops.unlink = dbfs_unlink;
    ops.symlink = dbfs_symlink;

    ops
};

pub const DBFS_FILE_INODE_OPS: InodeOps = {
    let mut ops = InodeOps::empty();
    ops
};
pub const DBFS_SYMLINK_INODE_OPS: InodeOps = {
    let mut ops = InodeOps::empty();
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

fn dbfs_common_create(
    dir: Arc<Inode>,
    dentry: Arc<DirEntry>,
    mode: FileMode,
    inode_mode: InodeMode,
    target_path: Option<&str>,
) -> StrResult<()> {
    ddebug!("dbfs_common_create");
    let new_number = DBFS_INODE_NUMBER.fetch_add(1, core::sync::atomic::Ordering::SeqCst);
    let inode_ops = match inode_mode {
        InodeMode::S_DIR => DBFS_DIR_INODE_OPS,
        InodeMode::S_FILE => DBFS_FILE_INODE_OPS,
        InodeMode::S_SYMLINK => DBFS_SYMLINK_INODE_OPS,
        _ => InodeOps::empty(),
    };
    let file_ops = match inode_mode {
        InodeMode::S_DIR => DBFS_DIR_FILE_OPS,
        InodeMode::S_FILE => DBFS_FILE_FILE_OPS,
        InodeMode::S_SYMLINK => DBFS_SYMLINK_FILE_OPS,
        _ => return Err("not support type"),
    };
    let n_inode = create_tmp_inode_from_sb_blk(
        dir.super_blk.upgrade().unwrap().clone(),
        new_number,
        inode_mode,
        0,
        inode_ops,
        file_ops,
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

fn inode_mode_from_file_mode(file_mode: FileMode) -> &'static str {
    match file_mode {
        FileMode::FMODE_WRITE => INODE_MODE[1],
        FileMode::FMODE_READ => INODE_MODE[0],
        FileMode::FMODE_EXEC => INODE_MODE[2],
        _ => INODE_MODE[3],
    }
}

fn inode_type_from_inode_mode(inode_mode: InodeMode) -> &'static str {
    match inode_mode {
        InodeMode::S_FILE => INODE_TYPE[0],
        InodeMode::S_DIR => INODE_TYPE[1],
        InodeMode::S_SYMLINK => INODE_TYPE[2],
        _ => INODE_TYPE[3],
    }
}
