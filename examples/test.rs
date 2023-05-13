use std::sync::Arc;
use std::time::SystemTime;
use jammdb::{Data, DB};
use dbfs2::fuse::mkfs::{FakeMMap, MyOpenOptions};
use dbfs2::SLICE_SIZE;


fn main() {
    // let db = DB::open::<MyOpenOptions<{ 3 * 1024 * 1024 * 1024 }>, _>(Arc::new(FakeMMap), "my-database.db").unwrap(); // TODO: error handling
    // {
    //     let start = SystemTime::now();
    //     let tx = db.tx(true).unwrap();
    //     let new_inode = tx.get_or_create_bucket("inode").unwrap();
    //     new_inode.put("size", (1usize).to_be_bytes()).unwrap();
    //     new_inode.put("hard_links", (1usize).to_be_bytes()).unwrap();
    //     new_inode.put("uid", (1usize).to_be_bytes()).unwrap();
    //     new_inode.put("gid", (1usize).to_be_bytes()).unwrap();
    //     // set time
    //     new_inode.put("atime", (1usize).to_be_bytes()).unwrap();
    //     new_inode.put("mtime", (1usize).to_be_bytes()).unwrap();
    //     new_inode.put("ctime", (1usize).to_be_bytes()).unwrap();
    //     new_inode.put("block_size", (SLICE_SIZE as u32).to_be_bytes()).unwrap();
    //     let data = vec![1u8;8192];
    //     for i in 0usize..1024*1024*1024/8192{
    //         new_inode.put(i.to_be_bytes(), data.clone()).unwrap();
    //     }
    //     tx.commit().unwrap();
    //     let end = SystemTime::now();
    //     println!("time:{:?}", end.duration_since(start).unwrap());
    // }

    let db = DB::open::<MyOpenOptions<{ 3 * 1024 * 1024 * 1024 }>, _>(Arc::new(FakeMMap), "my-database1.db").unwrap(); // TODO: error handling
    // {
    //
    //     const PER_SIZE: usize = 8192*2*2;
    //
    //     let start = SystemTime::now();
    //     let tx = db.tx(true).unwrap();
    //     let new_inode = tx.get_or_create_bucket("inode1").unwrap();
    //     new_inode.put("size", (1usize).to_be_bytes()).unwrap();
    //     new_inode.put("hard_links", (1usize).to_be_bytes()).unwrap();
    //     new_inode.put("uid", (1usize).to_be_bytes()).unwrap();
    //     new_inode.put("gid", (1usize).to_be_bytes()).unwrap();
    //     // set time
    //     new_inode.put("atime", (1usize).to_be_bytes()).unwrap();
    //     new_inode.put("mtime", (1usize).to_be_bytes()).unwrap();
    //     new_inode.put("ctime", (1usize).to_be_bytes()).unwrap();
    //
    //     new_inode.put("block_size", (SLICE_SIZE as u32).to_be_bytes()).unwrap();
    //     let data = vec![1u8;PER_SIZE];
    //     for i in 0usize..1024*1024*1024/PER_SIZE{
    //         new_inode.put(i.to_be_bytes(), data.clone()).unwrap();
    //     }
    //     tx.commit().unwrap();
    //     let end = SystemTime::now();
    //     println!("time:{:?}", end.duration_since(start).unwrap());
    //     println!("throughput:{:?}",1024.0/ end.duration_since(start).unwrap().as_secs_f64());
    // }

    const PER_SIZE: usize = 1024*1024*128-1;
    let buf = vec![1u8;PER_SIZE];
    let mut new_buf = vec![0u8;PER_SIZE];
    let start = SystemTime::now();
    unsafe {
        new_buf.as_mut_ptr().copy_from(buf.as_ptr(), PER_SIZE);
    }
    let end = SystemTime::now();
    println!("time:{:?}", end.duration_since(start).unwrap());
    assert_eq!(buf, new_buf);

    let start = SystemTime::now();
    unsafe {
        (new_buf.as_mut_ptr() as *mut u128)
            .copy_from_nonoverlapping(buf.as_ptr() as *const u128, PER_SIZE/16);
    }
    let end = SystemTime::now();
    println!("time:{:?}", end.duration_since(start).unwrap());
    assert_eq!(buf, new_buf)


}




