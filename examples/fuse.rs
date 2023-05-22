use clap::{crate_version, Arg, Command};
use dbfs2::fuse::attr::dbfs_fuse_getattr;
use dbfs2::fuse::{init_dbfs_fuse, DbfsFuse};
use fuser::MountOption;

fn test() {
    env_logger::init();
    init_dbfs_fuse("./test.dbfs", 64 * 1024 * 1024);
    let attr = dbfs_fuse_getattr(0).unwrap();
    println!("attr: {attr:#?}");
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
        .arg(
            Arg::new("default_permissions")
                .long("default_permissions")
                .short('d')
                .default_value("default_permissions")
                .help("Enable permission checking by kernel"),
        )
        .arg(
            Arg::new("direct-io")
                .long("direct-io")
                .requires("MOUNT_POINT")
                .help("Mount FUSE with direct IO"),
        )
        .arg(
            Arg::new("suid")
                .long("suid")
                .help("Enable setuid support when run as root"),
        )
        .get_matches();

    // env_logger::init();
    let mountpoint = matches.value_of("MOUNT_POINT").unwrap();
    let mut options = vec![MountOption::FSName("dbfs".to_string())];
    if matches.contains_id("auto_unmount") {
        options.push(MountOption::AutoUnmount);
    }
    options.push(MountOption::DefaultPermissions);

    let fs = DbfsFuse::new(
        true,
        matches.contains_id("suid"),
    );
    options.push(MountOption::AllowOther);
    options.push(MountOption::RW);
    options.push(MountOption::Async);
    options.push(MountOption::CUSTOM("direct_io readdir_ino".to_string()));

    println!("options: {:?}",options);
    fuser::mount2(fs, mountpoint, &options).unwrap();
}

fn main() {
    // test();
    fuse();
}
