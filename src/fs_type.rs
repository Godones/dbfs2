use crate::file::DBFS_DIR_FILE_OPS;
use crate::inode::{DBFS_DIR_INODE_OPS, DBFS_INODE_NUMBER};
use crate::{clone_db};
use alloc::boxed::Box;
use alloc::string::ToString;
use alloc::sync::{Arc, Weak};
use alloc::vec;
use rvfs::{
    create_tmp_inode_from_sb_blk, find_super_blk, DataOps, DirEntry, FileSystemAttr,
    FileSystemType, Inode, InodeMode, MountFlags, StrResult, SuperBlock, SuperBlockOps,
};
use spin::Mutex;

pub const DBFS_TYPE: FileSystemType = FileSystemType {
    name: "dbfs",
    fs_flags: FileSystemAttr::empty(),
    super_blk_s: vec![],
    get_super_blk: dbfs_get_super_blk,
    kill_super_blk: dbfs_kill_super_blk,
};

fn dbfs_get_super_blk(
    fs_type: Arc<Mutex<FileSystemType>>,
    flags: MountFlags,
    dev_name: &str,
    data: Option<Box<dyn DataOps>>,
) -> StrResult<Arc<Mutex<SuperBlock>>> {
    let compare =
        |sb_blk: Arc<Mutex<SuperBlock>>| -> bool { sb_blk.lock().blk_dev_name.as_str() == dev_name };
    let find_sb_blk = find_super_blk(fs_type.clone(), Some(&compare));
    let sb_blk = match find_sb_blk {
        // 找到了旧超级快
        Ok(_) => Err("super block exist, can't create new one"),
        Err(_) => {
            // 没有找到旧超级快需要重新分配
            info!("create new super block for ramfs");
            let sb_blk = dbfs_create_simple_super_blk(fs_type.clone(), flags, dev_name, data)?;
            dbfs_fill_super_block(sb_blk.clone())?;
            fs_type.lock().insert_super_blk(sb_blk.clone());
            Ok(sb_blk)
        }
    };
    sb_blk
}
fn dbfs_kill_super_blk(_super_blk: Arc<Mutex<SuperBlock>>) {}

fn dbfs_create_simple_super_blk(
    fs_type: Arc<Mutex<FileSystemType>>,
    flags: MountFlags,
    dev_name: &str,
    data: Option<Box<dyn DataOps>>,
) -> StrResult<Arc<Mutex<SuperBlock>>> {
    let sb_blk = SuperBlock {
        dev_desc: 0,
        device: None,
        block_size: 0,
        dirty_flag: false,
        file_max_bytes: 0,
        mount_flag: flags,
        magic: 0,
        file_system_type: Arc::downgrade(&fs_type),
        super_block_ops: SuperBlockOps::empty(),
        root: Arc::new(Mutex::new(DirEntry::empty())),
        dirty_inode: vec![],
        sync_inode: vec![],
        files: vec![],
        blk_dev_name: dev_name.to_string(),
        data,
    };
    Ok(Arc::new(Mutex::new(sb_blk)))
}

// TODO 从数据库中读取超级快信息/元数据信息
fn dbfs_fill_super_block(sb_blk: Arc<Mutex<SuperBlock>>) -> StrResult<()> {
    // 1.从磁盘读取超级快，dbfs没有超级快的结构，但保存了一部分信息在数据库中
    // let db = clone_db();
    //
    // let tx = db.tx(false);
    // if tx.is_err() {
    //     return Err("dbfs_fill_super_block: get db tx failed");
    // }
    // let tx = tx.unwrap();
    // let bucket = tx.get_bucket("super_blk");
    // if bucket.is_err() {
    //     return Err("dbfs_fill_super_block: get bucket failed");
    // }
    // let bucket = bucket.unwrap();
    // 生成inode对象与direntry对象
    let inode = dbfs_create_root_inode(sb_blk.clone())?;
    let dentry = dbfs_create_root_dentry(inode)?;
    sb_blk.lock().root = dentry;
    Ok(())
}
// create root inode for dbfs
fn dbfs_create_root_inode(sb_blk: Arc<Mutex<SuperBlock>>) -> StrResult<Arc<Mutex<Inode>>> {
    let db = clone_db();
    let tx = db.tx(true).unwrap();
    let _ = tx.get_or_create_bucket("0").unwrap();
    let first_number = DBFS_INODE_NUMBER.fetch_add(1, core::sync::atomic::Ordering::SeqCst);
    assert_eq!(first_number, 0);
    // create a inode from super block
    let inode = create_tmp_inode_from_sb_blk(sb_blk.clone())?;
    let mut inode_lk = inode.lock();
    inode_lk.mode = InodeMode::S_DIR;
    inode_lk.blk_count = 0;
    // set the number of inode to 0
    inode_lk.number = 0;
    // TODO 设置uid/gid
    inode_lk.inode_ops = DBFS_DIR_INODE_OPS;
    inode_lk.file_ops = DBFS_DIR_FILE_OPS;
    inode_lk.hard_links = 0;
    drop(inode_lk);
    Ok(inode)
}
fn dbfs_create_root_dentry(inode: Arc<Mutex<Inode>>) -> StrResult<Arc<Mutex<DirEntry>>> {
    let dentry = DirEntry::new(inode, Weak::new(), "/");
    // TODO 初始化 ops
    Ok(Arc::new(Mutex::new(dentry)))
}
