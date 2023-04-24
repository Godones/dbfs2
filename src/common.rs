#![allow(unused)]
use alloc::string::String;
use bitflags::bitflags;
use onlyerror::Error;

pub const FMODE_EXEC: i32 = 0x20;
pub const MAX_PATH_LEN: usize = 255;


pub const ACCESS_R_OK:u16 = 4;
pub const ACCESS_F_OK:u16 = 0;
pub const ACCESS_W_OK:u16 = 2;
pub const ACCESS_X_OK:u16 = 1;

#[derive(Debug, Default, Clone)]
pub struct DbfsDirEntry {
    pub ino: u64,
    pub offset: u64,
    pub kind: DbfsFileType,
    pub name: String,
}

#[derive(Error, Debug)]
pub enum DbfsError {
    #[error("DbfsError::PermissionDenied")]
    PermissionDenied = 1,
    #[error("DbfsError::NotFound")]
    NotFound = 2,
    #[error("DbfsError::AccessError")]
    AccessError = 13,
    #[error("DbfsError::FileExists")]
    FileExists = 17,
    #[error("DbfsError::InvalidArgument")]
    InvalidArgument = 22,
    #[error("DbfsError::NoSpace")]
    NoSpace = 28,
    #[error("DbfsError::RangeError")]
    RangeError = 34,
    #[error("DbfsError::NotEmpty")]
    NotEmpty = 39,
    #[error("DbfsError::Io")]
    Io = 5,
    #[error("DbfsError::NotSupported")]
    NotSupported = 95,
    #[error("DbfsError::NoData")]
    NoData = 61,
    #[error("DbfsError::Other")]
    Other = 999,
}

pub type DbfsResult<T> = Result<T, DbfsError>;

impl From<jammdb::Error> for DbfsError {
    fn from(value: jammdb::Error) -> Self {
        match value {
            jammdb::Error::BucketExists => DbfsError::FileExists,
            jammdb::Error::BucketMissing => DbfsError::NoData,
            jammdb::Error::KeyValueMissing => DbfsError::NoData,
            jammdb::Error::IncompatibleValue => DbfsError::InvalidArgument,
            jammdb::Error::ReadOnlyTx => DbfsError::AccessError,
            jammdb::Error::Io(_) => DbfsError::Io,
            jammdb::Error::Sync(_) => DbfsError::Io,
            jammdb::Error::InvalidDB(_) => DbfsError::Other,
        }
    }
}

bitflags! {
    pub struct DbfsPermission: u16 {
        const S_IFSOCK = 0o140000;
        const S_IFLNK = 0o120000;
        const S_IFREG = 0o100000;
        const S_IFBLK = 0o060000;
        const S_IFDIR = 0o040000;
        const S_IFCHR = 0o020000;
        const S_IFIFO = 0o010000;
        const S_ISUID = 0o004000;
        const S_ISGID = 0o002000;
        const S_ISVTX = 0o001000;
        const S_IRWXU = 0o700;
        const S_IRUSR = 0o400;
        const S_IWUSR = 0o200;
        const S_IXUSR = 0o100;
        const S_IRWXG = 0o070;
        const S_IRGRP = 0o040;
        const S_IWGRP = 0o020;
        const S_IXGRP = 0o010;
        const S_IRWXO = 0o007;
        const S_IROTH = 0o004;
        const S_IWOTH = 0o002;
        const S_IXOTH = 0o001;
    }
}

#[derive(Debug, Copy, Clone)]
pub enum DbfsFileType {
    NamedPipe,
    /// Character device (S_IFCHR)
    CharDevice,
    /// Block device (S_IFBLK)
    BlockDevice,
    /// Directory (S_IFDIR)
    Directory,
    /// Regular file (S_IFREG)
    RegularFile,
    /// Symbolic link (S_IFLNK)
    Symlink,
    /// Unix domain socket (S_IFSOCK)
    Socket,
}

impl Default for DbfsFileType {
    fn default() -> Self {
        DbfsFileType::RegularFile
    }
}

impl From<DbfsPermission> for DbfsFileType {
    fn from(value: DbfsPermission) -> Self {
        if value.contains(DbfsPermission::S_IFSOCK) {
            DbfsFileType::Socket
        } else if value.contains(DbfsPermission::S_IFLNK) {
            DbfsFileType::Symlink
        } else if value.contains(DbfsPermission::S_IFREG) {
            DbfsFileType::RegularFile
        } else if value.contains(DbfsPermission::S_IFBLK) {
            DbfsFileType::BlockDevice
        } else if value.contains(DbfsPermission::S_IFDIR) {
            DbfsFileType::Directory
        } else if value.contains(DbfsPermission::S_IFCHR) {
            DbfsFileType::CharDevice
        } else if value.contains(DbfsPermission::S_IFIFO) {
            DbfsFileType::NamedPipe
        } else {
            panic!("Invalid file type");
        }
    }
}

impl From<&[u8]> for DbfsFileType {
    fn from(value: &[u8]) -> Self {
        match value {
            b"p" => DbfsFileType::NamedPipe,
            b"c" => DbfsFileType::CharDevice,
            b"b" => DbfsFileType::BlockDevice,
            b"d" => DbfsFileType::Directory,
            b"f" => DbfsFileType::RegularFile,
            b"l" => DbfsFileType::Symlink,
            b"s" => DbfsFileType::Socket,
            _ => panic!("Invalid file type"),
        }
    }
}


