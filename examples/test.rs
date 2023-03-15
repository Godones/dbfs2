use jammdb::memfile::{FakeMap, FileOpenOptions};
use jammdb::DB;
use std::sync::Arc;

fn main() {
    let db = DB::open::<FileOpenOptions, _>(Arc::new(FakeMap), "my-database.db").unwrap();
    let tx = db.tx(true).unwrap();
    let bucket = tx.create_bucket("file").unwrap();
    bucket.put("name", "hello").unwrap();
    bucket.put("data1", "world").unwrap();
    bucket.put("data2", "world").unwrap();
    bucket.put("data3", "world").unwrap();
    bucket.put("data11", "world").unwrap();
    bucket.put("data111", "world").unwrap();
    tx.commit().unwrap();
    let tx = db.tx(true).unwrap();
    let bucket = tx.get_bucket("file").unwrap();
    bucket.kv_pairs().for_each(|x| {
        let key = String::from_utf8_lossy(x.key()).to_string();
        let value = String::from_utf8_lossy(x.value()).to_string();
        println!("{}:{}", key, value);
    });

    println!("{}", format!("data{:04x}", 3));
}
