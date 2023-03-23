use dbfs2::extend::{execute_operate, extend_create_global_bucket, show_dbfs};
use dbfs2::init_dbfs;
use jammdb::memfile::{FakeMap, FileOpenOptions};
use jammdb::DB;
use std::sync::Arc;
use dbop::{AddBucketOperate, AddKeyOperate, DeleteKeyOperate, Operate, OperateSet, ReadOperate, RenameKeyOperate, StepIntoOperate};

fn init_db(db: &DB) {
    let tx = db.tx(true).unwrap();
    let bucket = tx.get_or_create_bucket("super_blk").unwrap();
    bucket.put("continue_number", 0usize.to_le_bytes()).unwrap();
    bucket.put("magic", 1111u32.to_le_bytes()).unwrap();
    bucket.put("blk_size", 512u32.to_le_bytes()).unwrap();
    tx.commit().unwrap()
}

fn main() {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));
    let db = DB::open::<FileOpenOptions, _>(Arc::new(FakeMap), "my-database.db").unwrap();
    init_db(&db);
    init_dbfs(db);
    extend_create_global_bucket("test").unwrap();

    let addkey_operate = AddKeyOperate::new()
        .add_key("name", b"hello".to_vec())
        .add_key("data1", b"world".to_vec())
        .add_key("data2", b"world".to_vec());


    let buf = [0u8; 20];
    let read_operate = ReadOperate::new()
        .add_key("name")
        .add_key("data1")
        .add_key("data2")
        .set_buf(buf.as_ptr() as usize, 20);

    let mut add_bucket = AddBucketOperate::new("dir1",None);

    let add_operate = AddKeyOperate::new()
        .add_key("uid",b"111".to_vec())
        .add_key("gid",b"222".to_vec())
        .add_key("mode",b"333".to_vec());

    let add_bucket1 = AddBucketOperate::new("dir2",None);
    let operate_set = OperateSet::new()
        .add_operate(Operate::AddKey(add_operate))
        .add_operate(Operate::AddBucket(add_bucket1));
    add_bucket.add_other(Box::new(operate_set));


    let operate_set = OperateSet::new()
        .add_operate(Operate::AddKey(addkey_operate))
        .add_operate(Operate::AddBucket(add_bucket))
        .add_operate(Operate::Read(read_operate));


    let str = serde_json::to_string(&operate_set).unwrap();
    println!("{}",str);

    execute_operate("test", operate_set);
    println!("buf:{:?}", core::str::from_utf8(&buf).unwrap());
    show_dbfs().unwrap();


    // test step_into rename and delete
    let rename_operate = RenameKeyOperate::new("dir1","dir2");
    let mut step_into_operate = StepIntoOperate::new("dir2", None);
    let delete_operate = DeleteKeyOperate::new()
        .add_key("mode");

    step_into_operate.add_other(Box::new(OperateSet::new().add_operate(Operate::DeleteKey(delete_operate))));
    let operate_set = OperateSet::new()
        .add_operate(Operate::RenameKey(rename_operate))
        .add_operate(Operate::StepInto(step_into_operate));
    execute_operate("test", operate_set);
    println!("----------------------------");
    show_dbfs().unwrap();
}
