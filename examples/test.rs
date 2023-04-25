use jammdb::memfile::{FakeMap, FileOpenOptions};
use jammdb::{Data, DB};


use std::ops::Range;
use std::sync::Arc;

fn main() {
    let db = DB::open::<FileOpenOptions, _>(Arc::new(FakeMap), "my-database.db").unwrap();
    let tx = db.tx(true).unwrap();
    let bucket = tx.create_bucket("file").unwrap();
    for i in 0..100{
        let key  = generate_datakey(i);
        let value = format!("value{i}");
        bucket.put(key, value.as_bytes().to_owned()).unwrap();
    }
    let start_key = generate_datakey(10);
    let end_key = generate_datakey(20);
    let rang = Range{
        start: start_key.as_slice(),
        end: end_key.as_slice(),
    };
    bucket.range(rang).for_each(|x|{
        match x {
            Data::Bucket(x) => {
                println!("bucket: {x:?}")
            }
            Data::KeyValue(x) =>{
                println!("keyvalue: {x:?}")
            }
        }
    });
    let d_bucket = tx.create_bucket("dir").unwrap();
    d_bucket.put("dir1","dir1").unwrap();
    d_bucket.put("dir2","dir2").unwrap();
    let t_bucket = tx.get_bucket("dir").unwrap();
    t_bucket.delete("dir1").unwrap();
    tx.commit().unwrap();

    let tx = db.tx(false).unwrap();
    let bucket = tx.get_bucket("dir").unwrap();
    let value = bucket.get("dir1");
    println!("{value:?}");
}


fn generate_datakey(num:u32)->Vec<u8>{
    let mut datakey = b"data".to_vec();
    datakey.extend_from_slice(&num.to_be_bytes());
    datakey
}


