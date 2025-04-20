use alloc::{borrow::ToOwned, format, string::ToString, sync::Arc, vec, vec::Vec};
use core::{cmp::min, sync::atomic::AtomicUsize};

use log::{debug, error};
use rvfs::{
    ddebug,
    dentry::{DirEntry, LookUpData},
    file::{FileMode, FileOps},
    inode::{create_tmp_inode_from_sb_blk, Inode, InodeMode, InodeOps},
    warn, StrResult,
};

use crate::{
    attr::clear_suid_sgid,
    clone_db,
    common::{
        generate_data_key, generate_data_key_with_number, DbfsAttr, DbfsError, DbfsFileType,
        DbfsPermission, DbfsResult, DbfsTimeSpec, ACCESS_W_OK, RENAME_EXCHANGE,
    },
    dbfs_time_spec,
    file::{DBFS_DIR_FILE_OPS, DBFS_FILE_FILE_OPS, DBFS_SYMLINK_FILE_OPS},
    link::{dbfs_common_readlink, dbfs_common_unlink},
    u16, u32, u64, usize, SLICE_SIZE,
};

pub static DBFS_INODE_NUMBER: AtomicUsize = AtomicUsize::new(1);

pub const DBFS_DIR_INODE_OPS: InodeOps = {
    let mut ops = InodeOps::empty();
    ops.create = dbfs_create;
    ops.mkdir = dbfs_mkdir;
    ops.link = dbfs_link;
    ops.unlink = dbfs_unlink;
    ops.symlink = dbfs_symlink;
    ops.lookup = dbfs_lookup;
    ops.rmdir = dbfs_rmdir;
    ops.set_attr = dbfs_setattr;
    ops.get_attr = dbfs_getattr;
    ops.list_attr = dbfs_listattr;
    ops.remove_attr = dbfs_removeattr;
    ops.rename = dbfs_rename;
    ops
};

pub const DBFS_FILE_INODE_OPS: InodeOps = {
    let mut ops = InodeOps::empty();
    ops.set_attr = dbfs_setattr;
    ops.get_attr = dbfs_getattr;
    ops.list_attr = dbfs_listattr;
    ops.truncate = dbfs_truncate;
    ops
};
pub const DBFS_SYMLINK_INODE_OPS: InodeOps = {
    let mut ops = InodeOps::empty();
    ops.set_attr = dbfs_setattr;
    ops.get_attr = dbfs_getattr;
    ops.list_attr = dbfs_listattr;
    ops.readlink = dbfs_readlink;
    ops.follow_link = dbfs_followlink;
    ops
};

fn dbfs_create(dir: Arc<Inode>, dentry: Arc<DirEntry>, mode: FileMode) -> StrResult<()> {
    dbfs_rvfs_create(dir, dentry, mode, InodeMode::S_FILE, None)
}
fn dbfs_mkdir(dir: Arc<Inode>, dentry: Arc<DirEntry>, mode: FileMode) -> StrResult<()> {
    dbfs_rvfs_create(dir, dentry, mode, InodeMode::S_DIR, None)
}

fn dbfs_link(
    old_dentry: Arc<DirEntry>,
    dir: Arc<Inode>,
    new_dentry: Arc<DirEntry>,
) -> StrResult<()> {
    let old_inode = old_dentry.access_inner().d_inode.clone();
    let ino = old_inode.number;
    let name = new_dentry.access_inner().d_name.clone();
    let new_ino = dir.number;

    let _ = dbfs_common_link(0, 0, ino, new_ino, &name, DbfsTimeSpec::default())
        .map_err(|_| "DbfsError::NotFound")?;

    // update old inode data in memory
    // update hard_links
    // set the new dentry's inode to old inode
    old_inode.access_inner().hard_links += 1;
    dir.access_inner().file_size += 1;
    new_dentry.access_inner().d_inode = old_inode;
    Ok(())
}

pub fn dbfs_common_link(
    uid: u32,
    gid: u32,
    ino: usize,
    new_ino: usize,
    name: &str,
    ctime: DbfsTimeSpec,
) -> DbfsResult<DbfsAttr> {
    // checkout permission
    let attr = dbfs_common_attr(new_ino).map_err(|_| DbfsError::NotFound)?;
    if !checkout_access(
        attr.uid,
        attr.gid,
        attr.perm & 0o777,
        uid,
        gid,
        2, //libc::W_OK,
    ) {
        return Err(DbfsError::AccessError);
    }

    let db = clone_db();
    // update new inode data in db
    let tx = db.tx(true)?;
    let bucket = tx.get_bucket(new_ino.to_be_bytes())?;

    let key = generate_data_key(name);
    let value = format!("{}", ino);
    bucket.put(key, value).unwrap();

    let size = bucket.get_kv("size".to_string()).unwrap();
    let size = usize!(size.value());
    bucket.put("size", (size + 1).to_be_bytes())?;

    // update ctime/mtime
    bucket.put("ctime", ctime.to_be_bytes())?;
    bucket.put("mtime", ctime.to_be_bytes())?;

    // update old inode data in memory
    // update hard_links
    // set the new dentry's inode to old inode

    let old_bucket = tx.get_bucket(ino.to_be_bytes())?;
    let hard_links = old_bucket.get_kv("hard_links".to_string()).unwrap();
    let mut hard_links = u32!(hard_links.value());
    hard_links += 1;
    old_bucket.put("hard_links", hard_links.to_be_bytes())?;
    // update ctime: last change time
    old_bucket.put("ctime", ctime.to_be_bytes())?;

    tx.commit()?;
    let dbfs_attr = dbfs_common_attr(ino).map_err(|_| DbfsError::NotFound)?;
    Ok(dbfs_attr)
}

