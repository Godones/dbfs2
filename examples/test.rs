use std::fs::{File, OpenOptions};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::thread::{self, JoinHandle, Thread};
use spin::Mutex;



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
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open("test.txt")
        .unwrap();
    file.set_len(1024*1024*128).unwrap();
    let file = Arc::new(Mutex::new(file));
    let fake_file = FakeFile::new(file);
    thread::sleep(std::time::Duration::from_secs(10));
}

