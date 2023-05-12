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

    reference(&db,&vec![1u8;1*128*1024]);
    {
        let tx = db.tx(false).unwrap();
        let inode = tx.get_bucket("inode").unwrap();
        inode.kv_pairs().for_each(|x|{
            println!("{:?}: {}",x.key(),x.value().len());
        })
    }

}



fn reference(db:&DB,buf:&[u8]){
    let tx = db.tx(true).unwrap();
    const PER_SIZE: usize = 8192*2*2;
    let mut count  = 0;
    let len = buf.len();

    let inode = tx.get_or_create_bucket("inode").unwrap();
    let mut start = 0usize;
    let offset = 0;
    loop {
        let min = std::cmp::min(PER_SIZE, len - count);
        let data = &buf[offset..offset+min];
        inode.put(start.to_be_bytes(), data).unwrap();
        count += PER_SIZE;
        if count >= len{
            break;
        }
        start += 1;
    }
    tx.commit().unwrap();
}

