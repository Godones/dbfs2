# dbfs
A filesystem based on a key-value database.



## Interface implementation
```rust
pub const DBFS_DIR_INODE_OPS: InodeOps = {
    let mut ops = InodeOps::empty();
    ops.create = dbfs_create;
    ops.mkdir = dbfs_mkdir;
    ops.link = dbfs_link;
    ops.unlink = dbfs_unlink;
    ops.symlink = dbfs_symlink;
    ops.lookup = dbfs_lookup;
    ops.rmdir = dbfs_rmdir;
    ops.set_attr = dbfs_setattr;
    ops.get_attr = dbfs_getattr;
    ops.list_attr = dbfs_listattr;
    ops.remove_attr = dbfs_removeattr;
    ops.rename = dbfs_rename;
    ops
};

pub const DBFS_FILE_INODE_OPS: InodeOps = {
    let mut ops = InodeOps::empty();
    ops.set_attr = dbfs_setattr;
    ops.get_attr = dbfs_getattr;
    ops.list_attr = dbfs_listattr;
    ops
};
pub const DBFS_SYMLINK_INODE_OPS: InodeOps = {
    let mut ops = InodeOps::empty();
    ops.set_attr = dbfs_setattr;
    ops.get_attr = dbfs_getattr;
    ops.list_attr = dbfs_listattr;
    ops.readlink = dbfs_readlink;
    ops.follow_link = dbfs_followlink;
    ops
};
```


```rust
pub const DBFS_DIR_FILE_OPS: FileOps = {
    let mut ops = FileOps::empty();
    ops.readdir = dbfs_readdir;
    ops.open = |_| Ok(());
    ops
};
pub const DBFS_FILE_FILE_OPS: FileOps = {
    let mut ops = FileOps::empty();
    ops.write = dbfs_file_write;
    ops.read = dbfs_file_read;
    ops.open = |_| Ok(());
    ops
};
pub const DBFS_SYMLINK_FILE_OPS: FileOps = {
    let mut ops = FileOps::empty();
    ops.open = |_| Ok(());
    ops
};
```

## Usage
The dbfs need a key-value database to save the file. We chose the jammdb as the database, so we need to initialize the jammdb first. 
Before we begin to use the dbfs to manage the file, we should create a bucket named `super_blk` to save some meta information.The database can
be used in ram or in disk, it depends on the user.

```rust
let db = DB::open::<FileOpenOptions, _>(Arc::new(FakeMap), "my-database.db").unwrap();
init_db(&db);
dbfs2::init_dbfs(db);
let mnt = rvfs::mount_rootfs();
init_process_info(mnt);
register_filesystem(DBFS).unwrap();
vfs_mkdir::<FakeFSC>("/db", FileMode::FMODE_WRITE).unwrap();
let _db = do_mount::<FakeFSC>("block", "/db", "dbfs", MountFlags::empty(), None).unwrap();

/// init the dbfs
fn init_db(db: &DB) {
    let tx = db.tx(true).unwrap();
    let bucket = tx.get_or_create_bucket("super_blk").unwrap();
    bucket.put("continue_number", 0usize.to_be_bytes()).unwrap();
    bucket.put("magic", 1111u32.to_be_bytes()).unwrap();
    bucket.put("blk_size", 512u32.to_be_bytes()).unwrap();
    tx.commit().unwrap()
}

```