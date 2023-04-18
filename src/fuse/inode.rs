use downcast::_std::time::SystemTime;
use fuser::{FileAttr, Request};
use rvfs::warn;
use crate::common::{DbfsPermission, DbfsTimeSpec};

use crate::inode::{dbfs_common_attr, dbfs_common_create, dbfs_common_lookup};

pub fn dbfs_fuse_lookup(parent:u64, name: &str) -> Result<FileAttr, ()> {
   warn!("dbfs_fuse_lookup(parent:{},name:{})",parent,name);
   dbfs_common_lookup(parent as usize, name).map(|x|x.into())
}

pub fn dbfs_fuse_getattr(ino: u64) -> Result<FileAttr, ()>{
   warn!("dbfs_fuse_getattr(ino:{})",ino);
   dbfs_common_attr(ino as usize).map(|x|x.into())
}

pub fn dbfs_fuse_create(req:&Request<'_>,parent:u64, name: &str, mode: u32, _flags: i32)->Result<FileAttr,()>  {
   let permission = DbfsPermission::from_bits_truncate(mode as u16);
   let uid = req.uid();
   let gid = req.gid();
   let ctime = DbfsTimeSpec::from(SystemTime::now()).into();
   let res = dbfs_common_create(parent as usize,name,uid,gid,ctime,permission,None);
   if res.is_err() {
       return Err(())
   }
   let inode_number = res.unwrap();
   let attr = dbfs_common_attr(inode_number).unwrap();
    Ok(attr.into())
}