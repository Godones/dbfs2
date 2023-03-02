use crate::{clone_db, wwarn};
use alloc::string::ToString;
use alloc::sync::Arc;
use core::sync::atomic::AtomicUsize;
use rvfs::{
    create_tmp_inode_from_sb_blk, DirEntry, FileMode, Inode, InodeMode, InodeOps, StrResult,
};
use spin::Mutex;
use crate::file::DBFS_FILE_FILE_OPS;

pub static DBFS_INODE_NUMBER: AtomicUsize = AtomicUsize::new(0);

pub const DBFS_DIR_INODE_OPS: InodeOps = InodeOps {
    follow_link: |_, _| Err("Not a symlink"),
    readlink: |_,_|Err("Not support"),
    lookup: |_, _| Err("Not support"),
    create: dbfs_create,
    mkdir: dbfs_mkdir,
    rmdir: |_,_| Err("Not support"),
    link: dbfs_link,
    unlink: dbfs_unlink,
    truncate: |_|Err("Not support"),
    get_attr: |_, _, _| Err("Not support"),
    set_attr: |_,_,_|Err("Not support"),
    remove_attr: |_,_|Err("Not support"),
    list_attr: |_,_|Err("Not support"),
    symlink: dbfs_symlink,
    rename: |_,_,_,_|Err("Not support"),
};

pub const DBFS_FILE_INODE_OPS: InodeOps = {
    let mut ops = InodeOps::empty();
    ops
};
pub const DBFS_SYMLINK_INODE_OPS: InodeOps = {
    let mut ops = InodeOps::empty();
    ops
};

fn dbfs_create(
    dir: Arc<Mutex<Inode>>,
    dentry: Arc<Mutex<DirEntry>>,
    mode: FileMode,
) -> StrResult<()> {
    dbfs_common_create(dir, dentry, mode, InodeMode::S_FILE, None)
}
fn dbfs_mkdir(
    dir: Arc<Mutex<Inode>>,
    dentry: Arc<Mutex<DirEntry>>,
    mode: FileMode,
) -> StrResult<()> {
    dbfs_common_create(dir, dentry, mode, InodeMode::S_DIR, None)
}
fn dbfs_link(
    old_dentry: Arc<Mutex<DirEntry>>,
    dir: Arc<Mutex<Inode>>,
    new_dentry: Arc<Mutex<DirEntry>>,
) -> StrResult<()> {
    let old_dentry = old_dentry.lock();
    let mut new_dentry = new_dentry.lock();
    let dir = dir.lock();
    let mut old_inode = old_dentry.d_inode.lock();
    let number = dir.number;
    let db = clone_db();

    // update new inode data in db
    let tx = db.tx(true).unwrap();
    let bucket = tx.get_bucket(number.to_be_bytes()).unwrap();
    bucket
        .put(new_dentry.d_name.clone(), old_inode.number.to_be_bytes())
        .unwrap();
    tx.commit().unwrap();
    // update old inode data in db
    let tx = db.tx(true).unwrap();
    let bucket = tx.get_bucket(number.to_be_bytes()).unwrap();
    bucket
        .put(new_dentry.d_name.clone(), old_inode.number.to_be_bytes())
        .unwrap();
    let old_bucket = tx.get_bucket(old_inode.number.to_be_bytes()).unwrap();
    let hard_links = old_bucket.get_kv("hard_links".to_string()).unwrap();
    let mut value = u32::from_le_bytes(hard_links.value()[..].try_into().unwrap());
    value += 1;
    old_bucket.put("hard_links", value.to_be_bytes()).unwrap();
    tx.commit().unwrap();
    // update old inode data in memory
    old_inode.hard_links += 1;
    new_dentry.d_inode = old_dentry.d_inode.clone();
    Ok(())
}
fn dbfs_unlink(dir: Arc<Mutex<Inode>>, dentry: Arc<Mutex<DirEntry>>) -> StrResult<()> {
    let dir = dir.lock();
    let dentry = dentry.lock();
    let mut inode = dentry.d_inode.lock();
    let number = dir.number;
    let db = clone_db();

    // delete dentry in db
    let tx = db.tx(true).unwrap();
    let bucket = tx.get_bucket(number.to_be_bytes()).unwrap();
    bucket.delete(dentry.d_name.clone()).unwrap();
    tx.commit().unwrap();

    // update inode data in db
    let tx = db.tx(true).unwrap();
    let bucket = tx.get_bucket(inode.number.to_be_bytes()).unwrap();
    let hard_links = bucket.get_kv("hard_links".to_string()).unwrap();
    let mut value = u32::from_le_bytes(hard_links.value()[..].try_into().unwrap());
    value -= 1;
    bucket.put("hard_links", value.to_be_bytes()).unwrap();
    tx.commit().unwrap();
    inode.hard_links -= 1;
    assert_eq!(inode.hard_links, value);
    if inode.hard_links == 0 {
        // delete inode in db
        let tx = db.tx(true).unwrap();
        tx.delete_bucket(inode.number.to_be_bytes()).unwrap();
        tx.commit().unwrap();
    }
    Ok(())
}
fn dbfs_symlink(
    dir: Arc<Mutex<Inode>>,
    dentry: Arc<Mutex<DirEntry>>,
    target: &str,
) -> StrResult<()> {
    dbfs_common_create(
        dir,
        dentry,
        FileMode::FMODE_READ,
        InodeMode::S_SYMLINK,
        Some(target),
    )
}

