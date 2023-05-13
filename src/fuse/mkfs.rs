extern crate std;

use alloc::borrow::ToOwned;
use alloc::boxed::Box;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use core::fmt::Display;
use core::sync::atomic::AtomicBool;

use crate::common::DbfsTimeSpec;
use crate::fs_type::dbfs_common_root_inode;
use crate::{init_dbfs, SLICE_SIZE};
use downcast::_std::io::{Read, Seek, Write};
use downcast::_std::println;
use downcast::_std::time::SystemTime;
use jammdb::{
    Bucket, Data, DbFile, File, FileExt, IOResult, IndexByPageID, MemoryMap, MetaData, OpenOption,
    PathLike, DB,
};
use rvfs::warn;
use std::fs::OpenOptions;
use std::path::Path;
use spin::{Mutex, Once, RwLock};
use std::thread::{self, Thread,JoinHandle};

pub struct MyOpenOptions<const S:usize>{
    read: bool,
    write: bool,
    create: bool,
    size: usize,
}
impl <const S:usize> OpenOption for MyOpenOptions<S> {
    fn new() -> Self {
        MyOpenOptions {
            read: false,
            write: false,
            create: false,
            size: S,
        }
    }

    fn read(&mut self, read: bool) -> &mut Self {
        self.read = read;
        self
    }

    fn write(&mut self, write: bool) -> &mut Self {
        self.write = write;
        self
    }

    fn open<T: ToString + PathLike>(&mut self, path: &T) -> IOResult<File> {
        let file = OpenOptions::new()
            .read(self.read)
            .write(self.write)
            .create(self.create)
            .open(path.to_string())
            .unwrap();
        file.set_len(S as u64).unwrap();
        Ok(File::new(Box::new(FakeFile::new(file))))
    }

    fn create(&mut self, create: bool) -> &mut Self {
        self.create = create;
        self
    }
}

pub struct FakeFile {
    file:  std::fs::File,
    size: usize,
}

impl FakeFile {
    pub fn new(file: std::fs::File) -> Self {
        let meta = file.metadata().unwrap();
        let size = meta.len();
        FakeFile {
            file,
            size:size as usize
        }
    }
}


impl core2::io::Seek for FakeFile {
    fn seek(&mut self, pos: core2::io::SeekFrom) -> core2::io::Result<u64> {
        let pos = match pos {
            core2::io::SeekFrom::Start(x) => std::io::SeekFrom::Start(x),
            core2::io::SeekFrom::End(x) => std::io::SeekFrom::End(x),
            core2::io::SeekFrom::Current(x) => std::io::SeekFrom::Current(x),
        };
        self.file
            .seek(pos)
            .map_err(|_x| core2::io::Error::new(core2::io::ErrorKind::Other, "seek error"))
    }
}

impl core2::io::Read for FakeFile {
    fn read(&mut self, buf: &mut [u8]) -> core2::io::Result<usize> {
        self.file
            .read(buf)
            .map_err(|_x| core2::io::Error::new(core2::io::ErrorKind::Other, "read error"))
    }
}
impl core2::io::Write for FakeFile {
    fn write(&mut self, buf: &[u8]) -> core2::io::Result<usize> {
        self.file
            .write(buf)
            .map_err(|_x| core2::io::Error::new(core2::io::ErrorKind::Other, "write error"))
    }

    /// TODO: The first place for update
    fn flush(&mut self) -> core2::io::Result<()> {
        // self.file
        //     .flush()
        //     .map_err(|_x| core2::io::Error::new(core2::io::ErrorKind::Other, "flush error"))
        Ok(())
    }
}

impl FileExt for FakeFile {
    fn lock_exclusive(&self) -> IOResult<()> {
        Ok(())
    }

    /// TODO: The second place for update
    fn allocate(&mut self, new_size: u64) -> IOResult<()> {
        // self.file
        //     .set_len(new_size)
        //     .map_err(|_x| core2::io::Error::new(core2::io::ErrorKind::Other, "allocate error"))
        if self.size > new_size as usize{
            return Ok(())
        }else {
            panic!("Don't need allocate, the new size is {}MB, old size is {}",new_size/1024/1024,self.size/1024/1024);
            let res =
            self.file
                .set_len(new_size)
                .map_err(|_x| core2::io::Error::new(core2::io::ErrorKind::Other, "allocate error"));
            if res.is_ok(){
                self.size = new_size as usize
            }
            res
        }
    }

    fn unlock(&self) -> IOResult<()> {
        Ok(())
    }

    fn metadata(&self) -> IOResult<MetaData> {
        let meta = self
            .file
            .metadata()
            .map(|x| MetaData { len: x.len() })
            .map_err(|_x| core2::io::Error::new(core2::io::ErrorKind::Other, "metadata error"))?;
        Ok(meta)
    }