fn dbfs_unlink(dir: Arc<Inode>, dentry: Arc<DirEntry>) -> StrResult<()> {
    let inode = dentry.access_inner().d_inode.clone();
    let number = dir.number;
    let name = &dentry.access_inner().d_name;

    warn!("dbfs_unlink: dir.number={}, name={}", number, name);
    dbfs_common_unlink(
        0,
        0,
        number,
        name,
        Some(inode.number),
        DbfsTimeSpec::default(),
    )
    .map_err(|_| "dbfs_common_unlink failed")?;
    let mut inner = inode.access_inner();
    inner.hard_links -= 1;
    Ok(())
}

fn dbfs_symlink(dir: Arc<Inode>, dentry: Arc<DirEntry>, target: &str) -> StrResult<()> {
    dbfs_rvfs_create(
        dir,
        dentry,
        FileMode::FMODE_READ,
        InodeMode::S_SYMLINK,
        Some(target),
    )
}

fn dbfs_lookup(dir: Arc<Inode>, dentry: Arc<DirEntry>) -> StrResult<()> {
    let number = dir.number;
    let name = dentry.access_inner().d_name.clone();
    let res = dbfs_common_lookup(number, &name).map_err(|_| "dbfs_common_lookup failed")?;
    let inode_mode = InodeMode::from(res.kind);
    // create a inode according to the data in db
    let n_inode = create_tmp_inode_from_sb_blk(
        dir.super_blk.upgrade().unwrap().clone(),
        res.ino,
        inode_mode,
        0,
        inode_ops_from_inode_mode(inode_mode),
        file_ops_from_inode_mode(inode_mode),
        None,
    )?;
    n_inode.access_inner().file_size = res.size;
    n_inode.access_inner().hard_links = res.nlink;
    n_inode.access_inner().uid = res.uid;
    n_inode.access_inner().gid = res.gid;
    dentry.access_inner().d_inode = n_inode;
    Ok(())
}

pub fn dbfs_common_lookup(dir: usize, name: &str) -> DbfsResult<DbfsAttr> {
    let db = clone_db();
    let tx = db.tx(false)?;
    let bucket = tx.get_bucket(dir.to_be_bytes())?;

    let key = generate_data_key(name);
    let value = bucket.get_kv(key);
    if value.is_none() {
        return Err(DbfsError::NotFound);
    }
    let kv = value.unwrap();
    let value = kv.value();
    let str = core::str::from_utf8(value).unwrap();
    let number = str.parse::<usize>().unwrap();

    dbfs_common_attr(number)
}

pub fn dbfs_common_attr(number: usize) -> DbfsResult<DbfsAttr> {
    let db = clone_db();
    let tx = db.tx(false)?;
    let bucket = tx.get_bucket(number.to_be_bytes())?;
    let size = bucket.get_kv("size").unwrap();
    let size = usize!(size.value());

    let mode = bucket.get_kv("mode").unwrap();
    let mode = u16!(mode.value());
    let mode = DbfsPermission::from_bits(mode).unwrap();
    let file_type = DbfsFileType::from(mode);

    let n_links = bucket.get_kv("hard_links").unwrap();
    let n_links = u32!(n_links.value());
    let uid = bucket.get_kv("uid").unwrap();
    let uid = u32!(uid.value());
    let gid = bucket.get_kv("gid").unwrap();
    let gid = u32!(gid.value());

    let blksize = bucket.get_kv("block_size").unwrap();
    let blksize = u32!(blksize.value());
    let blocks = (size + blksize as usize - 1) / blksize as usize;

    let atime = bucket.get_kv("atime").unwrap();
    let atime = dbfs_time_spec!(atime.value());
    let mtime = bucket.get_kv("mtime").unwrap();
    let mtime = dbfs_time_spec!(mtime.value());
    let ctime = bucket.get_kv("ctime").unwrap();
    let ctime = dbfs_time_spec!(ctime.value());

    let rdev = if file_type == DbfsFileType::CharDevice || file_type == DbfsFileType::BlockDevice {
        let dev = bucket.get_kv("dev").unwrap();
        u32!(dev.value())
    } else {
        0
    };

    error!(
        "[[dbfs_common_attr]]: number={}, size={}, mode={:?}, n_links={}, rdev={}",
        number, size, mode, n_links, rdev
    );

    // fill dbfs_attr
    let dbfs_attr = DbfsAttr {
        ino: number,
        size,
        blocks,
        atime: DbfsTimeSpec::from(atime),
        mtime: DbfsTimeSpec::from(mtime),
        ctime: DbfsTimeSpec::from(ctime),
        crtime: DbfsTimeSpec::default(),
        kind: file_type,
        perm: mode.bits(),
        nlink: n_links,
        uid,
        gid,
        rdev,
        blksize,
        padding: 0,
        flags: 0,
    };
    Ok(dbfs_attr)
}

fn dbfs_rmdir(dir: Arc<Inode>, dentry: Arc<DirEntry>) -> StrResult<()> {
    let number = dir.number;
    let name = &dentry.access_inner().d_name;
    dbfs_common_rmdir(0, 0, number, name, DbfsTimeSpec::default()).map_err(|x| {
        warn!("dbfs_common_rmdir failed: {:?}", x);
        "dbfs_common_rmdir failed"
    })?;
    dir.access_inner().file_size -= 1;
    Ok(())
}

