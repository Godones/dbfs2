use downcast::_std::time::SystemTime;
use fuser::Request;
use log::{error, warn};

use crate::{
    common::{DbfsAttr, DbfsError, DbfsPermission, DbfsResult, DbfsTimeSpec, MAX_PATH_LEN},
    inode::{dbfs_common_create, dbfs_common_link},
    link::{dbfs_common_readlink, dbfs_common_unlink},
};

pub fn dbfs_fuse_link(
    req: &Request<'_>,
    ino: u64,
    newparent: u64,
    newname: &str,
) -> DbfsResult<DbfsAttr> {
    warn!(
        "dbfs_fuse_link(ino:{}, newparent:{}, newname:{:?}, newname.len:{})",
        ino,
        newparent,
        newname,
        newname.len()
    );
    // checkout the name length
    if newname.as_bytes().len() > MAX_PATH_LEN {
        return Err(DbfsError::NameTooLong);
    }
    let time = DbfsTimeSpec::from(SystemTime::now());
    dbfs_common_link(
        req.uid(),
        req.gid(),
        ino as usize,
        newparent as usize,
        newname,
        time,
    )
}

pub fn dbfs_fuse_symlink(
    req: &Request<'_>,
    parent: u64,
    name: &str,
    link: &str,
) -> DbfsResult<DbfsAttr> {
    warn!(
        "dbfs_fuse_symlink(parent:{}, name:{:?}, link:{:?}, name.len:{}, link.len:{})",
        parent,
        name,
        link,
        name.len(),
        link.len()
    );
    if name.as_bytes().len() > MAX_PATH_LEN {
        return Err(DbfsError::NameTooLong);
    }
    let time = DbfsTimeSpec::from(SystemTime::now());
    let mut permission = DbfsPermission::from_bits_truncate(0o777);
    permission |= DbfsPermission::S_IFLNK;
    let attr = dbfs_common_create(
        parent as usize,
        name,
        req.uid(),
        req.gid(),
        time,
        permission,
        Some(link),
        None,
    )?;
    Ok(attr)
}

pub fn dbfs_fuse_readlink(ino: u64) -> DbfsResult<[u8; MAX_PATH_LEN]> {
    warn!("dbfs_fuse_readlink(ino:{})", ino);
    let mut buf = [0u8; MAX_PATH_LEN];
    let _link = dbfs_common_readlink(ino as usize, &mut buf)?;
    Ok(buf)
}

pub fn dbfs_fuse_unlink(req: &Request<'_>, parent: u64, name: &str) -> DbfsResult<()> {
    error!("dbfs_fuse_unlink(parent:{}, name:{:?})", parent, name);
    let time = DbfsTimeSpec::from(SystemTime::now());
    dbfs_common_unlink(req.uid(), req.gid(), parent as usize, name, None, time)
}