    /// TODO: The first place for update
    fn sync_all(&self) -> IOResult<()> {
        // self.file
        //     .sync_all()
        //     .map_err(|_x| core2::io::Error::new(core2::io::ErrorKind::Other, "sync_all error"))
        Ok(())
    }

    /// no meaning
    fn size(&self) -> usize {
        self.size
    }

    /// no meaning
    fn addr(&self) -> usize {
        0
    }
}

impl DbFile for FakeFile {}

#[derive(Debug)]
pub struct FakePath {
    path: std::path::PathBuf,
}

impl FakePath {
    pub fn new(path: &str) -> Self {
        FakePath {
            path: std::path::PathBuf::from(path),
        }
    }
}

impl Display for FakePath {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.path.to_str().unwrap())
    }
}

impl PathLike for FakePath {
    fn exists(&self) -> bool {
        self.path.exists()
    }
}

pub struct FakeMMap;


struct IndexByPageIDImpl {
    // map: memmap2::Mmap,
    map:memmap2::Mmap
}

static MMAP:Once<Arc<IndexByPageIDImpl>> = Once::new();

impl MemoryMap for FakeMMap {
    fn do_map(&self, file: &mut File) -> IOResult<Arc<dyn IndexByPageID>> {
        if !MMAP.is_completed(){
            let file = &file.file;
            let fake_file = file.downcast_ref::<FakeFile>().unwrap();
            let res = mmap(&fake_file.file, false);
            if res.is_err() {
                warn!("mmap res: {:?}", res);
                return Err(core2::io::Error::new(
                    core2::io::ErrorKind::Other,
                    "not support",
                ));
            }
            let map = res.unwrap();
            let map = Arc::new(IndexByPageIDImpl{map});
            MMAP.call_once(||map);
        }
        Ok(MMAP.get().unwrap().clone())
    }
}

/// populate
fn mmap(file: &std::fs::File, populate: bool) -> Result<memmap2::Mmap, ()> {
    use memmap2::MmapOptions;
    let mut options = MmapOptions::new();
    if populate {
        options.populate();
    }
    let mmap = unsafe { options.map(file).unwrap() };
    // On Unix we advice the OS that page access will be random.
    mmap.advise(memmap2::Advice::Random).unwrap();
    Ok(mmap)
}

impl IndexByPageID for IndexByPageIDImpl {
    fn index(&self, page_id: u64, page_size: usize) -> IOResult<&[u8]> {
        let start = page_id as usize * page_size;
        let end = start + page_size;
        Ok(&self.map[start..end])
    }

    fn len(&self) -> usize {
        self.map.len()
    }
}


pub fn init_dbfs_fuse<T: AsRef<Path>>(path: T, size: u64) {
    use super::FILE_SIZE;
    let path = path.as_ref().to_str().unwrap();
    let db = DB::open::<MyOpenOptions<FILE_SIZE>, _>(Arc::new(FakeMMap), path).unwrap();
    init_db(&db, size);
    test_dbfs(&db);
    init_dbfs(db);
    let uid = unsafe { libc::getuid() };
    let gid = unsafe { libc::getgid() };
    let time = DbfsTimeSpec::from(SystemTime::now());
    dbfs_common_root_inode(uid, gid, time).unwrap();
}

pub fn init_db(db: &DB, size: u64) {
    let tx = db.tx(true).unwrap();
    let bucket = tx.get_or_create_bucket("super_blk").unwrap();
    bucket.put("continue_number", 0usize.to_be_bytes()).unwrap();
    bucket.put("magic", 1111u32.to_be_bytes()).unwrap();
    bucket.put("blk_size", (SLICE_SIZE as u32).to_be_bytes()).unwrap();
    bucket.put("disk_size", size.to_be_bytes()).unwrap(); //16MB
    tx.commit().unwrap()
}

pub fn test_dbfs(db: &DB) {
    let tx = db.tx(true).unwrap();
    tx.buckets().for_each(|(name, x)| {
        let key = name.name();
        let key = String::from_utf8_lossy(key).to_string();
        println!("BUCKET:{}", key);
        show_bucket(0, x);
    });
}

fn show_bucket(tab: usize, bucket: Bucket) {
    bucket.cursor().for_each(|x| match x {
        Data::Bucket(x) => {
            let key = x.name().to_owned();
            let value = format!("BUCKET:{:?}", String::from_utf8_lossy(&key).to_string());
            let v = tab * 2 + value.len();
            println!("{value:>w$}", w = v, value = value);
            let bucket = bucket.get_bucket(key).unwrap();
            show_bucket(tab + 1, bucket);
        }
        Data::KeyValue(kv) => {
            let key = kv.key();
            let value = kv.value();
            let key = String::from_utf8_lossy(key).to_string();
            let value = format!("{}:{:?}", key, value);
            let v = tab * 2 + value.len();
            println!("{value:>w$}", w = v, value = value);
        }
    })
}
