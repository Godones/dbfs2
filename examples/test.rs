use std::io::Write;
use memmap2::MmapOptions;
fn main() {
    // let _db = DB::open::<MyOpenOptions<{ 3 * 1024 * 1024 * 1024 }>, _>(Arc::new(FakeMMap), "my-database1.db").unwrap(); // TODO: error handling
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

    // const PER_SIZE: usize = 1024*32;
    // let buf = vec![1u8;PER_SIZE];
    // let mut new_buf = vec![0u8;PER_SIZE];
    // let start = SystemTime::now();
    // unsafe {
    //     new_buf.as_mut_ptr().copy_from(buf.as_ptr(), PER_SIZE);
    // }
    // let end = SystemTime::now();
    // println!("time:{:?}", end.duration_since(start).unwrap());
    // assert_eq!(buf, new_buf);
    //
    // let start = SystemTime::now();
    // unsafe {
    //     (new_buf.as_mut_ptr() as *mut u128)
    //         .copy_from_nonoverlapping(buf.as_ptr() as *const u128, PER_SIZE/16);
    // }
    // let end = SystemTime::now();
    // println!("time:{:?}", end.duration_since(start).unwrap());
    // assert_eq!(buf, new_buf)
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open("my-database1.db")
        .unwrap();


    file.set_len(16*1024*1024*1024).unwrap();//4G
    println!("file size:{:?}",file.metadata().unwrap().len());
    let data = vec![1u8;4096*1024];
    file.write(data.as_ref()).unwrap();

    let mut map = MmapOptions::new();
        map
        .populate()
        .len(16*1024*1024*1024);

    let file_buf = unsafe { map.map(&file) }.unwrap();
    loop {
        let start = rand::random::<usize>() % (4*1024*1024*1024-4096);
        let t = &file_buf[start..start+4096];
        println!("{:?}",t[0]);
    }
    std::fs::remove_file("my-database1.db").unwrap();
}