/// create a new attribute for a dentry
/// if the key is already exist, it will be overwrite
/// if the key is not exist, it will be created
fn dbfs_setattr(dentry: Arc<DirEntry>, key: &str, val: &[u8]) -> StrResult<()> {
    let db = clone_db();
    let tx = db.tx(true).unwrap();
    let number = dentry.access_inner().d_inode.number;
    let bucket = tx.get_bucket(number.to_be_bytes()).unwrap();
    let key = format!("attr:{}", key);
    bucket.put(key, val).unwrap();
    tx.commit().unwrap();
    Ok(())
}
fn dbfs_removeattr(dentry: Arc<DirEntry>, key: &str) -> StrResult<()> {
    let db = clone_db();
    let tx = db.tx(true).unwrap();
    let number = dentry.access_inner().d_inode.number;
    let bucket = tx.get_bucket(number.to_be_bytes()).unwrap();
    let key = format!("attr:{}", key);
    let res = bucket.delete(key);
    let res = if res.is_err() {
        Err("not found")
    } else {
        Ok(())
    };
    tx.commit().unwrap();
    res
}
fn dbfs_getattr(dentry: Arc<DirEntry>, key: &str, buf: &mut [u8]) -> StrResult<usize> {
    let db = clone_db();
    let tx = db.tx(false).unwrap();
    let number = dentry.access_inner().d_inode.number;
    let bucket = tx.get_bucket(number.to_be_bytes()).unwrap();
    let key = format!("attr:{}", key);
    let value = bucket.get_kv(key);
    let value = if value.is_none() {
        return Err("not found");
    } else {
        value.unwrap()
    };
    let value = value.value();
    let len = min(value.len(), buf.len());
    buf[..len].copy_from_slice(&value[..len]);
    Ok(value.len())
}

fn dbfs_listattr(dentry: Arc<DirEntry>, buf: &mut [u8]) -> StrResult<usize> {
    let db = clone_db();
    let tx = db.tx(false).unwrap();
    let number = dentry.access_inner().d_inode.number;
    let bucket = tx.get_bucket(number.to_be_bytes()).unwrap();
    let mut len = 0;
    let mut total_attr_buf = 0;
    for kv in bucket.kv_pairs() {
        let key = kv.key();
        if key.starts_with("attr:".as_bytes()) {
            let key = key.splitn(2, |c| *c == b':').collect::<Vec<&[u8]>>();
            let key = key[1];
            let key_len = key.len();
            total_attr_buf += key_len + 1;
            if len + key_len >= buf.len() {
                continue;
            }
            buf[len..len + key_len].copy_from_slice(key);
            buf[len + key_len] = 0;
            len += key_len + 1;
        }
    }
    Ok(total_attr_buf)
}

fn dbfs_readlink(dentry: Arc<DirEntry>, buf: &mut [u8]) -> StrResult<usize> {
    let number = dentry.access_inner().d_inode.number;
    dbfs_common_readlink(number, buf).map_err(|_| "not a symlink")
}

fn dbfs_followlink(dentry: Arc<DirEntry>, lookup_data: &mut LookUpData) -> StrResult<()> {
    let db = clone_db();
    let tx = db.tx(false).unwrap();
    let number = dentry.access_inner().d_inode.number;
    let bucket = tx.get_bucket(number.to_be_bytes()).unwrap();
    let value = bucket.get_kv("data").unwrap();
    let value = value.value();
    let str = core::str::from_utf8(value).unwrap();
    lookup_data.symlink_names.push(str.to_string());
    Ok(())
}

fn dbfs_rename(
    old_dir: Arc<Inode>,
    old_dentry: Arc<DirEntry>,
    new_dir: Arc<Inode>,
    new_dentry: Arc<DirEntry>,
) -> StrResult<()> {
    let db = clone_db();
    let tx = db.tx(false).unwrap();
    let old_number = old_dir.number;

    let old_bucket = tx.get_bucket(old_number.to_be_bytes()).unwrap();
    let old_name = old_dentry.access_inner().d_name.clone();

    // let kv = old_bucket.kv_pairs().find(|kv| {
    //     kv.key().starts_with("data".as_bytes()) && kv.value().starts_with(old_name.as_bytes())
    // });

    let key = generate_data_key(&old_name);
    let kv = old_bucket.get_kv(key);

    let new_number = new_dir.number;
    if let Some(kv) = kv {
        let key = kv.key();
        let value = kv.value();
        let str = core::str::from_utf8(value).unwrap();
        let _number = str.parse::<usize>().unwrap();

        let new_name = new_dentry.access_inner().d_name.clone();
        let new_value = format!("{}", new_number);
        let tx = db.tx(true).unwrap();
        let old_bucket = tx.get_bucket(old_number.to_be_bytes()).unwrap();
        if new_number == old_number {
            // in the same bucket
            // update old bucket
            old_bucket.put(key, new_value).unwrap();
        } else {
            // in different bucket
            let new_bucket = tx.get_bucket(new_number.to_be_bytes()).unwrap();
            // update old bucket
            old_bucket.delete(key).unwrap();
            let size = old_bucket.get_kv("size").unwrap();
            let size = usize!(size.value());
            old_bucket.put("size", (size - 1).to_string()).unwrap();

            // update new bucket
            let new_key = generate_data_key(&new_name);
            new_bucket.put(new_key, new_value).unwrap();
            // update size
            let size = new_bucket.get_kv("size").unwrap();
            let size = usize!(size.value());
            new_bucket.put("size", (size + 1).to_string()).unwrap();

            old_dir.access_inner().file_size -= 1;
            new_dir.access_inner().file_size += 1;
        }
        tx.commit().unwrap();
    } else {
        return Err("dbfs_rename: old_dentry not found");
    }
    Ok(())
}

fn dbfs_truncate(inode: Arc<Inode>) -> StrResult<()> {
    let number = inode.number;
    let inode_inner = inode.access_inner();
    let f_size = inode_inner.file_size;
    let _res = dbfs_common_truncate(0, 0, number, DbfsTimeSpec::default(), f_size)
        .map_err(|_| "dbfs_truncate: truncate failed")?;
    Ok(())
}

