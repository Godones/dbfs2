use std::sync::Arc;

use dbfs2::{DBFS, SLICE_SIZE};
use jammdb::{
    memfile::{FakeMap, FileOpenOptions},
    DB,
};
use rvfs::{
    dentry::{vfs_truncate, vfs_truncate_by_file},
    file::{vfs_mkdir, vfs_open_file, vfs_read_file, vfs_write_file, FileMode, OpenFlags},
    init_process_info,
    mount::{do_mount, MountFlags},
    stat::vfs_getattr_by_file,
    superblock::register_filesystem,
    FakeFSC,
};

fn main() {
    env_logger::init();
    let db = DB::open::<FileOpenOptions, _>(Arc::new(FakeMap), "my-database.db").unwrap();
    init_db(&db);
    dbfs2::init_dbfs(db);
    let mnt = rvfs::mount_rootfs();
    init_process_info(mnt);
    register_filesystem(DBFS).unwrap();
    vfs_mkdir::<FakeFSC>("/db", FileMode::FMODE_WRITE).unwrap();
    let file = vfs_open_file::<FakeFSC>(
        "/file1",
        OpenFlags::O_RDWR | OpenFlags::O_CREAT,
        FileMode::FMODE_WRITE,
    )
    .unwrap();
    println!("file1:{file:#?}");
    let _db = do_mount::<FakeFSC>("block", "/db", "dbfs", MountFlags::empty(), None).unwrap();
    // println!("db mnt:{:#?}", db);

    let dbf1 = vfs_open_file::<FakeFSC>(
        "/db/f1",
        OpenFlags::O_RDWR | OpenFlags::O_CREAT,
        FileMode::FMODE_WRITE,
    )
    .unwrap();
    vfs_write_file::<FakeFSC>(dbf1.clone(), b"hello world", 0).unwrap();
    let stat = vfs_getattr_by_file(dbf1.clone()).unwrap();
    println!("stat:{stat:#?}"); // size == 11
    vfs_truncate_by_file(dbf1.clone(), 5).unwrap();
    let stat = vfs_getattr_by_file(dbf1.clone()).unwrap();
    println!("stat:{stat:#?}"); // size == 5
    let mut buf = [0u8; 2048];
    let r = vfs_read_file::<FakeFSC>(dbf1.clone(), &mut buf, 0).unwrap();
    println!("read:{}", std::str::from_utf8(&buf[..r]).unwrap());
    vfs_truncate::<FakeFSC>("/db/f1", 20).unwrap();
    let stat = vfs_getattr_by_file(dbf1.clone()).unwrap();
    println!("stat:{stat:#?}"); // size == 20
    let r = vfs_read_file::<FakeFSC>(dbf1.clone(), &mut buf, 0).unwrap();
    println!("read byte:{}, content:{:?}", r, &buf[..r]); //20
    vfs_write_file::<FakeFSC>(dbf1.clone(), b"hello world", 0).unwrap();
    let stat = vfs_getattr_by_file(dbf1.clone()).unwrap();
    println!("stat:{stat:#?}"); // size == 20

    let r = vfs_write_file::<FakeFSC>(dbf1.clone(), b"hello world", 1024).unwrap();
    println!("write byte:{r}"); // 11

    let stat = vfs_getattr_by_file(dbf1.clone()).unwrap();
    println!("stat:{stat:#?}"); // size == 1024 + 11

    let r = vfs_read_file::<FakeFSC>(dbf1, &mut buf, 0).unwrap();
    println!("read {r} bytes"); // == 1024 + 11 = 1035
}

fn init_db(db: &DB) {
    let tx = db.tx(true).unwrap();
    let bucket = tx.get_or_create_bucket("super_blk").unwrap();
    bucket.put("continue_number", 1usize.to_be_bytes()).unwrap();
    bucket.put("magic", 1111u32.to_be_bytes()).unwrap();
    bucket
        .put("blk_size", (SLICE_SIZE as u32).to_be_bytes())
        .unwrap();
    bucket
        .put("disk_size", (1024 * 1024 * 16u64).to_be_bytes())
        .unwrap(); //16MB
    tx.commit().unwrap()
}
