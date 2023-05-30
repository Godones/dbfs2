use clap::{Parser};

use dbfs2::fuse::{DbfsFuse};
use fuser::MountOption;

#[derive(Parser,Debug)]
#[command(author, version, about, long_about = None)]
struct Args{
    /// Mount point
    #[arg(long)]
    mount_point:String,
    /// Automatically unmount on process exit
    #[arg(long)]
    auto_unmount:bool,
    /// Allow root user to access filesystem
    #[arg(long)]
    allow_other:bool,
    /// Mount FUSE with direct IO
    #[arg(long)]
    direct_io:bool,
    /// Enable setuid support when run as root
    #[arg(long)]
    suid:bool,
    /// Other fuse options
    #[arg(long)]
    other:Vec<String>,
}

fn main() {
    let args = Args::parse();
    let mount_point = args.mount_point;
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
    let other = args.other.join(" ");
    options.push(MountOption::CUSTOM(other));
    let dbfs = DbfsFuse::new(args.direct_io,args.suid);
    println!("options: {:?}",options);
    fuser::mount2(dbfs, mount_point, &options).unwrap();
}
