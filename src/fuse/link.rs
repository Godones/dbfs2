use crate::common::{DbfsAttr, DbfsError, DbfsPermission, DbfsResult, DbfsTimeSpec, MAX_PATH_LEN};
use crate::inode::{dbfs_common_create, dbfs_common_link};
use crate::link::{dbfs_common_readlink, dbfs_common_unlink};
use downcast::_std::time::SystemTime;
use fuser::Request;
use log::{error, warn};

pub fn dbfs_fuse_link(
    req: &Request<'_>,
    ino: u64,
    newparent: u64,
    newname: &str,
) -> Result<DbfsAttr, DbfsError> {
    warn!(
        "dbfs_fuse_link(ino:{}, newparent:{}, newname:{:?})",
        ino, newparent, newname
    );
    let time = DbfsTimeSpec::from(SystemTime::now()).into();
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
        "dbfs_fuse_symlink(parent:{}, name:{:?}, link:{:?})",
        parent, name, link
    );
    let time = DbfsTimeSpec::from(SystemTime::now()).into();
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
    )
    .map_err(|_| DbfsError::NotFound)?;
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
    let time = DbfsTimeSpec::from(SystemTime::now()).into();
    let _res = dbfs_common_unlink(req.uid(), req.gid(), parent as usize, name, None, time)?;
    Ok(())
}