pub fn permission_from_mode(_mode: FileMode, inode_mode: InodeMode) -> DbfsPermission {
    // we don't use mode now,make all permission to true
    let mut permission = DbfsPermission::from_bits_truncate(0o777);
    match inode_mode {
        InodeMode::S_FILE => permission |= DbfsPermission::S_IFREG,
        InodeMode::S_DIR => permission |= DbfsPermission::S_IFDIR,
        InodeMode::S_SYMLINK => permission |= DbfsPermission::S_IFLNK,
        _ => {}
    }
    permission
}

fn dbfs_rvfs_create(
    dir: Arc<Inode>,
    dentry: Arc<DirEntry>,
    mode: FileMode,
    inode_mode: InodeMode,
    target_path: Option<&str>,
) -> StrResult<()> {
    let dir_number = dir.number;
    let name = dentry.access_inner().d_name.to_owned();
    let permission = permission_from_mode(mode, inode_mode);

    let attr = dbfs_common_create(
        dir_number,
        &name,
        0,
        0,
        DbfsTimeSpec::default(),
        permission,
        target_path,
        None,
    )
    .map_err(|_| "dbfs_rvfs_create: dbfs_common_create failed")?;

    let n_inode = create_tmp_inode_from_sb_blk(
        dir.super_blk.upgrade().unwrap().clone(),
        attr.ino,
        inode_mode,
        0,
        inode_ops_from_inode_mode(inode_mode),
        file_ops_from_inode_mode(inode_mode),
        None,
    )?;

    // update the parent size of the directory
    dir.access_inner().file_size += 1;
    // set dentry with inode
    dentry.access_inner().d_inode = n_inode;
    ddebug!("dbfs_common_create end");
    Ok(())
}

/// checkout the permission
pub fn checkout_access(
    p_uid: u32,
    p_gid: u32,
    mode: u16,
    uid: u32,
    gid: u32,
    access_mask: u16,
) -> bool {
    if access_mask == 0 {
        return true;
    }
    let permission = mode;
    let mut access_mask = access_mask;
    // root is allowed to read & write anything
    if uid == 0 {
        // root only allowed to exec if one of the X bits is set
        access_mask &= 0o1;
        access_mask -= access_mask & (permission >> 6);
        access_mask -= access_mask & (permission >> 3);
        access_mask -= access_mask & permission;
        return access_mask == 0;
    }
    // check user
    if p_uid == uid {
        access_mask -= access_mask & (permission >> 6);
    } else if p_gid == gid {
        access_mask -= access_mask & (permission >> 3);
    } else {
        // check other
        access_mask -= access_mask & permission;
    }

    access_mask == 0
}

fn creation_gid(p_gid: u32, p_mode: DbfsPermission, gid: u32) -> u32 {
    if p_mode.contains(DbfsPermission::S_ISGID) {
        return p_gid;
    }
    gid
}

pub fn dbfs_common_create(
    dir: usize,
    name: &str,
    uid: u32,
    gid: u32,
    c_time: DbfsTimeSpec,
    permission: DbfsPermission,
    target_path: Option<&str>,
    dev: Option<u32>,
) -> DbfsResult<DbfsAttr> {
    ddebug!("dbfs_common_create");
    let new_number = DBFS_INODE_NUMBER.fetch_add(1, core::sync::atomic::Ordering::SeqCst);
    let db = clone_db();
    let tx = db.tx(true)?;

    // find the dir
    let parent = tx.get_bucket(dir.to_be_bytes())?;

    // check the permission
    let p_uid = parent.get_kv("uid").unwrap();
    let p_uid = u32!(p_uid.value());
    let p_gid = parent.get_kv("gid").unwrap();
    let p_gid = u32!(p_gid.value());
    let p_mode = parent.get_kv("mode").unwrap();
    let p_mode = u16!(p_mode.value());
    let bool = checkout_access(p_uid, p_gid, p_mode & 0o777, uid, gid, 0o2);
    if !bool {
        return Err(DbfsError::AccessError);
    }

    let size = parent.get_kv("size").unwrap();
    let size = usize!(size.value());
    // update the size of the dir
    parent.put("size", (size + 1).to_be_bytes()).unwrap();

    let key = generate_data_key(name);
    let value = format!("{}", new_number);
    parent.put(key, value).unwrap(); // add a new entry to the dir

    // update dir ctime/mtime
    parent.put("ctime", c_time.to_be_bytes()).unwrap();
    parent.put("mtime", c_time.to_be_bytes()).unwrap();

    let mut mode = permission;
    if uid != 0 {
        mode -= DbfsPermission::S_ISUID;
        mode -= DbfsPermission::S_ISGID;
    }

    let p_mode = DbfsPermission::from_bits_truncate(p_mode);

    {
        if permission.contains(DbfsPermission::S_IFDIR) {
            // for dir, set the S_ISGID bit if the parent dir has the S_ISGID bit set
            if p_mode.contains(DbfsPermission::S_IFDIR) {
                mode |= DbfsPermission::S_ISGID;
            }
        }
    }

    // set the gid of inode
    let gid = creation_gid(p_gid, permission, gid);

    // create a new inode

    let new_inode = tx.create_bucket(new_number.to_be_bytes())?;

    // set the mode of inode
    new_inode.put("mode", mode.bits().to_be_bytes())?;
    // set the size of inode to 0

    let (hard_link, file_size, dev) = if permission.contains(DbfsPermission::S_IFSOCK)
        || permission.contains(DbfsPermission::S_IFCHR)
        || permission.contains(DbfsPermission::S_IFBLK)
        || permission.contains(DbfsPermission::S_IFIFO)
    {
        (1u32, 0usize, dev)
    } else if permission.contains(DbfsPermission::S_IFDIR) {
        (2, 2, None)
    } else if permission.contains(DbfsPermission::S_IFLNK) {
        assert!(target_path.is_some());
        (1, target_path.as_ref().unwrap().len(), None)
    } else {
        (1, 0, None)
    };
    if permission.contains(DbfsPermission::S_IFDIR) {
        // new_inode.put("next_number", 2u32.to_be_bytes())?;
        let dot_value = format!("{}", new_number);
        new_inode.put(generate_data_key("."), dot_value)?;
        let dotdot_value = format!("{}", dir);
        new_inode.put(generate_data_key(".."), dotdot_value)?;
    }
    new_inode.put("size", file_size.to_be_bytes())?;
    new_inode.put("hard_links", hard_link.to_be_bytes())?;
    new_inode.put("uid", uid.to_be_bytes())?;
    new_inode.put("gid", gid.to_be_bytes())?;
    // set time
    new_inode.put("atime", c_time.to_be_bytes())?;
    new_inode.put("mtime", c_time.to_be_bytes())?;
    new_inode.put("ctime", c_time.to_be_bytes())?;

    new_inode.put("block_size", (SLICE_SIZE as u32).to_be_bytes())?;
    if permission.contains(DbfsPermission::S_IFLNK) {
        new_inode.put("data", target_path.unwrap())?;
    }

    if dev.is_some() {
        new_inode.put("dev", dev.as_ref().unwrap().to_be_bytes())?;
    }

    tx.commit()?;

    warn!(
        "dbfs_common_create: create a new inode {}, hard_links:{}",
        new_number, hard_link
    );

    let dbfs_attr = DbfsAttr {
        ino: new_number,
        size: file_size,
        blocks: 0,
        atime: DbfsTimeSpec::from(c_time),
        mtime: DbfsTimeSpec::from(c_time),
        ctime: DbfsTimeSpec::from(c_time),
        crtime: DbfsTimeSpec::default(),
        kind: DbfsFileType::from(permission),
        perm: mode.bits(),
        nlink: hard_link,
        uid,
        gid,
        rdev: dev.unwrap_or(0),
        blksize: 512,
        padding: 0,
        flags: 0,
    };

    ddebug!("dbfs_common_create end");
    Ok(dbfs_attr)
}

