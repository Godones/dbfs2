use crate::file::DBFS_DIR_FILE_OPS;
use crate::inode::{permission_from_mode, DBFS_DIR_INODE_OPS, DBFS_INODE_NUMBER};
use crate::{clone_db, u32, usize};
use alloc::boxed::Box;
use alloc::string::ToString;
use alloc::sync::{Arc, Weak};
use alloc::{format, vec};
use log::warn;
use rvfs::dentry::{DirEntry, DirEntryOps, DirFlags};
use rvfs::file::FileMode;
use rvfs::inode::{create_tmp_inode_from_sb_blk, Inode, InodeMode};
use rvfs::mount::MountFlags;
use rvfs::superblock::{
    find_super_blk, DataOps, FileSystemAttr, FileSystemType, FileSystemTypeInner, SuperBlock,
    SuperBlockInner, SuperBlockOps,
};
use rvfs::{ddebug, StrResult};
use spin::Mutex;

pub const DBFS: FileSystemType = FileSystemType {
    name: "dbfs",
    fs_flags: FileSystemAttr::FS_REQUIRES_DEV,
    get_super_blk: dbfs_get_super_blk,
    kill_super_blk: dbfs_kill_super_blk,
    inner: Mutex::new(FileSystemTypeInner {
        super_blk_s: vec![],
    }),
};
const DBFS_SB_BLK_OPS: SuperBlockOps = {
    let mut sb_ops = SuperBlockOps::empty();
    sb_ops.sync_fs = dbfs_sync_fs;
    sb_ops
};

fn dbfs_sync_fs(_sb_blk: Arc<SuperBlock>) -> StrResult<()> {
    let db = clone_db();
    let tx = db.tx(true).unwrap();
    let bucket = tx.get_or_create_bucket("super_blk".as_bytes()).unwrap();
    let continue_number = DBFS_INODE_NUMBER.load(core::sync::atomic::Ordering::SeqCst);
    bucket
        .put("continue_number".as_bytes(), continue_number.to_be_bytes())
        .unwrap();
    tx.commit().map_err(|_| "dbfs_sync_fs error")?;
    Ok(())
}

fn dbfs_get_super_blk(
    fs_type: Arc<FileSystemType>,
    flags: MountFlags,
    dev_name: &str,
    data: Option<Box<dyn DataOps>>,
) -> StrResult<Arc<SuperBlock>> {
    ddebug!("dbfs_get_super_blk");
    let compare = |sb_blk: Arc<SuperBlock>| -> bool { sb_blk.blk_dev_name.as_str() == dev_name };
    let find_sb_blk = find_super_blk(fs_type.clone(), Some(&compare));
    let sb_blk = match find_sb_blk {
        // find the old sb_blk
        Ok(_) => Err("super block exist, can't create new one"),
        Err(_) => {
            // create new sb_blk
            ddebug!("create new super block for ramfs");
            let sb_blk = dbfs_create_simple_super_blk(fs_type.clone(), flags, dev_name, data)?;
            dbfs_fill_super_block(sb_blk.clone())?;
            fs_type.insert_super_blk(sb_blk.clone());
            Ok(sb_blk)
        }
    };
    ddebug!("dbfs_get_super_blk end");
    sb_blk
}
fn dbfs_kill_super_blk(_super_blk: Arc<SuperBlock>) {}

fn dbfs_create_simple_super_blk(
    fs_type: Arc<FileSystemType>,
    flags: MountFlags,
    dev_name: &str,
    data: Option<Box<dyn DataOps>>,
) -> StrResult<Arc<SuperBlock>> {
    let db = clone_db();
    let tx = db.tx(false);
    if tx.is_err() {
        return Err("dbfs_fill_super_block: get db tx failed");
    }
    let tx = tx.unwrap();
    let bucket = tx.get_bucket("super_blk");
    if bucket.is_err() {
        return Err("dbfs_fill_super_block: get bucket failed");
    }
    let bucket = bucket.unwrap();
    let continue_number = bucket.get_kv("continue_number").unwrap();
    let continue_number = usize!(continue_number.value());
    // set the next inode number
    DBFS_INODE_NUMBER.store(continue_number, core::sync::atomic::Ordering::SeqCst);
    let blk_size = bucket.get_kv("blk_size").unwrap();
    let blk_size = u32!(blk_size.value());
    let magic = bucket.get_kv("magic").unwrap();
    let magic = u32!(magic.value());
    let sb_blk = SuperBlock {
        dev_desc: 0,
        device: None,
        block_size: blk_size,
        dirty_flag: false,
        file_max_bytes: usize::MAX,
        mount_flag: flags,
        magic,
        file_system_type: Arc::downgrade(&fs_type),
        super_block_ops: DBFS_SB_BLK_OPS,
        blk_dev_name: dev_name.to_string(),
        data,
        inner: Mutex::new(SuperBlockInner::empty()),
    };
    let sb_blk = Arc::new(sb_blk);
    Ok(sb_blk)
}

