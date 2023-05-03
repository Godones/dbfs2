use dbfs2::{DBFS, SLICE_SIZE};
use jammdb::memfile::{FakeMap, FileOpenOptions};
use jammdb::DB;
use rvfs::file::{vfs_mkdir, vfs_open_file, vfs_read_file, vfs_write_file, FileMode, OpenFlags};
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
    println!("file1:{file:#?}");
    let _db = do_mount::<FakeFSC>("block", "/db", "dbfs", MountFlags::empty(), None).unwrap();
    // println!("db mnt:{:#?}", db);
    let f1_file = vfs_open_file::<FakeFSC>(
        "/db/f1",
        OpenFlags::O_RDWR | OpenFlags::O_CREAT,
        FileMode::FMODE_WRITE,
    )
    .unwrap();
    println!("write to file 128MB");
    let mut buf_read: Vec<u8> = vec![1u8; 1024 * 1024 * 128];
    // rand data
    let buf: Vec<u8> = (0..1024 * 1024 * 128).map(|_| rand::random::<u8>()).collect();
    let res = vfs_write_file::<FakeFSC>(f1_file.clone(), &buf, 0).unwrap();
    println!("write res:{:#?}", res);
    assert_eq!(res, buf.len());

    let read = vfs_read_file::<FakeFSC>(f1_file, &mut buf_read, 0).unwrap();
    println!("read res:{:#?}", read);
    assert_eq!(read, buf.len());
}
fn init_db(db: &DB) {
    let tx = db.tx(true).unwrap();
    let bucket = tx.get_or_create_bucket("super_blk").unwrap();
    bucket.put("continue_number", 1usize.to_be_bytes()).unwrap();
    bucket.put("magic", 1111u32.to_be_bytes()).unwrap();
    bucket.put("blk_size", (SLICE_SIZE as u32).to_be_bytes()).unwrap();
    bucket
        .put("disk_size", (1024 * 1024 * 16u64).to_be_bytes())
        .unwrap(); //16MB
    tx.commit().unwrap()
}