pub fn dbfs_common_access(p_uid: u32, p_gid: u32, ino: usize, mask: i32) -> DbfsResult<bool> {
    let db = clone_db();
    let tx = db.tx(false)?;
    let inode = tx.get_bucket(ino.to_be_bytes())?;
    let mode = inode.get_kv("mode").unwrap();
    let mode = u16!(mode.value());
    let uid = inode.get_kv("uid").unwrap();
    let uid = u32!(uid.value());
    let gid = inode.get_kv("gid").unwrap();
    let gid = u32!(gid.value());
    let res = checkout_access(p_uid, p_gid, mode, uid, gid, mask as u16);
    Ok(res)
}

pub fn dbfs_common_truncate(
    r_uid: u32,
    r_gid: u32,
    ino: usize,
    ctime: DbfsTimeSpec,
    f_size: usize,
) -> DbfsResult<DbfsAttr> {
    warn!("dbfs_truncate: set size to {}", f_size);
    let mut attr = dbfs_common_attr(ino).map_err(|_| DbfsError::NotFound)?;
    // checkout permission
    if !checkout_access(attr.uid, attr.gid, attr.perm, r_uid, r_gid, ACCESS_W_OK) {
        return Err(DbfsError::AccessError);
    }

    let db = clone_db();
    let tx = db.tx(true)?;
    let bucket = tx.get_bucket(ino.to_be_bytes()).unwrap();
    let start = f_size / SLICE_SIZE;
    let offset = f_size % SLICE_SIZE;

    let current_size = attr.size;
    // if current file size < f_size, allocate new blocks
    // if current file size > f_size, free blocks

    let current_block = current_size / SLICE_SIZE;
    if current_block < start {
        // We don't need to allocate new blocks
        // When write or read occurs, it will allocate new blocks or ignore
        // We need set the size of the file
        let sb_blk = tx.get_bucket("super_blk".as_bytes()).unwrap();
        let disk_size = sb_blk.get_kv("disk_size").unwrap();
        let disk_size = u64!(disk_size.value());
        let gap = f_size.saturating_sub(current_size); // newsize - oldsize
        if disk_size < gap as u64 {
            return Err(DbfsError::NoSpace);
        }
        let new_disk_size = disk_size - gap as u64;
        sb_blk.put("disk_size", new_disk_size.to_be_bytes())?;
    } else {
        // we need to free blocks
        for i in start + 1..=current_block {
            let key = generate_data_key_with_number(i as u32);
            if bucket.get_kv(&key).is_some() {
                bucket.delete(&key)?;
            }
        }
        // fill the first data to zero
        let start_key = generate_data_key_with_number(start as u32);
        let value = bucket.get_kv(&start_key);
        if value.is_some() {
            let value = value.unwrap();
            let mut value = value.value().to_vec();
            // set the data in offset to 0
            for i in offset..SLICE_SIZE {
                value[i] = 0;
            }
            bucket.put(start_key, value).unwrap();
        }
        let sb_blk = tx.get_bucket("super_blk".as_bytes()).unwrap();
        let disk_size = sb_blk.get_kv("disk_size").unwrap();
        let disk_size = u64!(disk_size.value());
        let additional_size = (current_block - start) * SLICE_SIZE; // 1 - 0
        let new_disk_size = disk_size + additional_size as u64;
        sb_blk.put("disk_size", new_disk_size.to_be_bytes())?;
    }
    // update inode size
    bucket.put("size", f_size.to_be_bytes())?;
    // update ctime/mtime
    bucket.put("ctime", ctime.to_be_bytes())?;
    bucket.put("mtime", ctime.to_be_bytes())?;
    //Clear SETUID & SETGID on truncate
    let perm = attr.perm;
    let new_perm = clear_suid_sgid(DbfsPermission::from_bits_truncate(perm));
    bucket.put("mode", new_perm.bits().to_be_bytes())?;

    attr.size = f_size;
    attr.ctime = ctime;
    attr.mtime = ctime;
    attr.perm = new_perm.bits();

    tx.commit()?;
    Ok(attr)
}

