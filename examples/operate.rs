use std::{cmp::min, sync::Arc};

use dbfs2::{
    extend::{execute_operate, extend_create_global_bucket, show_dbfs},
    init_dbfs, SLICE_SIZE,
};
use dbop::{
    add_key, make_operate_set, read_key, AddBucketOperate, AddKeyOperate, DeleteKeyOperate,
    Operate, OperateSet, ReadOperate, RenameKeyOperate, StepIntoOperate,
};
use jammdb::{
    memfile::{FakeMap, FileOpenOptions},
    DB,
};

fn init_db(db: &DB) {
    let tx = db.tx(true).unwrap();
    let bucket = tx.get_or_create_bucket("super_blk").unwrap();
    bucket.put("continue_number", 1usize.to_be_bytes()).unwrap();
    bucket.put("magic", 1111u32.to_be_bytes()).unwrap();
    bucket
        .put("blk_size", (SLICE_SIZE as u32).to_be_bytes())
        .unwrap();
    bucket
        .put("disk_size", (1024 * 1024 * 16u64).to_be_bytes())
        .unwrap(); //16MB
    tx.commit().unwrap()
}

fn main() {
    env_logger::init();
    let db = DB::open::<FileOpenOptions, _>(Arc::new(FakeMap), "my-database.db").unwrap();
    init_db(&db);
    init_dbfs(db);
    extend_create_global_bucket("test").unwrap();

    add_key!(
        addkey_operate,
        ("name", b"hello".to_vec()),
        ("data1", b"world".to_vec()),
        ("data2", b"world".to_vec())
    );
    let buf = [0u8; 20];
    read_key!(read_operate, ["name", "data1", "data2"], buf.as_ptr(), 20);
    let mut add_bucket = AddBucketOperate::new("dir1", None);
    add_key!(
        add_operate,
        ("uid", b"111".to_vec()),
        ("gid", b"222".to_vec()),
        ("mode", b"333".to_vec())
    );
    let add_bucket1 = AddBucketOperate::new("dir2", None);
    make_operate_set!(
        operate_set,
        [
            Operate::AddKey(add_operate),
            Operate::AddBucket(add_bucket1)
        ]
    );
    add_bucket.add_other(Box::new(operate_set));
    make_operate_set!(
        operate_set,
        [
            Operate::AddKey(addkey_operate),
            Operate::AddBucket(add_bucket),
            Operate::Read(read_operate)
        ]
    );

    let str = serde_json::to_string(&operate_set).unwrap();
    println!("{str}");

    execute_operate("test", operate_set);
    println!("buf:{:?}", core::str::from_utf8(&buf).unwrap());
    show_dbfs().unwrap();

    // test step_into rename and delete
    let rename_operate = RenameKeyOperate::new("dir1", "dir2");
    let mut step_into_operate = StepIntoOperate::new("dir2", None);
    let delete_operate = DeleteKeyOperate::new().add_key("mode");

    step_into_operate.add_other(Box::new(
        OperateSet::new().add_operate(Operate::DeleteKey(delete_operate)),
    ));
    let operate_set = OperateSet::new()
        .add_operate(Operate::RenameKey(rename_operate))
        .add_operate(Operate::StepInto(step_into_operate));
    execute_operate("test", operate_set);
    println!("----------------------------");
    show_dbfs().unwrap();
}

#[allow(unused)]
fn equal(buf: &mut [u8]) {
    let db = DB::open::<FileOpenOptions, _>(Arc::new(FakeMap), "my-database.db").unwrap();
    let tx = db.tx(true).unwrap();
    let bucket = tx.get_bucket("test").unwrap();
    bucket.put("name", b"hello".to_vec()).unwrap();
    bucket.put("data1", b"world".to_vec()).unwrap();
    bucket.put("data2", b"world".to_vec()).unwrap();
    let mut start = 0;
    let v1 = bucket.get_kv("name").unwrap();
    let min_ = min(v1.value().len(), buf.len() - start);
    buf[start..start + min_].copy_from_slice(&v1.value()[..min_]);
    start += min_;
    let v2 = bucket.get_kv("data1").unwrap();
    let min_ = min(v2.value().len(), buf.len() - start);
    buf[start..start + min_].copy_from_slice(&v2.value()[..min_]);
    start += min_;
    let v3 = bucket.get_kv("data2").unwrap();
    let min_ = min(v3.value().len(), buf.len() - start);
    buf[start..start + min_].copy_from_slice(&v3.value()[..min_]);

    let dir = bucket.create_bucket("dir1").unwrap();
    dir.put("uid", b"111".to_vec()).unwrap();
    dir.put("gid", b"222".to_vec()).unwrap();
    dir.put("mode", b"333".to_vec()).unwrap();
    dir.create_bucket("dir2").unwrap();
}
