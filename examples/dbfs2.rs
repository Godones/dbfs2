use jammdb::memfile::{FakeMap, FileOpenOptions};
use jammdb::DB;
use std::fs::{File, OpenOptions};
use std::os::unix::fs::FileExt;
use std::sync::Arc;

fn main() {
    let db = DB::open::<FileOpenOptions, _>(Arc::new(FakeMap), "my-database.db").unwrap();
    dbfs2::init_dbfs(db);
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open("./f1")
        .unwrap();
    let meta = file.metadata().unwrap().len();
    println!("meta: {}", meta);
    file.write_all_at(b"hello", 10).unwrap();
    let meta = file.metadata().unwrap().len();
    println!("meta: {}", meta);
}
