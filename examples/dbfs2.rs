use jammdb::memfile::{FakeMap, FileOpenOptions};
use jammdb::DB;
use std::sync::Arc;

fn main() {
    let db = DB::open::<FileOpenOptions, _>(Arc::new(FakeMap), "my-database.db").unwrap();
    dbfs2::init_dbfs(db).unwrap();
}
