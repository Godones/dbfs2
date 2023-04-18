use alloc::vec;
use fuser::{ReplyDirectory};
use rvfs::warn;
use crate::common::DbfsDirEntry;
use crate::file::{dbfs_common_read, dbfs_common_readdir, dbfs_common_write};

pub fn dbfs_fuse_read(ino:u64, offset:i64, _size:u32, buf:&mut [u8]) -> Result<usize, ()> {
    assert!(offset >= 0);
    dbfs_common_read(ino as usize,buf,offset as u64).map_err(|_|())
}

pub fn dbfs_fuse_write(ino:u64, offset:i64, buf:&[u8]) -> Result<usize, ()> {
    assert!(offset >= 0);
    dbfs_common_write(ino as usize,buf,offset as u64).map_err(|_|())
}


pub fn dbfs_fuse_readdir(ino:u64, offset:i64, mut repl:ReplyDirectory){
    warn!("dbfs_fuse_readdir(ino:{},offset:{})",ino,offset);
    assert!(offset >= 0);
    let mut entries = vec![DbfsDirEntry::default(); 16]; // we read 16 entries at a time
    loop {
        let res = dbfs_common_readdir(ino as usize,&mut entries,offset as u64);
        if res.is_err() {
            repl.error(libc::ENOENT);
            return
        }
        if res.unwrap() == 0 {
            repl.ok();
            return
        }
        for i in 0..res.unwrap(){
            let x = &entries[i];
            if repl.add(x.ino , x.offset as i64 +1, x.kind.into(), x.name.as_str()){
                // buf full
                repl.ok();
                return
            }
        }
    }
}