pub fn dbfs_common_rmdir(
    r_uid: u32,
    r_gid: u32,
    p_ino: usize,
    name: &str,
    c_time: DbfsTimeSpec,
) -> DbfsResult<()> {
    let db = clone_db();
    let tx = db.tx(true)?;
    let p_bucket = tx.get_bucket(p_ino.to_be_bytes())?;

    let key = generate_data_key(name);
    let kv = p_bucket.get_kv(&key);
    let kv = kv.unwrap();
    let v_value = kv.value();
    let str = core::str::from_utf8(v_value).unwrap();
    let number = str.parse::<usize>().unwrap();
    let bucket = tx.get_bucket(number.to_be_bytes()).unwrap();

    // checkout the directory is empty
    let size = bucket.get_kv("size").unwrap();
    let size = usize!(size.value());
    // if size > 2, it means the directory is not empty
    //  Directories always have a self and parent link
    error!("dbfs_rmdir {}: size {}", number, size);
    if size > 2 {
        return Err(DbfsError::NotEmpty);
    }
    let p_uid = p_bucket.get_kv("uid").unwrap();
    let p_uid = u32!(p_uid.value());
    let p_gid = p_bucket.get_kv("gid").unwrap();
    let p_gid = u32!(p_gid.value());
    let p_mode = p_bucket.get_kv("mode").unwrap();
    let p_mode = u16!(p_mode.value());
    let p_size = p_bucket.get_kv("size").unwrap();
    let p_size = usize!(p_size.value());
    if !checkout_access(p_uid, p_gid, p_mode & 0o777, r_uid, r_gid, ACCESS_W_OK) {
        return Err(DbfsError::AccessError);
    }
    // "Sticky bit" handling
    let uid = bucket.get_kv("uid").unwrap();
    let uid = u32!(uid.value());
    let p_perm = DbfsPermission::from_bits_truncate(p_mode);
    if p_perm.contains(DbfsPermission::S_ISVTX) && r_uid != 0 && r_uid != p_uid && r_uid != uid {
        return Err(DbfsError::AccessError);
    }
    // update the parent directory
    p_bucket.put("mtime", c_time.to_be_bytes())?;
    p_bucket.put("ctime", c_time.to_be_bytes())?;
    // delete the directory
    p_bucket.delete(kv.key())?;
    p_bucket.put("size", (p_size - 1).to_be_bytes())?;
    // delete the inode
    tx.delete_bucket(number.to_be_bytes())?;
    error!("======== delete dir {} =========", name);
    tx.commit()?;
    Ok(())
}

pub fn dbfs_common_fallocate(
    r_uid: u32,
    r_gid: u32,
    ino: usize,
    offset: usize,
    size: usize,
    mode: u32,
    ctime: DbfsTimeSpec,
) -> DbfsResult<()> {
    let db = clone_db();
    let tx = db.tx(true)?;
    let bucket = tx.get_bucket(ino.to_be_bytes()).unwrap();

    let uid = bucket.get_kv("uid").unwrap();
    let uid = u32!(uid.value());
    let gid = bucket.get_kv("gid").unwrap();
    let gid = u32!(gid.value());
    let perm = bucket.get_kv("mode").unwrap();
    let perm = u16!(perm.value());
    let i_size = bucket.get_kv("size").unwrap();
    let i_size = usize!(i_size.value());

    // checkout permission
    if !checkout_access(uid, gid, perm, r_uid, r_gid, ACCESS_W_OK) {
        return Err(DbfsError::AccessError);
    }

    let f_size = offset + size;
    let start = f_size / SLICE_SIZE;
    let current_size = i_size;
    let current_block = i_size / SLICE_SIZE;
    if current_block < start {
        // We don't need to allocate new blocks
        // When write or read occurs, it will allocate new blocks or ignore
        // We need set the size of the file
        let sb_blk = tx.get_bucket("super_blk".as_bytes()).unwrap();
        let disk_size = sb_blk.get_kv("disk_size").unwrap();
        let disk_size = u64!(disk_size.value());
        let gap = f_size.saturating_sub(current_size); // newsize - oldsize
        if disk_size < gap as u64 {
            return Err(DbfsError::NoSpace);
        }
        let new_disk_size = disk_size - gap as u64;
        sb_blk.put("disk_size", new_disk_size.to_be_bytes())?;
    } else {
    }
    const FALLOC_FL_KEEP_SIZE: u32 = 0x01;
    if mode & FALLOC_FL_KEEP_SIZE == 0 {
        // update ctime/mtime
        bucket.put("ctime", ctime.to_be_bytes())?;
        bucket.put("mtime", ctime.to_be_bytes())?;
        if f_size > i_size {
            bucket.put("size", f_size.to_be_bytes())?;
        }
    }
    tx.commit()?;
    Ok(())
}

