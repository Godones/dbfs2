# DBFS

A file system implemented using a key-value pair database. The filesystem supports linux fuse and is ported to a self-written OS.

## Project Structure

![fsinterface.drawio](assert/fsinterface.drawio.svg)

The diagram above shows the interface design of DBFS. From the bottom up, DBFS is connected by various layers of interfaces and each layer is an independent module that can be reused by other projects. The functions of each layer are described as follows:

1. the bottom layer is the storage medium for final data, in user state, DBFS can store data in a common file, in kernel state, DBFS stores data in a block device like other kernel file systems.
2. The database layer is responsible for organizing the storage of data, managing all the information of the file system, and acting as the engine of the file system implementation.
3. DBFS layer is the file system implementation layer, the file system is built by the functions provided by the database, DBFS provides a common layer of interface that allows DBFS to run in both user state and kernel state by adaptation.
4. The top layer is the final form of DBFS. In user mode and kernel mode, the general interface of DBFS will be adapted to the interface of fuse and vfs framework respectively.



## Usage

DBFS adapts the fuse interface and the [rvfs](https://github.com/Godones/rvfs) framework implemented by the author using rust on the general interface layer.

1. Fuse
- DBFS-Fuse is compatible with libfuse3. Please make sure you have the latest version of FUSE installed.
- Use pkg-config to check if libfuse3 is correctly installed. 
For example:

```bash
pkg-config --modversion fuse3
 ```

Clone the repository locally, then run (please make sure the bench directory exists in advance):
```bash
git clone https://github.com/Godones/dbfs2.git
cargo run --release --example fuse -- --allow-other --auto-unmount --mount-point ./bench/dbfs
```

2. Adapt to `VFS` framework

For the `VFS` framework implemented by the user, DBFS can be introduced as a library. DBFS provides a layer of general interface, the form of which is as follows:

```rust
pub fn dbfs_common_write(number: usize, buf: &[u8], offset: u64) -> DbfsResult<usize> 
pub fn dbfs_common_removexattr(
    r_uid: u32,
    r_gid: u32,
    ino: usize,
    key: &str,
    ctime: DbfsTimeSpec,
) -> DbfsResult<()> 
```

You only need to connect this common interface with the interface of `VFS` implemented by yourself. I implemented a `VFS` framework `rvfs` myself, so if you choose to use `rvfs`, then DBFS is provided out of the box.

Before using DBFS, it is necessary to initialize the global database entity, because the database and DBFS are two modules, so the user can decide how to implement the database interface. At the same time, the user needs to initialize a super block structure in the database so that DBFS can obtain disk metadata normally. An example of using DBFS in the kernel is as follows:

```rust
let db = DB::open::<FileOpenOptions, _>(Arc::new(FakeMap), "my-database.db").unwrap();
init_db(&db);
dbfs2::init_dbfs(db);// init the global db
register_filesystem(DBFS).unwrap();
vfs_mkdir::<FakeFSC>("/db", FileMode::FMODE_WRITE).unwrap();
let _db = do_mount::<FakeFSC>("block", "/db", "dbfs", MountFlags::empty(), None).unwrap();

/// init the dbfs
pub fn init_db(db: &DB, size: u64) {
    let tx = db.tx(true).unwrap();
    let bucket = tx.get_bucket("super_blk");
    let bucket = if bucket.is_ok() {
        return;
    }else {
        tx.create_bucket("super_blk").unwrap()
    };
    bucket.put("continue_number", 1usize.to_be_bytes()).unwrap();
    bucket.put("magic", 1111u32.to_be_bytes()).unwrap();
    bucket.put("blk_size", (SLICE_SIZE as u32).to_be_bytes()).unwrap();
    bucket.put("disk_size", size.to_be_bytes()).unwrap(); //16MB
    tx.commit().unwrap()
}
```

## Test

The fuse implementation of DBFS is fully tested, including correctness and performance tests. The test scripts are located in the `bench` directory.

### Configuration

1. Install the `fuse2fs` tool to ensure that the system supports mounting ext series user mode file systems. See [e2fsprogs](https://github.com/tytso/e2fsprogs/tree/master) for more information.
2. Download the `pjdfstest` test set, which is used to test the POSIX compatibility of the user file system. See [pjdfstest](https://github.com/pjd/pjdfstest) for more information.
3. Download the `mdtest` tool, which is used for performance testing of file system metadata operations. See [mdtest](https://www.gsp.com/cgi-bin/man.cgi?section=1&topic=mdtest) for more information.
4. Download the `fio` tool, which is used for reading and writing performance testing. Check out [fio](https://github.com/axboe/fio) for more information.
5. Download the `filebench` tool, which is used to simulate real application load, see [filebench](https://github.com/filebench/filebench) for more information.
6. Install `python` and possibly other dependencies.



### Run

1. In the DBFS project directory, run the implementation of its fuse

```
make
```

2. Switch to the `bench` directory and create an ext3/ext4 file system image

```
make pre_file
```

3. Mount the ext file system

```4
make ext
```

4. Run the `mdtest` test, the result is in `bench/result/mdtest`

```
make mdtest
```

5. Run the `filebench` test, the results are in `bench/result/filebench`

```
make fbench
```

Since the `filebench` test needs to modify the running directory in the configuration file, before testing, please modify the configuration of the three application loads in the `bench/filebench/` directory, only need to modify the `dir` directory

```
set $dir={your path}/dbfs2/bench/ext3
```

6. Run the `fio` test, the results are located in `bench/result/fiotest`.

```
make fio_sw_1 //seq write 1job
make fio_sw_4 //seq write 4job
make fio_rw_1 //rand write 1job
make fio_rw_4
make_fio_sr_1
make_fio_sr_4
make_fio_rr_1
make_fio_rr_4
```

7. Run `pjdfstest`, this test needs to enter a specific directory, so make sure you are currently in the `dbfs` directory.

```
sudo prove -rv {your}/pjdfstest/tests/
```

If you want to run a single test like `rename`

```
sudo prove -rv {your}/pjdfstest/tests/rename
```

## Feature

- [ ] linux VFS
