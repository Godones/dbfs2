use std::{io::Write, println};

use crate::{clone_db, fs_type::dbfs_common_umount, fuse::mkfs::FakeFile};

pub fn dbfs_fuse_destroy() {
    println!("dbfs_fuse_destroy");
    dbfs_common_umount().unwrap();
    {
        let db = clone_db();
        let mut file = db.file();
        println!("Get file from db");
        let file = &mut file.file;
        let fake_file = file.downcast_mut::<FakeFile>().unwrap();
        fake_file.file.sync_all().unwrap();
        fake_file.file.flush().unwrap();
        println!("sync_all and flush");
    }
    let _db = clone_db();
    //test_dbfs(&db);
    println!("dbfs_fuse_destroy end");
}
