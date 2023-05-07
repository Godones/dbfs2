use std::alloc::alloc;
use std::fs::{File};

use std::io::{Read};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::thread::{self, JoinHandle};
use jammdb::{Data, DB};

use spin::Mutex;
use dbfs2::fuse::mkfs::{FakeMMap, MyOpenOptions};
use dbfs2::SLICE_SIZE;


static FLAG:AtomicBool = AtomicBool::new(false);

#[derive(Debug)]
pub struct FakeFile{
    file: Arc<Mutex<File>>,
    size: usize,
    thread:Option<JoinHandle<()>>
}

impl Drop for FakeFile{
    fn drop(&mut self) {
        FLAG.store(true,std::sync::atomic::Ordering::Relaxed);
        self.thread.take().unwrap().join().unwrap();
        println!("Thread is over");
    }
}


impl  FakeFile{
    pub fn new(file:Arc<Mutex<File>>) -> Self {
        let meta = file.lock().metadata().unwrap();
        let size = meta.len() as usize;
        let file_t = file.clone();
        let thread = thread::spawn( || {
            let file = file_t;
            while !FLAG.load(std::sync::atomic::Ordering::Relaxed) {
                let meta = file.lock().metadata().unwrap();
                println!("The file size is {}",meta.len());
                thread::sleep(std::time::Duration::from_secs(1));
            }
        });
        FakeFile {
            file:file.clone(),
            size,
            thread:Some(thread),
        }
    }
}


fn main() {
    let db = DB::open::<MyOpenOptions<{ 512 * 1024 * 1024 }>, _>(Arc::new(FakeMMap), "my-database.db").unwrap(); // TODO: error handling
    {
        let tx = db.tx(true).unwrap();
        let new_inode = tx.get_or_create_bucket("inode").unwrap();
        new_inode.put("size", (1usize).to_be_bytes()).unwrap();
        new_inode.put("hard_links", (1usize).to_be_bytes()).unwrap();
        new_inode.put("uid", (1usize).to_be_bytes()).unwrap();
        new_inode.put("gid", (1usize).to_be_bytes()).unwrap();
        // set time
        new_inode.put("atime", (1usize).to_be_bytes()).unwrap();
        new_inode.put("mtime", (1usize).to_be_bytes()).unwrap();
        new_inode.put("ctime", (1usize).to_be_bytes()).unwrap();

        new_inode.put("block_size", (SLICE_SIZE as u32).to_be_bytes()).unwrap();
        new_inode.put("dev", (1usize).to_be_bytes()).unwrap();


        new_inode.put("data:a", "a").unwrap();
        new_inode.put("data:b", "b").unwrap();
        new_inode.put("data:aa", "aa").unwrap();
        new_inode.put("data:ab", "ab").unwrap();
        new_inode.put("data", [1u8; 16]).unwrap();
        tx.commit().unwrap();
    }

    {
        let tx = db.tx(false).unwrap();
        let inode = tx.get_bucket("inode").unwrap();
        let mut c = inode.cursor();
        c.seek("data:aa".as_bytes());
        c.for_each(|data| {
            match data {
                Data::KeyValue(kv) => {
                    let key = core::str::from_utf8(kv.key()).unwrap();
                    let value = kv.value();
                    println!("{}: {:?}", key, value)
                }
                _ => {}
            }
        });
    }

    let tmp = [0u8;16];

    let tx = db.tx(true).unwrap();
    let bucket = tx.get_bucket("inode").unwrap();
    for i in 0..10{
        let key = format!("data:{}",i);
        let kv = bucket.get_kv(key.clone());
        let new_value = if kv.is_some() {
            // kv.unwrap().value()
            let ptr = kv.as_ref().unwrap().value().as_ptr();
            println!("ptr is {:p}",ptr);
            ptr
        } else {
            unsafe { alloc(core::alloc::Layout::from_size_align(16, 1).unwrap())}
        };
        // let ptr = kv.as_ref().unwrap().value().as_ptr();
        let new_value:&'static mut [u8] = unsafe {
            std::slice::from_raw_parts_mut(new_value as *mut u8, 16)
        };
        new_value.copy_from_slice(&[i; 16]);
        // let key = kv.as_ref().unwrap().key().to_vec();
        bucket.put(key, &*new_value).unwrap();
    }
    tx.commit().unwrap();


    println!("--------------------");
    {
        let tx = db.tx(false).unwrap();
        let bucket = tx.get_bucket("inode").unwrap();
        for i in 0..10 {
            let key = format!("data:{}", i);
            let kv = bucket.get_kv(key.clone());
            println!("{:?}",kv);
        }
    }

    // let mut file = OpenOptions::new()
    //     .read(true)
    //     .write(true)
    //     .create(true)
    //     .open("test.txt").unwrap();
    // file.set_len(20).unwrap();
    // let mut mmap = unsafe { MmapMut::map_mut(&file).expect("failed to map the file") };
    // let len = mmap.len();
    // println!("len is {}",len);
    // let min = std::cmp::min(len, 13);
    // mmap[0..min].copy_from_slice("hello, world!".as_bytes());
    //
    // let mut  buf = [0u8;13];
    // let read = file.read(&mut buf).unwrap();
    // println!("read is {}",read);
}