fn dbfs_common_create(
    dir: Arc<Mutex<Inode>>,
    dentry: Arc<Mutex<DirEntry>>,
    mode: FileMode,
    inode_mode: InodeMode,
    target_path: Option<&str>,
) -> StrResult<()> {
    wwarn!("dbfs_common_create");
    // 1. 为与目录项对象相关的普通文件创建一个新的磁盘索引节点。
    // 2. 为与目录项对象相关的普通文件创建一个新的inode对象。
    let dir = dir.lock();
    let n_inode = create_tmp_inode_from_sb_blk(dir.super_blk.upgrade().unwrap().clone())?;
    let inode_number = dir.number;
    let db = clone_db();
    let tx = db.tx(true).unwrap();
    let parent = tx.get_bucket(inode_number.to_be_bytes()).unwrap();
    let new_number = DBFS_INODE_NUMBER.fetch_add(1, core::sync::atomic::Ordering::SeqCst);
    let mut dentry = dentry.lock();
    parent
        .put(dentry.d_name.clone(), new_number.to_be_bytes())
        .unwrap();
    let new_inode = tx.create_bucket(new_number.to_be_bytes()).unwrap();
    new_inode
        .put("mode", inode_mode_from_file_mode(mode))
        .unwrap();
    new_inode
        .put("type", inode_type_from_inode_mode(inode_mode))
        .unwrap();
    new_inode.put("size", 0usize.to_be_bytes()).unwrap();
    new_inode.put("hard_links", 1u32.to_be_bytes()).unwrap();
    new_inode.put("uid", 0usize.to_be_bytes()).unwrap();
    new_inode.put("gid", 0usize.to_be_bytes()).unwrap();
    new_inode.put("atime", 0usize.to_be_bytes()).unwrap();
    new_inode.put("mtime", 0usize.to_be_bytes()).unwrap();
    new_inode.put("ctime", 0usize.to_be_bytes()).unwrap();
    if inode_mode == InodeMode::S_SYMLINK {
        new_inode.put("data", target_path.unwrap()).unwrap();
    } else {
        new_inode.put("data", "").unwrap();
    }
    tx.commit().unwrap();
    // fill inode
    let mut inode = n_inode.lock();
    inode.inode_ops = match inode_mode {
        InodeMode::S_DIR => DBFS_DIR_INODE_OPS,
        InodeMode::S_FILE => DBFS_FILE_INODE_OPS,
        _ => InodeOps::empty(),
    };
    inode.file_ops = DBFS_FILE_FILE_OPS;
    inode.number = new_number;
    inode.mode = inode_mode;
    inode.hard_links = match inode_mode {
        InodeMode::S_DIR => 2,
        _ => 1,
    };
    drop(inode);
    // set dentry with inode
    dentry.d_inode = n_inode;
    wwarn!("dbfs_common_create end");
    Ok(())
}

const INODE_MODE: [&str; 4] = ["r", "w", "x", "-"];
const INODE_TYPE: [&str; 5] = ["f", "d", "l", "b", "-"];

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
        _ => INODE_TYPE[4],
    }
}
