use std::{sync::Arc, time::SystemTime};

use dbfs2::{
    fuse::mkfs::{FakeMMap, FakePath, MyOpenOptions},
    BUCKET_DATA_SIZE, SLICE_SIZE,
};
use jammdb::{Data, DB};

fn main() {
    let path = FakePath::new("my-database1.db");

    const PER_SIZE: usize = 1024 * 32;
    const FILE_SIZE: usize = 1024 * 1024 * 1024 * 2; //write 8GB
    const WRITE_BK: usize = 1024 * 1024;

    let db = DB::open::<MyOpenOptions<{ 20 * 1024 * 1024 * 1024 }>, _>(Arc::new(FakeMMap), path)
        .unwrap(); // TODO: error handling
    {
        let tx = db.tx(true).unwrap();
        let bucket = tx.get_bucket("inode1");
        if bucket.is_err() {
            let new_inode = tx.create_bucket("inode1").unwrap();
            new_inode.put("size", (1usize).to_be_bytes()).unwrap();
            new_inode.put("hard_links", (1usize).to_be_bytes()).unwrap();
            new_inode.put("uid", (1usize).to_be_bytes()).unwrap();
            new_inode.put("gid", (1usize).to_be_bytes()).unwrap();
            // set time
            new_inode.put("atime", (1usize).to_be_bytes()).unwrap();
            new_inode.put("mtime", (1usize).to_be_bytes()).unwrap();
            new_inode.put("ctime", (1usize).to_be_bytes()).unwrap();
            new_inode
                .put("block_size", (SLICE_SIZE as u32).to_be_bytes())
                .unwrap();
        }
        tx.commit().unwrap();

        let start_time = SystemTime::now();

        #[cfg(feature = "write1")]
        {
            let data = vec![1u8; PER_SIZE];
            let mut start = 0usize;
            loop {
                let tx = db.tx(true).unwrap();
                let new_inode = tx.get_bucket("inode1").unwrap();
                for i in start..start + WRITE_BK / PER_SIZE {
                    let ptr = data.as_ptr();
                    let data = unsafe { std::slice::from_raw_parts(ptr, PER_SIZE) };
                    new_inode.put(i.to_be_bytes(), data).unwrap();
                }
                tx.commit().unwrap();
                start += WRITE_BK / PER_SIZE;
                if start >= FILE_SIZE / PER_SIZE {
                    break;
                }
            }
        }
        #[cfg(feature = "write1")]
        {
            let data = vec![1u8; PER_SIZE];
            let mut start = 0usize;
            let mut count = 0;
            for i in 0..FILE_SIZE / BUCKET_DATA_SIZE {
                loop {
                    let tx = db.tx(true).unwrap();
                    let new_inode = tx.get_bucket("inode1").unwrap();
                    let bucket = new_inode.get_or_create_bucket(i.to_be_bytes()).unwrap();
                    for i in start..start + WRITE_BK / PER_SIZE {
                        let ptr = data.as_ptr();
                        let data = unsafe { std::slice::from_raw_parts(ptr, PER_SIZE) };
                        bucket.put(i.to_be_bytes(), data).unwrap();
                    }
                    tx.commit().unwrap(); //commit every 1MB
                    start += WRITE_BK / PER_SIZE;
                    if start - count == BUCKET_DATA_SIZE / PER_SIZE {
                        println!("start:{:?}", start);
                        count = start;
                        break;
                    }
                }
            }
            println!("count:{:?}", count);
            assert_eq!(count, FILE_SIZE / PER_SIZE);

            let end = SystemTime::now();
            println!("time:{:?}", end.duration_since(start_time).unwrap());
            println!("key size:{:?}", FILE_SIZE / PER_SIZE);
            println!(
                "throughput:{:?}MB/s",
                FILE_SIZE as f64
                    / 1024.0
                    / 1024.0
                    / end.duration_since(start_time).unwrap().as_secs_f64()
            );
        }

        // #[cfg(feature = "read")]
        {
            let mut data = vec![1u8; PER_SIZE];
            let mut start = 0usize;
            let mut count = 0;
            for i in 0..FILE_SIZE / BUCKET_DATA_SIZE {
                loop {
                    let tx = db.tx(false).unwrap();
                    let new_inode = tx.get_bucket("inode1").unwrap();
                    let bucket = new_inode.get_bucket(i.to_be_bytes()).unwrap();
                    for i in start..start + WRITE_BK / PER_SIZE {
                        let value = bucket.get_kv(i.to_be_bytes()).unwrap();
                        assert_eq!(value.value().len(), PER_SIZE);
                        data.copy_from_slice(value.value());
                    }
                    start += WRITE_BK / PER_SIZE;
                    if start - count == BUCKET_DATA_SIZE / PER_SIZE {
                        println!("start:{:?}", start);
                        count = start;
                        break;
                    }
                }
            }
            println!("count:{:?}", count);
            assert_eq!(count, FILE_SIZE / PER_SIZE);

            let end = SystemTime::now();
            println!("time:{:?}", end.duration_since(start_time).unwrap());
            println!("key size:{:?}", FILE_SIZE / PER_SIZE);
            println!(
                "throughput:{:?}MB/s",
                FILE_SIZE as f64
                    / 1024.0
                    / 1024.0
                    / end.duration_since(start_time).unwrap().as_secs_f64()
            );
        }
    }
    {
        let tx = db.tx(false).unwrap();
        let bucket = tx.get_bucket("inode1").unwrap();
        bucket.cursor().for_each(|data| match data {
            Data::Bucket(b) => {
                println!("bucket:{:?}", b.name());
            }
            Data::KeyValue(kv) => {
                println!("key:{:?}", kv.key());
            }
        })
    }
}