// TODO save metadata to db
fn dbfs_fill_super_block(sb_blk: Arc<SuperBlock>) -> StrResult<()> {
    let inode = dbfs_create_root_inode(sb_blk.clone())?;
    let dentry = dbfs_create_root_dentry(inode)?;
    sb_blk.access_inner().root = dentry;
    Ok(())
}

// create root inode for dbfs
fn dbfs_create_root_inode(sb_blk: Arc<SuperBlock>) -> StrResult<Arc<Inode>> {
    let count = dbfs_common_root_inode(0, 0, 0)?;
    let first_number = DBFS_INODE_NUMBER.load(core::sync::atomic::Ordering::SeqCst);
    assert_eq!(first_number, 2);
    // create a inode from super block
    let inode = create_tmp_inode_from_sb_blk(
        sb_blk.clone(),
        first_number - 1,
        InodeMode::S_DIR,
        0,
        DBFS_DIR_INODE_OPS,
        DBFS_DIR_FILE_OPS,
        None,
    )?;
    // because the default value of hard_links is 2,so we need to set it to 1
    inode.access_inner().hard_links = 0;
    inode.access_inner().file_size = count;
    Ok(inode)
}

pub fn dbfs_common_root_inode(uid: u32, gid: u32, ctime: usize) -> Result<usize, &'static str> {
    let db = clone_db();
    let tx = db.tx(true).unwrap();
    if tx.get_bucket(1usize.to_be_bytes()).is_err() {
        let permission = permission_from_mode(FileMode::FMODE_RDWR, InodeMode::S_DIR);
        let new_inode = tx.create_bucket(1usize.to_be_bytes()).unwrap();
        DBFS_INODE_NUMBER.fetch_add(1, core::sync::atomic::Ordering::SeqCst);
        new_inode
            .put("mode", permission.bits().to_be_bytes())
            .unwrap();
        // set the size of inode to 0
        // new_inode.put("size", 0usize.to_be_bytes()).unwrap();
        new_inode.put("hard_links", 0u32.to_be_bytes()).unwrap();
        new_inode.put("uid", uid.to_be_bytes()).unwrap();
        new_inode.put("gid", gid.to_be_bytes()).unwrap();
        // set time
        new_inode.put("atime", ctime.to_be_bytes()).unwrap();
        new_inode.put("mtime", ctime.to_be_bytes()).unwrap();
        new_inode.put("ctime", ctime.to_be_bytes()).unwrap();
        new_inode.put("block_size", 512u32.to_be_bytes()).unwrap();
        new_inode.put("next_number", 0usize.to_be_bytes()).unwrap();
        new_inode.put("size", 0usize.to_be_bytes()).unwrap();
    }
    let bucket = tx.get_bucket(1usize.to_be_bytes()).unwrap();
    let count = bucket.get_kv("size").unwrap();
    let count = usize!(count.value());
    tx.commit().map_err(|_| "create root false")?;
    Ok(count)
}

fn dbfs_create_root_dentry(inode: Arc<Inode>) -> StrResult<Arc<DirEntry>> {
    let dentry = DirEntry::new(
        DirFlags::empty(),
        inode,
        DirEntryOps::empty(),
        Weak::new(),
        "/",
    );
    let dentry = Arc::new(dentry);
    dentry.access_inner().parent = Arc::downgrade(&dentry);
    Ok(dentry)
}
