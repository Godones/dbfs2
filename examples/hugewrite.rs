use dbfs2::DBFS;
use jammdb::memfile::{FakeMap, FileOpenOptions};
use jammdb::DB;
use rvfs::file::{vfs_mkdir, vfs_open_file, vfs_write_file, FileMode, OpenFlags};

use rvfs::mount::{do_mount, MountFlags};

use rvfs::superblock::register_filesystem;
use rvfs::{init_process_info, FakeFSC};
use std::sync::Arc;

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
    println!("file1:{:#?}", file);
    let _db = do_mount::<FakeFSC>("block", "/db", "dbfs", MountFlags::empty(), None).unwrap();
    // println!("db mnt:{:#?}", db);

    let f1_file = vfs_open_file::<FakeFSC>(
        "/db/f1",
        OpenFlags::O_RDWR | OpenFlags::O_CREAT,
        FileMode::FMODE_WRITE,
    )
    .unwrap();

    println!("write to file 1MB");
    let buf = [0u8; 1024];
    for i in 0..1024 {
        vfs_write_file::<FakeFSC>(f1_file.clone(), &buf, i * 1024).unwrap();
    }
}
fn init_db(db: &DB) {
    let tx = db.tx(true).unwrap();
    let bucket = tx.get_or_create_bucket("super_blk").unwrap();
    bucket.put("continue_number", 0usize.to_le_bytes()).unwrap();
    bucket.put("magic", 1111u32.to_le_bytes()).unwrap();
    bucket.put("blk_size", 512u32.to_le_bytes()).unwrap();
    tx.commit().unwrap()
}
