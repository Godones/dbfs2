use dbfs2::DBFS;
use jammdb::memfile::{FakeMap, FileOpenOptions};
use jammdb::DB;
use rvfs::file::{vfs_mkdir, vfs_open_file, vfs_readdir, FileMode, OpenFlags};
use rvfs::link::vfs_link;
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
    let file = vfs_open_file::<FakeFSC>("/", OpenFlags::O_RDWR, FileMode::FMODE_WRITE).unwrap();
    vfs_mkdir::<FakeFSC>("/db", FileMode::FMODE_WRITE).unwrap();
    let file = vfs_open_file::<FakeFSC>(
        "/file1",
        OpenFlags::O_RDWR | OpenFlags::O_CREAT,
        FileMode::FMODE_WRITE,
    )
    .unwrap();
    let db = do_mount::<FakeFSC>("block", "/db", "dbfs", MountFlags::empty(), None).unwrap();

    let f1_file = vfs_open_file::<FakeFSC>(
        "/db/f1",
        OpenFlags::O_RDWR | OpenFlags::O_CREAT,
        FileMode::FMODE_WRITE,
    )
    .unwrap();
    // println!("{:#?}",file);
    println!("--------------");
    vfs_link::<FakeFSC>("/db/f1", "/db/f3").unwrap();
    println!("{:#?}", f1_file);
    let root = vfs_open_file::<FakeFSC>("/db", OpenFlags::O_RDWR, FileMode::FMODE_WRITE).unwrap();
    vfs_readdir(root.clone()).unwrap().for_each(|x| {
        println!("{:#?}", x);
    });
}

fn init_db(db: &DB) {
    let tx = db.tx(true).unwrap();
    let bucket = tx.get_or_create_bucket("super_blk").unwrap();
    bucket.put("continue_number", 0usize.to_le_bytes()).unwrap();
    bucket.put("magic", 1111u32.to_le_bytes()).unwrap();
    bucket.put("blk_size", 512u32.to_le_bytes()).unwrap();
    tx.commit().unwrap()
}