#[derive(Debug)]
pub enum XattrNamespace {
    Security,
    System,
    Trusted,
    User,
}

#[derive(Debug, Copy, Clone, Default)]
pub struct DbfsTimeSpec {
    pub sec: u64,
    pub nsec: u32,
}

impl Into<usize> for DbfsTimeSpec {
    fn into(self) -> usize {
        // transform into ms
        (self.sec * 1000 + (self.nsec / 1000) as u64) as usize
    }
}

impl From<usize> for DbfsTimeSpec {
    fn from(value: usize) -> Self {
        let sec = value / 1000;
        let nsec = (value % 1000) * 1000;
        Self {
            sec: sec as u64,
            nsec: nsec as u32,
        }
    }
}
impl DbfsTimeSpec {
    #[allow(unused)]
    pub fn new(sec: u64, nsec: u32) -> Self {
        Self { sec, nsec }
    }
}

#[derive(Debug)]
pub struct DbfsAttr {
    /// Inode number
    pub ino: usize,
    /// Size in bytes
    pub size: usize,
    /// Size in blocks
    pub blocks: usize,
    /// Time of last access
    pub atime: DbfsTimeSpec,
    /// Time of last modification
    pub mtime: DbfsTimeSpec,
    /// Time of last change
    pub ctime: DbfsTimeSpec,
    /// Time of creation (macOS only)
    pub crtime: DbfsTimeSpec,
    /// Kind of file (directory, file, pipe, etc)
    pub kind: DbfsFileType,
    /// Permissions, it does not include the file type
    pub perm: u16,
    /// Number of hard links
    pub nlink: u32,
    /// User id
    pub uid: u32,
    /// Group id
    pub gid: u32,
    /// Rdev
    pub rdev: u32,
    /// Block size
    pub blksize: u32,
    /// Padding
    pub padding: u32,
    /// Flags (macOS only, see chflags(2))
    pub flags: u32,
}

#[derive(Debug)]
#[repr(C)]
pub struct DbfsFsStat {
    pub f_bsize: u64,
    pub f_frsize: u64,
    pub f_blocks: u64,
    pub f_bfree: u64,
    pub f_bavail: u64,
    pub f_files: u64,
    pub f_ffree: u64,
    pub f_favail: u64,
    pub f_fsid: u64,
    pub f_flag: u64,
    pub f_namemax: u64,
    pub name: [u8; 32],
}

#[cfg(feature = "fuse")]
mod impl_fuse {
    extern crate std;
    use super::*;
    use fuser::{FileAttr, FileType};
    use std::time::SystemTime;

    impl From<DbfsFileType> for FileType {
        fn from(value: DbfsFileType) -> Self {
            match value {
                DbfsFileType::NamedPipe => FileType::NamedPipe,
                DbfsFileType::CharDevice => FileType::CharDevice,
                DbfsFileType::BlockDevice => FileType::BlockDevice,
                DbfsFileType::Directory => FileType::Directory,
                DbfsFileType::RegularFile => FileType::RegularFile,
                DbfsFileType::Symlink => FileType::Symlink,
                DbfsFileType::Socket => FileType::Socket,
            }
        }
    }

    impl From<DbfsTimeSpec> for SystemTime {
        fn from(value: DbfsTimeSpec) -> Self {
            SystemTime::UNIX_EPOCH + std::time::Duration::new(value.sec as u64, value.nsec as u32)
        }
    }

    impl From<SystemTime> for DbfsTimeSpec {
        fn from(value: SystemTime) -> Self {
            let duration = value.duration_since(SystemTime::UNIX_EPOCH).unwrap();
            DbfsTimeSpec {
                sec: duration.as_secs(),
                nsec: duration.subsec_nanos(),
            }
        }
    }

    impl From<DbfsAttr> for FileAttr {
        fn from(value: DbfsAttr) -> Self {
            FileAttr {
                ino: value.ino as u64,
                size: value.size as u64,
                blocks: value.blocks as u64,
                atime: SystemTime::from(value.atime),
                mtime: SystemTime::from(value.mtime),
                ctime: SystemTime::from(value.ctime),
                crtime: SystemTime::from(value.crtime),
                kind: value.kind.into(),
                perm: value.perm & 0o777,
                nlink: value.nlink,
                uid: value.uid,
                gid: value.gid,
                rdev: value.rdev,
                blksize: value.blksize,
                padding: 0,
                flags: 0,
            }
        }
    }
}

#[cfg(feature = "rvfs")]
mod impl_rvfs {
    use crate::common::{DbfsFileType, DbfsFsStat};
    use rvfs::inode::InodeMode;
    use rvfs::superblock::StatFs;

    impl From<DbfsFileType> for InodeMode {
        fn from(value: DbfsFileType) -> Self {
            match value {
                DbfsFileType::Directory => InodeMode::S_DIR,
                DbfsFileType::RegularFile => InodeMode::S_FILE,
                DbfsFileType::Symlink => InodeMode::S_SYMLINK,
                _ => panic!("Invalid file type"),
            }
        }
    }
    impl From<DbfsFsStat> for StatFs {
        fn from(value: DbfsFsStat) -> Self {
            StatFs {
                fs_type: value.f_fsid as u32,
                block_size: value.f_bsize,
                total_blocks: value.f_blocks,
                free_blocks: value.f_bfree,
                total_inodes: value.f_files,
                name_len: value.f_namemax as u32,
                name: value.name,
            }
        }
    }
}
