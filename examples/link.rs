use std::sync::Arc;

use dbfs2::{DBFS, SLICE_SIZE};
use jammdb::{
    memfile::{FakeMap, FileOpenOptions},
    DB,
};
use rvfs::{
    dentry::Dirent64Iterator,
    file::{vfs_mkdir, vfs_open_file, vfs_readdir, vfs_write_file, File, FileMode, OpenFlags},
    init_process_info,
    link::{vfs_link, vfs_unlink},
    mount::{do_mount, MountFlags},
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
    let _file = vfs_open_file::<FakeFSC>("/", OpenFlags::O_RDWR, FileMode::FMODE_WRITE).unwrap();
    vfs_mkdir::<FakeFSC>("/db", FileMode::FMODE_WRITE).unwrap();
    let _db = do_mount::<FakeFSC>("block", "/db", "dbfs", MountFlags::empty(), None).unwrap();
    let f1_file = vfs_open_file::<FakeFSC>(
        "/db/f1",
        OpenFlags::O_RDWR | OpenFlags::O_CREAT,
        FileMode::FMODE_WRITE,
    )
    .unwrap();
    // println!("{:#?}",file);
    println!("--------------");
    vfs_link::<FakeFSC>("/db/f1", "/db/f3").unwrap();
    println!("{f1_file:#?}");
    let root = vfs_open_file::<FakeFSC>("/db", OpenFlags::O_RDWR, FileMode::FMODE_WRITE).unwrap();

    readdir(root.clone());

    vfs_unlink::<FakeFSC>("/db/f1").unwrap();
    readdir(root.clone());
    vfs_write_file::<FakeFSC>(f1_file, b"hello world", 0)
        .is_err()
        .then(|| {
            println!("write error, because it has been unlinked");
        });
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

fn readdir(dir: Arc<File>) {
    let len = vfs_readdir(dir.clone(), &mut [0; 0]).unwrap();
    assert!(len > 0);
    let mut dirents = vec![0u8; len];

    let r = vfs_readdir(dir, &mut dirents[..]).unwrap();
    assert_eq!(r, len);
    Dirent64Iterator::new(&dirents[..]).for_each(|x| {
        println!("{} {:?} {}", x.get_name(), x.type_, x.ino);
    });
}
