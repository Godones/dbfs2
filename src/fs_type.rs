use alloc::boxed::Box;
use alloc::sync::Arc;
use rvfs::{DataOps, FileSystemAttr, FileSystemType, MountFlags, StrResult, SuperBlock};
use alloc::vec;
use spin::Mutex;

const DBFS_TYPE :FileSystemType = FileSystemType{
    name: "dbfs",
    fs_flags: FileSystemAttr::FS_REQUIRES_DEV,
    super_blk_s: vec![],
    get_super_blk: dbfs_get_super_blk,
    kill_super_blk: dbfs_kill_super_blk,
};


fn dbfs_get_super_blk(fs_type:Arc<Mutex<FileSystemType>>, flags: MountFlags, dev_name: &str, data: Option<Box<dyn DataOps>>) -> StrResult<Arc<Mutex<SuperBlock>>>{
    unimplemented!()
}
fn dbfs_kill_super_blk(_super_blk: Arc<Mutex<SuperBlock>>) {}
