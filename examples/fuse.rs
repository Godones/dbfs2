use clap::Parser;
use dbfs2::fuse::DbfsFuse;
use fuser::MountOption;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Mount point
    #[arg(long)]
    mount_point: String,
    /// Automatically unmount on process exit
    #[arg(long)]
    auto_unmount: bool,
    /// Allow root user to access filesystem
    #[arg(long)]
    allow_other: bool,
    /// Mount FUSE with direct IO
    #[arg(long)]
    direct_io: bool,
    /// Enable setuid support when run as root
    #[arg(long)]
    suid: bool,
    /// Other FUSE options
    #[arg(long)]
    other: Vec<String>,
}

fn main() {
    // 解析命令行参数
    let args = Args::parse();
    let mount_point = args.mount_point;

    // 构建挂载选项
    let mut options = vec![MountOption::FSName("dbfs".to_string())];
    if args.auto_unmount {
        options.push(MountOption::AutoUnmount);
    }
    if args.allow_other {
        options.push(MountOption::AllowOther);
    }
    options.push(MountOption::DefaultPermissions);
    options.push(MountOption::RW);
    options.push(MountOption::Async);

    // 处理自定义选项
    let other = args.other.join(" ").trim().to_string();
    if !other.is_empty() {
        options.push(MountOption::CUSTOM(other));
    }

    // 初始化文件系统
    let dbfs = DbfsFuse::new(args.direct_io, args.suid);

    // 打印挂载选项供调试
    println!("Mount options: {:?}", options);

    // 调用 FUSE 挂载并处理错误
    if let Err(e) = fuser::mount2(dbfs, mount_point, &options) {
        eprintln!("Failed to mount FUSE filesystem: {:?}", e);
        std::process::exit(1);
    }
}