pub fn dbfs_common_rename(
    r_uid: u32,
    r_gid: u32,
    old_dir: usize,
    old_name: &str,
    new_dir: usize,
    new_name: &str,
    flags: u32,
    ctime: DbfsTimeSpec,
) -> DbfsResult<()> {
    let db = clone_db();
    let (old_key, old_number, old_uid, old_gid, old_perm) = {
        let tx = db.tx(false)?;
        let old_dir_bucket = tx.get_bucket(old_dir.to_be_bytes())?;

        let key = generate_data_key(old_name);
        let value = old_dir_bucket.get_kv(&key);

        if value.is_none() {
            return Err(DbfsError::NotFound);
        }

        let old_dir_uid = old_dir_bucket.get_kv("uid").unwrap();
        let old_dir_uid = u32!(old_dir_uid.value());
        let old_dir_gid = old_dir_bucket.get_kv("gid").unwrap();
        let old_dir_gid = u32!(old_dir_gid.value());
        let old_dir_perm = old_dir_bucket.get_kv("mode").unwrap();
        let old_dir_perm = u16!(old_dir_perm.value());

        if !checkout_access(
            old_dir_uid,
            old_dir_gid,
            old_dir_perm & 0o777,
            r_uid,
            r_gid,
            ACCESS_W_OK,
        ) {
            return Err(DbfsError::AccessError);
        }

        let value = value.unwrap();
        let v_value = value.value();
        let str = core::str::from_utf8(v_value).unwrap();
        let number = str.parse::<usize>().unwrap();
        let bucket = tx.get_bucket(number.to_be_bytes()).unwrap();
        let old_uid = bucket.get_kv("uid").unwrap();
        let old_uid = u32!(old_uid.value());

        // "Sticky bit" handling
        let old_dir_perm = DbfsPermission::from_bits_truncate(old_dir_perm);
        if old_dir_perm.contains(DbfsPermission::S_ISVTX)
            && r_uid != 0
            && r_uid != old_dir_uid
            && r_uid != old_uid
        {
            return Err(DbfsError::AccessError);
        }

        let old_gid = bucket.get_kv("gid").unwrap();
        let old_gid = u32!(old_gid.value());
        let old_perm = bucket.get_kv("mode").unwrap();
        let old_perm = u16!(old_perm.value());

        (value.key().to_owned(), number, old_uid, old_gid, old_perm)
    };
    let (new_key, new_number, new_perm, new_size) = {
        let tx = db.tx(false)?;
        let new_dir_bucket = tx.get_bucket(new_dir.to_be_bytes())?;
        let new_dir_uid = new_dir_bucket.get_kv("uid").unwrap();
        let new_dir_uid = u32!(new_dir_uid.value());
        let new_dir_gid = new_dir_bucket.get_kv("gid").unwrap();
        let new_dir_gid = u32!(new_dir_gid.value());
        let new_dir_perm = new_dir_bucket.get_kv("mode").unwrap();
        let new_dir_perm = u16!(new_dir_perm.value());
        if !checkout_access(
            new_dir_uid,
            new_dir_gid,
            new_dir_perm & 0o777,
            r_uid,
            r_gid,
            ACCESS_W_OK,
        ) {
            return Err(DbfsError::AccessError);
        }
        // "Sticky bit" handling in new_parent
        // The new inode may not exist yet, so we have to check the parent

        let new_dir_mode = DbfsPermission::from_bits_truncate(new_dir_perm);

        let key = generate_data_key(new_name);
        let value = new_dir_bucket.get_kv(&key);

        if value.is_some() && new_dir_mode.contains(DbfsPermission::S_ISVTX) {
            let value = value.unwrap();
            let v_value = value.value();
            let str = core::str::from_utf8(v_value).unwrap();
            let number = str.parse::<usize>().unwrap();
            let bucket = tx.get_bucket(number.to_be_bytes()).unwrap();
            let new_uid = bucket.get_kv("uid").unwrap();
            let new_uid = u32!(new_uid.value());
            if r_uid != 0 && r_uid != new_dir_uid && r_uid != new_uid {
                return Err(DbfsError::AccessError);
            }
            let new_perm = bucket.get_kv("mode").unwrap();
            let new_perm = u16!(new_perm.value());
            let new_size = bucket.get_kv("size").unwrap();
            let new_size = usize!(new_size.value());

            (value.key().to_owned(), Some(number), new_perm, new_size)
        } else {
            (vec![], None, 0, 0)
        }
    };

    // Atomic exchange
    if flags & RENAME_EXCHANGE != 0 {
        // we need to check if the new name is already used
        if new_number.is_none() {
            return Err(DbfsError::NotFound);
        }
        let new_number = new_number.unwrap();
        let tx = db.tx(true)?;
        let old_dir_bucket = tx.get_bucket(old_dir.to_be_bytes())?;
        let new_dir_bucket = tx.get_bucket(new_dir.to_be_bytes())?;

        let value = format!("{}", old_number);
        new_dir_bucket.put(new_key, value)?; // new_dir insert old_name and number using new_key

        let value = format!("{}", new_number);
        old_dir_bucket.put(old_key, value)?; // old_dir insert new_name and number using old_key

        // update time
        old_dir_bucket.put("ctime", ctime.to_be_bytes())?;
        old_dir_bucket.put("mtime", ctime.to_be_bytes())?;
        new_dir_bucket.put("ctime", ctime.to_be_bytes())?;
        new_dir_bucket.put("mtime", ctime.to_be_bytes())?;

        let old_bucket = tx.get_bucket(old_number.to_be_bytes())?;
        old_bucket.put("ctime", ctime.to_be_bytes())?;
        let new_bucket = tx.get_bucket(new_number.to_be_bytes())?;
        new_bucket.put("ctime", ctime.to_be_bytes())?;

        // When the old or new name is a dir, we need to update the parent of the children
        // we know that the .. file is the second data

        let old_mode = DbfsPermission::from_bits_truncate(old_perm);
        if old_mode.contains(DbfsPermission::S_IFDIR) {
            let value = format!("{}", new_dir);
            old_bucket.put(generate_data_key("."), value)?;
        }
        let new_mode = DbfsPermission::from_bits_truncate(new_perm);
        if new_mode.contains(DbfsPermission::S_IFDIR) {
            let value = format!("{}", old_dir);
            new_bucket.put(generate_data_key(".."), value)?;
        }

        tx.commit()?;
        return Ok(());
    }

    debug!("We should mv instead of exchange");
    // Only overwrite an existing directory if it's empty
    if new_number.is_some() {
        let perm = DbfsPermission::from_bits_truncate(new_perm);
        if perm.contains(DbfsPermission::S_IFDIR) && new_size > 2 {
            return Err(DbfsError::NotEmpty);
        }
    }

    // Only move an existing directory to a new parent, if we have write access to it,
    // because that will change the ".." link in it
    let old_mode = DbfsPermission::from_bits_truncate(old_perm);
    if old_mode.contains(DbfsPermission::S_IFDIR)
        &&  old_dir != new_dir  // different parent
        &&!checkout_access(
        old_uid,
        old_gid,
        old_perm & 0o777,
        r_uid,
        r_gid,
        ACCESS_W_OK
    ) {
        return Err(DbfsError::AccessError);
    }

    let tx = db.tx(true)?;
    // let new_dir_bucket = tx.get_bucket(new_dir.to_be_bytes())?;

    let old_dir_bucket = tx.get_bucket(old_dir.to_be_bytes())?;
    let new_dir_bucket = tx.get_bucket(new_dir.to_be_bytes())?;

    let old_dir_bucket = &old_dir_bucket;

    let new_dir_bucket = if old_dir == new_dir {
        old_dir_bucket
    } else {
        &new_dir_bucket
    };

    let new_dir_size = new_dir_bucket.get_kv("size").unwrap();
    let mut new_dir_size = usize!(new_dir_size.value());

    // If target already exists decrement its hardlink count
    if new_number.is_some() {
        // debug!("we delete the new_number :{:?}",new_number);
        // 1. delete the new_key
        new_dir_bucket.delete(new_key.clone())?;
        // 2.1 update the size
        // new_dir_bucket.put("size",(new_dir_size - 1).to_be_bytes())?;

        new_dir_size = new_dir_size - 1;

        // 2.2 update the hardlink count
        let new_perm = DbfsPermission::from_bits_truncate(new_perm);
        let new_number = new_number.unwrap();
        if new_perm.contains(DbfsPermission::S_IFDIR) {
            // dir don't have hardlink, so we delete it's bucket of inode
            tx.delete_bucket(new_number.to_be_bytes())?;
        } else {
            // file have hardlink, so we update the hardlink count
            let bucket = tx.get_bucket(new_number.to_be_bytes())?;
            let hardlink = bucket.get_kv("hard_links").unwrap();
            let hardlink = usize!(hardlink.value());
            let hardlink = hardlink - 1;
            if hardlink == 0 {
                tx.delete_bucket(new_number.to_be_bytes())?;
            } else {
                bucket.put("hard_links", hardlink.to_be_bytes())?;
                // update ctime
                bucket.put("ctime", ctime.to_be_bytes())?;
            }
        }
    }
    // debug!("we delete the old_number :{:?}",old_number);
    // 3. delete the old_key

    old_dir_bucket.delete(old_key.as_slice())?;
    // 3.1 update the size

    let old_dir_size = old_dir_bucket.get_kv("size").unwrap();
    let old_dir_size = usize!(old_dir_size.value());

    old_dir_bucket.put("size", (old_dir_size - 1).to_be_bytes())?;

    // debug!("we insert the old_number to new_dir :{:?}",old_number);
    // 4. insert the old_key to new_dir
    let value = format!("{}", old_number);
    if new_number.is_some() {
        new_dir_bucket.put(new_key, value)?;
    } else {
        // let next_number = new_dir_bucket.get_kv("next_number").unwrap();
        // let next_number = u32!(next_number.value());
        let key = generate_data_key(new_name);
        new_dir_bucket.put(key, value)?;
        // new_dir_bucket.put("next_number", (next_number + 1).to_be_bytes())?;
    }

    // 4.1 update the size
    let new_dir_size = if old_dir == new_dir {
        new_dir_size
    } else {
        new_dir_size + 1
    };
    new_dir_bucket.put("size", new_dir_size.to_be_bytes())?;

    // 5.update ctime/mtime for old_dir and new_dir
    old_dir_bucket.put("ctime", ctime.to_be_bytes())?;
    old_dir_bucket.put("mtime", ctime.to_be_bytes())?;
    new_dir_bucket.put("ctime", ctime.to_be_bytes())?;
    new_dir_bucket.put("mtime", ctime.to_be_bytes())?;

    // 6. update ctime for old_bucket
    let old_bucket = tx.get_bucket(old_number.to_be_bytes())?;
    old_bucket.put("ctime", ctime.to_be_bytes())?;

    // 7. update parent of old_bucket
    let old_mode = DbfsPermission::from_bits_truncate(old_perm);
    if old_mode.contains(DbfsPermission::S_IFDIR) {
        let value = format!("{}", new_dir);
        old_bucket.put(generate_data_key(".."), value)?;
    }
    tx.commit()?;
    Ok(())
}

fn inode_ops_from_inode_mode(inode_mode: InodeMode) -> InodeOps {
    match inode_mode {
        InodeMode::S_FILE => DBFS_FILE_INODE_OPS,
        InodeMode::S_DIR => DBFS_DIR_INODE_OPS,
        InodeMode::S_SYMLINK => DBFS_SYMLINK_INODE_OPS,
        _ => InodeOps::empty(),
    }
}

fn file_ops_from_inode_mode(inode_mode: InodeMode) -> FileOps {
    match inode_mode {
        InodeMode::S_FILE => DBFS_FILE_FILE_OPS,
        InodeMode::S_DIR => DBFS_DIR_FILE_OPS,
        InodeMode::S_SYMLINK => DBFS_SYMLINK_FILE_OPS,
        _ => FileOps::empty(),
    }
}
