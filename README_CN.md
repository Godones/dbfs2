# DBFS

一个使用键值对数据库实现的文件系统。文件系统支持linux的fuse，同时被移植到一个自己编写的OS当中。

## 项目结构

![fsinterface.drawio](assert/fsinterface.drawio.svg)

上图显示了DBFS的接口设计。自下而上，DBFS由各层接口连接起来，且每一层都是一个独立的模块，可以被其他项目所复用。各层的功能描述如下：

1. 最底层是最终数据的存储介质，在用户态，DBFS可以将数据存储在一个普通文件中，在内核态，DBFS与其他内核文件系统一样，将数据存储于块设备中。
2. 数据库层负责组织数据的存储，管理文件系统的所有信息，作为文件系统实现的引擎。
3. DBFS层是文件系统实现层，文件系统的构建依靠数据库提供的功能，DBFS提供了一层通用的接口，使得DBFS可以通过适配从而运行于用户态和内核态。
4. 最上层是DBFS最终的表现形式，如果将DBFS用在用户态，可以通过DBFS的通用接口适配fuse提供的接口，如果将DBFS移植到内核态，那么可以接入内核的VFS接口。



## 使用

DBFS在通用的接口层上适配了fuse接口与笔者使用rust实现的[rvfs](https://github.com/Godones/rvfs)框架。

1. 用户态文件系统
- DBFS-Fuse 兼容 libfuse3，确保支持最新版本的 FUSE。
- 请通过 pkg-config 检查 libfuse3 是否已正确安装，例如：
```
pkg-config --modversion fuse3
```
克隆到本地，然后运行（请提前检查是否有bench目录）：
```
git clone https://github.com/Godones/dbfs2.git
cargo run --release --example fuse -- --allow-other --auto-unmount --mount-point ./bench/dbfs
```

2. 接入`VFS`框架

对于用户自己实现的`VFS`框架，可以将DBFS作为一个库引入，DBFS提供了一层通用的接口，其形式如下：

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

只需要将这个通用接口与其`VFS`的接口对接即可。笔者自己实现了一个`VFS`框架`rvfs`，因此如果你选择使用`rvfs`，那么DBFS是开箱即用的。

DBFS使用前需要对全局的数据库实体初始化，因为数据库与DBFS是两个模块，因此用户可以决定如何实现数据库的接口，同时，用户需要在数据库中初始化一个超级块结构以便DBFS可以正常获得磁盘元数据。一个在内核中使用DBFS的示例如下：

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



## 测试

DBFS的fuse实现进行了完整的测试，包括正确性与性能测试。测试脚本位于`bench`目录下。

### 配置

1. 安装`fuse2fs`工具，确保系统支持挂载ext系列用户态文件系统。查看[e2fsprogs](https://github.com/tytso/e2fsprogs/tree/master)获取更多信息。
2. 下载`pjdfstest`测试集，此测试集用户文件系统POSIX兼容性测试。查看[pjdfstest](https://github.com/pjd/pjdfstest)获取更多信息。
3. 下载`mdtest`工具，此工具用于文件系统元数据操作性能测试。查看[mdtest](https://www.gsp.com/cgi-bin/man.cgi?section=1&topic=mdtest)查看更多信息。
4. 下载`fio`工具，此工具用于读写性能测试。查看[fio](https://github.com/axboe/fio)查看更多信息。
5. 下载`filebench`工具，此工具用于模拟真实的应用负载，查看[filebench](https://github.com/filebench/filebench)查看更多信息。
6. 安装`python`以及其它可能的依赖。

### 运行

1. 在DBFS项目目录下，运行其fuse的实现

```
make
```

2. 切换到`bench`目录下，创建ext3/ext4的文件系统镜像

```
make pre_file
```

3. 挂载ext文件系统

```4
make ext
```

4. 运行`mdtest`测试，结果位于`bench/result/mdtest`中

```
make mdtest
```

5. 运行`filebench`测试，结果位于`bench/result/filebench`中

```
make fbench
```

由于`filebench`测试需要修改配置文件中的运行目录，因此在测试前，请修改`bench/filebench/`目录中三个应用负载的配置，只需要修改`dir`目录即可

```
set $dir={your path}/dbfs2/bench/ext3
```

6. 运行`fio`测试，结果位于`bench/result/fiotest`中。

```
make fio_sw_1  //seq write 1job
make fio_sw_4  //seq write 4job
make fio_rw_1  //rand write 1job
make fio_rw_4
make_fio_sr_1
make_fio_sr_4
make_fio_rr_1
make_fio_rr_4
```

7. 运行`pjdfstest`，此测试需要进入到具体目录中，因此确保当前位于`dbfs`目录中。

```
sudo prove -rv {your}/pjdfstest/tests/
```

如果想要运行单个测试，如`rename`

```
sudo prove -rv {your}/pjdfstest/tests/rename
```



## Feature

- [ ] linux VFS
