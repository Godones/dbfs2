#![allow(unused)]
use alloc::{collections::BTreeMap, format, string::String, vec, vec::Vec};
use core::{
    fmt::{Debug, Display, Formatter},
    ops::Deref,
};

use bitflags::bitflags;
use onlyerror::Error;
use rvfs::dentry::DirentType;
use spin::{Once, RwLock};

use crate::{u32, u64};

pub const FMODE_EXEC: i32 = 0x20;
pub const MAX_PATH_LEN: usize = 255;

pub const ACCESS_R_OK: u16 = 4;
pub const ACCESS_F_OK: u16 = 0;
pub const ACCESS_W_OK: u16 = 2;
pub const ACCESS_X_OK: u16 = 1;

pub const RENAME_EXCHANGE: u32 = 0x2;

#[derive(Default, Clone)]
pub struct DbfsDirEntry {
    pub ino: u64,
    pub offset: u64,
    pub kind: DbfsFileType,
    pub name: String,
    /// for readdir plus
    pub attr: Option<DbfsAttr>,
}

impl Debug for DbfsDirEntry {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("DbfsDirEntry")
            .field("ino", &self.ino)
            .field("offset", &self.offset)
            .field("kind", &self.kind)
            .field("name", &self.name)
            .field("attr", &self.attr.is_some())
            .finish()
    }
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
    #[error("DbfsError::NameTooLong")]
    NameTooLong = 36,
    #[error("DbfsError::NoSys")]
    NoSys = 38,
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
        const S_IFMT = 0o17_0000;
        const S_IFSOCK = 0o14_0000;
        const S_IFLNK = 0o12_0000;
        const S_IFREG = 0o10_0000;
        const S_IFBLK = 0o06_0000;
        const S_IFDIR = 0o04_0000;
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

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
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

impl Into<DirentType> for DbfsFileType {
    fn into(self) -> DirentType {
        match self {
            DbfsFileType::NamedPipe => DirentType::DT_UNKNOWN,
            DbfsFileType::CharDevice => DirentType::DT_UNKNOWN,
            DbfsFileType::BlockDevice => DirentType::DT_UNKNOWN,
            DbfsFileType::Directory => DirentType::DT_DIR,
            DbfsFileType::RegularFile => DirentType::DT_REG,
            DbfsFileType::Symlink => DirentType::DT_LNK,
            DbfsFileType::Socket => DirentType::DT_UNKNOWN,
        }
    }
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

impl Display for DbfsTimeSpec {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}.{}", self.sec, self.nsec)
    }
}

impl DbfsTimeSpec {
    pub fn to_be_bytes(&self) -> Vec<u8> {
        let mut buf = vec![];
        buf.extend_from_slice(self.sec.to_be_bytes().as_ref());
        buf.extend_from_slice(self.nsec.to_be_bytes().as_ref());
        buf
    }
}

impl Into<Vec<u8>> for DbfsTimeSpec {
    fn into(self) -> Vec<u8> {
        let mut buf = vec![];
        buf.extend_from_slice(self.sec.to_be_bytes().as_ref());
        buf.extend_from_slice(self.nsec.to_be_bytes().as_ref());
        buf
    }
}

impl From<Vec<u8>> for DbfsTimeSpec {
    fn from(value: Vec<u8>) -> Self {
        let sec = u64!(value[0..8]);
        let nsec = u32!(value[8..12]);
        Self { sec, nsec }
    }
}

impl From<&[u8]> for DbfsTimeSpec {
    fn from(value: &[u8]) -> Self {
        let sec = u64!(value[0..8]);
        let nsec = u32!(value[8..12]);
        Self { sec, nsec }
    }
}

impl DbfsTimeSpec {
    #[allow(unused)]
    pub fn new(sec: u64, nsec: u32) -> Self {
        Self { sec, nsec }
    }
}

#[derive(Debug, Default, Clone)]
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

pub fn generate_data_key_with_number(num: u32) -> Vec<u8> {
    let mut datakey = b"zdata:".to_vec();
    datakey.extend_from_slice(&num.to_be_bytes());
    datakey
}

pub fn generate_data_key(value: &str) -> String {
    format!("data:{}", value)
}

#[derive(Debug, Clone)]
pub struct ReadDirInfo {
    pub offset: usize,
    pub key: String,
}

impl ReadDirInfo {
    pub fn new(offset: usize, key: String) -> Self {
        Self { offset, key }
    }
}

/// When readdir firstly, we need to store the offset and key for the ino so that
/// we can continue to read the directory when the fuse call readdir again.
pub static GLOBAL_READDIR_TABLE: RwLock<BTreeMap<usize, ReadDirInfo>> =
    RwLock::new(BTreeMap::new());

pub fn push_readdir_table(ino: usize, info: ReadDirInfo) {
    let mut table = GLOBAL_READDIR_TABLE.write();
    table.insert(ino, info);
}

pub fn pop_readdir_table(ino: usize) -> Option<ReadDirInfo> {
    let mut table = GLOBAL_READDIR_TABLE.write();
    table.remove(&ino)
}

/// This function will be called when the fuse call readdir.
pub fn get_readdir_table(ino: usize) -> Option<ReadDirInfo> {
    let table = GLOBAL_READDIR_TABLE.read();
    table.get(&ino).cloned()
}

#[cfg(feature = "fuse")]
mod impl_fuse {
    extern crate std;
    use std::time::SystemTime;

    use fuser::{FileAttr, FileType};

    use super::*;

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
                flags: 0,
            }
        }
    }

    impl From<&DbfsAttr> for FileAttr {
        fn from(value: &DbfsAttr) -> Self {
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
                flags: 0,
            }
        }
    }
}

#[cfg(feature = "rvfs")]
mod impl_rvfs {
    use rvfs::{inode::InodeMode, superblock::StatFs};

    use crate::common::{DbfsFileType, DbfsFsStat};

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
