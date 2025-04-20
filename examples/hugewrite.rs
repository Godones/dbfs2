use std::sync::Arc;

use dbfs2::{
    fuse::{
        mkfs::{FakeMMap, FakePath, MyOpenOptions},
        sblk::dbfs_fuse_destroy,
    },
    DBFS, SLICE_SIZE,
};
use jammdb::{PathLike, DB};
use rvfs::{
    file::{vfs_mkdir, vfs_open_file, vfs_read_file, vfs_write_file, FileMode, OpenFlags},
    init_process_info,
    mount::{do_mount, MountFlags},
    stat::vfs_getattr_by_file,
    superblock::register_filesystem,
    FakeFSC,
};

fn main() {
    env_logger::init();
    let path = FakePath::new("my-database1.db");
    let flag = path.exists();
    let db =
        DB::open::<MyOpenOptions<{ 20 * 1024 * 1024 * 1024 }>, _>(Arc::new(FakeMMap), path.clone())
            .unwrap(); // TODO: error handling
    if !flag {
        init_db(&db);
    }
    dbfs2::init_dbfs(db);
    let mnt = rvfs::mount_rootfs();
    init_process_info(mnt);
    register_filesystem(DBFS).unwrap();
    vfs_mkdir::<FakeFSC>("/db", FileMode::FMODE_WRITE).unwrap();
    let _file = vfs_open_file::<FakeFSC>(
        "/file1",
        OpenFlags::O_RDWR | OpenFlags::O_CREAT,
        FileMode::FMODE_WRITE,
    )
    .unwrap();
    // println!("file1:{file:#?}");
    let db = do_mount::<FakeFSC>("block", "/db", "dbfs", MountFlags::empty(), None).unwrap();
    println!("db mnt:{:#?}", db);
    let f1_file = vfs_open_file::<FakeFSC>(
        "/db/f1",
        OpenFlags::O_RDWR | OpenFlags::O_CREAT,
        FileMode::FMODE_WRITE,
    )
    .unwrap();
    println!("f1_file:{:#?}", f1_file);
    println!("write to file 128MB");
    let mut buf_read: Vec<u8> = vec![1u8; 1024 * 1024 * 128];
    // rand data
    let buf: Vec<u8> = (0..1024 * 1024 * 128)
        .map(|_| rand::random::<u8>())
        .collect();
    let res = vfs_write_file::<FakeFSC>(f1_file.clone(), &buf, 0).unwrap();
    println!("write res:{:#?}", res);
    assert_eq!(res, buf.len());

    let read = vfs_read_file::<FakeFSC>(f1_file.clone(), &mut buf_read, 0).unwrap();
    println!("read res:{:#?}", read);
    assert_eq!(read, buf.len());

    for i in 0..buf.len() {
        assert_eq!(buf[i], buf_read[i]);
    }
    println!("{:?}", &buf[..10]);
    println!("{:?}", &buf_read[..10]);

    let stat = vfs_getattr_by_file(f1_file).unwrap();
    println!("stat:{:#?}", stat);
    // const FILE_SIZE: usize = 1024 * 1024 * 1024 * 4; //write 8GB
    // const BK: usize = 1024*1024;
    #[cfg(feature = "write")]
    {
        let start = SystemTime::now();
        let buf = vec![1u8; BK];
        // write 2GB
        for i in 0..FILE_SIZE / BK {
            if i == 0 {
                FLAG.store(true, std::sync::atomic::Ordering::SeqCst);
            }
            let res = vfs_write_file::<FakeFSC>(f1_file.clone(), &buf, (i * BK) as u64).unwrap();
            assert_eq!(res, BK);
            if i == 0 {
                FLAG.store(false, std::sync::atomic::Ordering::SeqCst);
            }
        }
        let end = SystemTime::now();
        println!("write 4GB cost:{:#?}", end.duration_since(start).unwrap());
        println!(
            "Throughput: {} MB/s",
            FILE_SIZE as f64 / 1024.0 / 1024.0 / end.duration_since(start).unwrap().as_secs_f64()
        );
    }

    #[cfg(feature = "read")]
    {
        let start = SystemTime::now();
        let mut buf = vec![1u8; BK];
        for i in 0..FILE_SIZE / BK {
            if i == 0 || i == FILE_SIZE / BK {
                FLAG.store(true, std::sync::atomic::Ordering::SeqCst);
            }
            let res = vfs_read_file::<FakeFSC>(f1_file.clone(), &mut buf, (i * BK) as u64).unwrap();
            assert_eq!(res, BK);
            assert_eq!(res, BK);
            if i == 0 || i == FILE_SIZE / BK {
                FLAG.store(false, std::sync::atomic::Ordering::SeqCst);
            }
            if i % 512 == 0 {
                println!(
                    "read 4GB cost:{:#?}",
                    SystemTime::now().duration_since(start).unwrap()
                );
                println!(
                    "Throughput: {} MB/s",
                    ((i + 1) * BK) as f64
                        / 1024.0
                        / 1024.0
                        / SystemTime::now()
                            .duration_since(start)
                            .unwrap()
                            .as_secs_f64()
                );
            }
        }
        let end = SystemTime::now();
        println!("read 4GB cost:{:#?}", end.duration_since(start).unwrap());
        println!(
            "Throughput: {} MB/s",
            FILE_SIZE as f64 / 1024.0 / 1024.0 / end.duration_since(start).unwrap().as_secs_f64()
        );
    }
    dbfs_fuse_destroy();
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
        .put("disk_size", (1024 * 1024 * 1024 * 20u64).to_be_bytes())
        .unwrap(); //16MB
    tx.commit().unwrap()
}
