use std::sync::Arc;

use dbfs2::{DBFS, SLICE_SIZE};
use jammdb::{
    memfile::{FakeMap, FileOpenOptions},
    DB,
};
use rvfs::{
    dentry::{vfs_rename, vfs_rmdir, Dirent64Iterator},
    file::{
        vfs_mkdir, vfs_open_file, vfs_read_file, vfs_readdir, vfs_write_file, File, FileMode,
        OpenFlags,
    },
    init_process_info,
    link::{vfs_link, vfs_readlink, vfs_symlink},
    mount::{do_mount, MountFlags},
    stat::{vfs_getxattr, vfs_listxattr, vfs_setxattr},
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
    let file = vfs_open_file::<FakeFSC>("/", OpenFlags::O_RDWR, FileMode::FMODE_WRITE).unwrap();
    println!("root: {file:#x?}");
    vfs_mkdir::<FakeFSC>("/db", FileMode::FMODE_WRITE).unwrap();
    let file = vfs_open_file::<FakeFSC>(
        "/file1",
        OpenFlags::O_RDWR | OpenFlags::O_CREAT,
        FileMode::FMODE_WRITE,
    )
    .unwrap();
    println!("file1:{file:#?}");
    let db = do_mount::<FakeFSC>("block", "/db", "dbfs", MountFlags::empty(), None).unwrap();
    println!("db mnt:{db:#?}");

    let f1_file = vfs_open_file::<FakeFSC>(
        "/db/f1",
        OpenFlags::O_RDWR | OpenFlags::O_CREAT,
        FileMode::FMODE_WRITE,
    )
    .unwrap();
    // println!("{:#?}",file);
    vfs_link::<FakeFSC>("/db/f1", "/db/f2").unwrap();
    println!("{f1_file:#?}");
    let root = vfs_open_file::<FakeFSC>("/db", OpenFlags::O_RDWR, FileMode::FMODE_WRITE).unwrap();

    readdir(root.clone());

    let len = vfs_write_file::<FakeFSC>(f1_file.clone(), b"hello world", 0).unwrap();
    println!("len:{len}");
    let mut buf = [0u8; 20];
    let len = vfs_read_file::<FakeFSC>(f1_file.clone(), &mut buf, 0).unwrap();
    println!("len:{len}");
    println!("buf:{}", std::str::from_utf8(&buf).unwrap());

    vfs_setxattr::<FakeFSC>("/db/f1", "note", "the test file".as_bytes()).unwrap();
    vfs_setxattr::<FakeFSC>("/db/f1", "note1", "note something".as_bytes()).unwrap();
    let mut buf = [0u8; 20];
    let len = vfs_listxattr::<FakeFSC>("/db/f1", &mut buf).unwrap();
    println!("len: {len}");
    buf.split(|&x| x == 0)
        .collect::<Vec<&[u8]>>()
        .iter()
        .map(|&x| std::str::from_utf8(x).unwrap())
        .collect::<Vec<&str>>()
        .iter()
        .for_each(|x| {
            if x.is_empty() {
                return;
            }
            println!("attr: {x}");
        });
    let mut buf = [0u8; 20];
    let len = vfs_getxattr::<FakeFSC>("/db/f1", "note1", &mut buf).unwrap();
    println!("len: {len}");
    println!("note: {}", std::str::from_utf8(&buf).unwrap());

    vfs_symlink::<FakeFSC>("/db/f1", "/db/symf1").unwrap();
    let mut buf = [0u8; 10];
    let size = vfs_readlink::<FakeFSC>("/db/symf1", buf.as_mut()).unwrap();
    println!("size: {size}");
    println!("link: {}", std::str::from_utf8(&buf).unwrap());

    let file =
        vfs_open_file::<FakeFSC>("/db/symf1", OpenFlags::O_RDWR, FileMode::FMODE_WRITE).unwrap();
    println!("file:{file:#?}");
    println!("f1_file:{f1_file:#?}");
    assert!(Arc::ptr_eq(&file, &f1_file));

    vfs_mkdir::<FakeFSC>("/db/dir1", FileMode::FMODE_WRITE).unwrap();

    readdir(root.clone());

    vfs_rmdir::<FakeFSC>("db/dir1").unwrap();

    readdir(root.clone());

    vfs_rename::<FakeFSC>("db/f1", "db/f3").unwrap();

    readdir(root.clone());
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
