use alloc::{
    borrow::ToOwned,
    boxed::Box,
    format,
    string::{String, ToString},
    vec::Vec,
};
use core::cmp::min;

use dbop::{Operate, OperateSet};
use jammdb::{Bucket, Data};
use preprint::pprintln;
use rvfs::{info, warn, StrResult};

use crate::clone_db;

/// bucket: root:key1:key2:key3
pub fn execute_operate(bucket: &str, operate: OperateSet) -> isize {
    info!("execute_operate");
    let db = clone_db();
    let tx = db.tx(true).unwrap();
    let path = bucket.split(":").collect::<Vec<&str>>();
    let mut bucket = tx.get_bucket(path[0]).unwrap();

    for i in 1..path.len() {
        bucket = bucket.get_bucket(path[i]).unwrap();
    }
    execute_operate_real(bucket, Box::new(operate));
    tx.commit().unwrap();
    0
}

fn execute_operate_real(bucket: Bucket, operate: Box<OperateSet>) -> isize {
    for i in operate.operate {
        match i {
            Operate::RenameKey(operate) => {
                let old_key = operate.old_key;
                if bucket.get(old_key.clone()).is_none() {
                    continue;
                }
                let new_key = operate.new_key;
                if old_key == new_key {
                    continue;
                }
                if bucket.get(new_key.clone()).is_some() {
                    continue;
                }
                // we need copy old value to new key
                let value = bucket.get(old_key.clone()).unwrap();
                match value {
                    Data::KeyValue(kv) => {
                        bucket.put(new_key.clone(), kv.value().to_owned()).unwrap();
                    }
                    Data::Bucket(bucket_name) => {
                        let new_bucket = bucket.create_bucket(new_key.clone()).unwrap();
                        let old_bucket = bucket.get_bucket(bucket_name).unwrap();
                        copy_bucket_data_recursive(old_bucket, new_bucket);
                        bucket.delete_bucket(old_key).unwrap();
                    }
                }
            }
            Operate::AddKey(operate) => {
                for (key, value) in operate.map {
                    bucket.put(key, value).unwrap();
                }
            }
            Operate::AddBucket(operate) => {
                let key = operate.key;
                if bucket.get_bucket(key.clone()).is_ok() {
                    continue;
                }
                let new_bucket = bucket.create_bucket(key.clone()).unwrap();
                if let Some(operate) = operate.other {
                    execute_operate_real(new_bucket, operate);
                }
            }
            Operate::DeleteKey(operate) => {
                for key in operate.keys {
                    bucket.delete(key).unwrap();
                }
            }
            Operate::Read(operate) => {
                let buf = unsafe {
                    core::slice::from_raw_parts_mut(operate.buf_addr as *mut u8, operate.buf_size)
                };
                let mut offset = 0;
                for key in operate.keys {
                    let value = bucket.get_kv(key).unwrap();
                    let len = min(buf.len() - offset, value.value().len());
                    buf[offset..offset + len].copy_from_slice(&value.value()[..len]);
                    offset += len;
                }
            }
            Operate::StepInto(operate) => {
                let key = operate.key;
                let new_bucket = bucket.get_bucket(key).unwrap();
                if let Some(operate) = operate.other {
                    execute_operate_real(new_bucket, operate);
                }
            }
        }
    }
    0
}

/// This operation will cause low performance
fn copy_bucket_data_recursive<'a, 'tx>(old_bucket: Bucket<'a, 'tx>, new_bucket: Bucket<'a, 'tx>) {
    old_bucket.cursor().for_each(|data| match data {
        Data::KeyValue(kv) => {
            new_bucket
                .put(kv.key().to_owned(), kv.value().to_owned())
                .unwrap();
        }
        Data::Bucket(bucket_name) => {
            let new_bucket = new_bucket.create_bucket(bucket_name.clone()).unwrap();
            let old_bucket = old_bucket.get_bucket(bucket_name).unwrap();
            copy_bucket_data_recursive(old_bucket, new_bucket);
        }
    })
}

pub fn extend_create_global_bucket(key: &str) -> StrResult<()> {
    let db = clone_db();
    let tx = db.tx(true).unwrap();
    let bucket = tx.create_bucket(key);
    if bucket.is_err() {
        Err("create bucket failed")
    } else {
        tx.commit().unwrap();
        Ok(())
    }
}

pub fn show_dbfs() -> StrResult<()> {
    let db = clone_db();
    let tx = db.tx(true).unwrap();
    tx.buckets().for_each(|(name, x)| {
        let key = name.name();
        let key = String::from_utf8_lossy(key).to_string();
        info!("BUCKET:{}", key);
        pprintln!("BUCKET:{}", key);
        show_bucket(0, x);
    });
    Ok(())
}

fn show_bucket(tab: usize, bucket: Bucket) {
    bucket.cursor().for_each(|x| match x {
        Data::Bucket(x) => {
            let key = x.name().to_owned();
            let value = format!("BUCKET:{:?}", String::from_utf8_lossy(&key).to_string());
            let v = tab * 2 + value.len();
            info!("{value:>w$}", w = v, value = value);
            pprintln!("{value:>w$}", w = v, value = value);
            let bucket = bucket.get_bucket(key).unwrap();
            show_bucket(tab + 1, bucket);
        }
        Data::KeyValue(kv) => {
            let key = kv.key();
            let value = kv.value();
            let key = String::from_utf8_lossy(key).to_string();
            let value = format!("{}:{:?}", key, value);
            let v = tab * 2 + value.len();
            info!("{value:>w$}", w = v, value = value);
            pprintln!("{value:>w$}", w = v, value = value);
        }
    })
}

pub enum Para<'a, 'tx> {
    Data(&'a [u8]),
    Bucket(Bucket<'a, 'tx>),
}

#[repr(C)]
pub struct MyPara<'a, 'tx>(pub Para<'a, 'tx>);

/// root:key:subkey:subkey:subkey
pub fn execute<T, R>(key: &str, func: T, buf: &mut [u8]) -> R
where
    T: FnOnce(&str, MyPara, &mut [u8]) -> R,
{
    let db = clone_db();
    let tx = db.tx(true).unwrap();
    let component = key.split(":").collect::<Vec<&str>>();
    let mut bucket = tx.get_bucket(component[0]).unwrap();
    for i in 1..component.len() - 1 {
        step_into(component[i].to_string(), &mut bucket).unwrap();
    }
    warn!("dbfs try execute user function");
    let res = func(
        component[component.len() - 1],
        MyPara(Para::Bucket(bucket)),
        buf,
    );
    tx.commit().unwrap();
    res
}

fn step_into(key: String, bucket: &mut Bucket) -> StrResult<()> {
    let res = bucket.get_bucket(key);
    if res.is_err() {
        Err("not found")
    } else {
        *bucket = res.unwrap();
        Ok(())
    }
}
