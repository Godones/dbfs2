use jammdb::memfile::{FakeMap, FileOpenOptions};
use jammdb::{Bucket, DB};
use std::cmp::min;
use std::fmt::{Debug, Display};
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

    let n_bucket = bucket.create_bucket("dir1").unwrap();
    solve(n_bucket);
    tx.commit().unwrap();

    println!("--------");
    let tx = db.tx(true).unwrap();
    let bucket = tx.get_bucket("file").unwrap();
    let n_bucket = bucket.get_bucket("dir1").unwrap();
    let value = n_bucket.get_kv("name").unwrap();
    println!(
        "{}:{}",
        core::str::from_utf8(value.key()).unwrap(),
        core::str::from_utf8(value.value()).unwrap()
    );

    test("name".to_string(), n_bucket);
    println!("{}", format!("data{:04x}", 3));
    let mut buf = [0u8; 10];
    let x = execute(example_fun, &mut buf);
    println!("{:?}", x);
    println!("{:?}", core::str::from_utf8(&buf[0..x]));
    tx.commit().unwrap();

    for i in 1..1 {
        println!("{}", format!("data{:04x}", i));
    }
}

fn test(key: String, bucket: Bucket) {
    bucket.put(key, "hello").unwrap();
}

fn solve(bucket: Bucket) {
    bucket.put("name", "hello").unwrap();
}

pub enum Para<'a, 'tx> {
    Data(&'a [u8]),
    Bucket(Bucket<'a, 'tx>),
}

#[repr(C)]
pub struct MyPara<'a, 'tx>(Para<'a, 'tx>);

fn execute<T, R: Display + Debug>(func: T, buf: &mut [u8]) -> R
where
    T: FnOnce(MyPara, &mut [u8]) -> R,
{
    let db = DB::open::<FileOpenOptions, _>(Arc::new(FakeMap), "my-database1.db").unwrap();
    let tx = db.tx(true).unwrap();
    let bucket = tx.create_bucket("file").unwrap();
    bucket.put("name", "hello").unwrap();
    let para = MyPara(Para::Bucket(bucket));
    let r = func(para, buf);
    tx.commit().unwrap();
    r
}

fn example_fun(para: MyPara, buf: &mut [u8]) -> usize {
    match para.0 {
        Para::Data(data) => {
            let len = min(buf.len(), data.len());
            buf[..len].copy_from_slice(&data[..len]);
            len
        }
        Para::Bucket(bucket) => {
            let value = bucket.get_kv("name").unwrap();
            let len = min(buf.len(), value.value().len());
            buf[..len].copy_from_slice(&value.value()[..len]);
            len
        }
    }
}
