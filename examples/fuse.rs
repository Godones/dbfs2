use clap::{Arg, Command, crate_version};
use fuser::MountOption;
use dbfs2::fuse::inode::dbfs_fuse_getattr;
use dbfs2::fuse::{DbfsFuse, init_dbfs_fuse};

fn test(){
    env_logger::init();
    init_dbfs_fuse("./test.dbfs",64*1024*1024);
    let attr = dbfs_fuse_getattr(0).unwrap();
    println!("attr: {:#?}", attr);
}

fn fuse() {
    let matches = Command::new("dbfs")
        .version(crate_version!())
        .author("Christopher Berner")
        .arg(
            Arg::new("MOUNT_POINT")
                .required(true)
                .index(1)
                .help("Act as a client, and mount FUSE at given path"),
        )
        .arg(
            Arg::new("auto_unmount")
                .long("auto_unmount")
                .help("Automatically unmount on process exit"),
        )
        .arg(
            Arg::new("allow-root")
                .long("allow-root")
                .help("Allow root user to access filesystem"),
        )
        .get_matches();

    env_logger::init();
    init_dbfs_fuse("./test.dbfs",64*1024*1024);


    let mountpoint = matches.value_of("MOUNT_POINT").unwrap();
    let mut options = vec![MountOption::FSName("dbfs".to_string())];
    if matches.is_present("auto_unmount") {
        options.push(MountOption::AutoUnmount);
    }
    // if matches.is_present("allow-root") {
    //     options.push(MountOption::AllowRoot);
    // }
    // options.push(MountOption::AllowRoot);

    let fs = DbfsFuse;

    options.push(MountOption::AllowOther);
    options.push(MountOption::RW);
    fuser::mount2(fs, mountpoint, &options).unwrap();
}

fn main(){
    // test();
    fuse();
}