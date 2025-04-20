use core::cmp::min;

use log::{error, warn};

use crate::{
    clone_db,
    common::{generate_data_key, DbfsError, DbfsPermission, DbfsResult, DbfsTimeSpec, ACCESS_W_OK},
    inode::checkout_access,
    u16, u32, usize,
};

pub fn dbfs_common_readlink(ino: usize, buf: &mut [u8]) -> DbfsResult<usize> {
    let db = clone_db();
    let tx = db.tx(false)?;
    let bucket = tx.get_bucket(ino.to_be_bytes())?;
    let value = bucket.get_kv("data").unwrap();
    let value = value.value();
    let len = min(value.len(), buf.len());
    buf[..len].copy_from_slice(&value[..len]);
    Ok(len)
}

pub fn dbfs_common_unlink(
    uid: u32,
    gid: u32,
    dir: usize,
    name: &str,
    ino: Option<usize>,
    c_time: DbfsTimeSpec,
) -> DbfsResult<()> {
    let db = clone_db();
    let tx = db.tx(true)?;
    // find the parent dir
    let p_bucket = tx.get_bucket(dir.to_be_bytes())?;
    // check if the name exists
    // let value = p_bucket
    //     .kv_pairs()
    //     .find(|kv| kv.key().starts_with(b"data") && kv.value().starts_with(name.as_bytes()));
    // if value.is_none() {
    //     return Err(DbfsError::NotFound);
    // }
    let key = generate_data_key(name);
    let kv = p_bucket.get_kv(key);
    let kv = kv.unwrap();

    warn!(
        "dbfs_common_unlink(uid:{}, gid:{}, dir:{}, name:{:?}, ino:{:?}, c_time:{})",
        uid, gid, dir, name, ino, c_time
    );
    // get the uid/gid/perm of the parent dir
    let p_uid = p_bucket.get_kv("uid").unwrap();
    let p_uid = u32!(p_uid.value());
    let p_gid = p_bucket.get_kv("gid").unwrap();
    let p_gid = u32!(p_gid.value());
    let p_perm = p_bucket.get_kv("mode").unwrap();
    let p_perm = u16!(p_perm.value());

    // checkout permission
    if !checkout_access(p_uid, p_gid, p_perm & 0o777, uid, gid, ACCESS_W_OK) {
        return Err(DbfsError::AccessError);
    }

    // find the inode with the name
    let (bucket, ino) = if ino.is_some() {
        let ino = ino.unwrap();
        let bucket = tx.get_bucket(ino.to_be_bytes())?;
        (bucket, ino)
    } else {
        let value = kv.value(); // ino
        let ino = core::str::from_utf8(value).unwrap();
        let ino = ino.parse::<usize>().unwrap();
        let bucket = tx
            .get_bucket(ino.to_be_bytes())
            .map_err(|_| DbfsError::NotFound)?;
        (bucket, ino)
    };

    let ino_uid = bucket.get_kv("uid").unwrap();
    let ino_uid = u32!(ino_uid.value());

    // "Sticky bit" handling
    let p_perm = DbfsPermission::from_bits_truncate(p_perm);
    if p_perm.contains(DbfsPermission::S_ISVTX) && uid != 0 && uid != p_uid && uid != ino_uid {
        return Err(DbfsError::AccessError);
    }

    // delete the kv pair
    p_bucket.delete(kv.key())?;
    // update size
    let size = p_bucket.get_kv("size").unwrap();
    let size = usize!(size.value());
    p_bucket.put("size", (size - 1).to_be_bytes())?;
    // update ctime/mtime
    p_bucket.put("ctime", c_time.to_be_bytes())?;
    p_bucket.put("mtime", c_time.to_be_bytes())?;

    // update the link count
    let h_link = bucket.get_kv("hard_links").unwrap();
    let h_link = u32!(h_link.value());
    error!("---------- hard_links: {}", h_link);
    if h_link == 1 {
        // delete the bucket
        tx.delete_bucket(ino.to_be_bytes())?;
    } else {
        bucket.put("hard_links", (h_link - 1).to_be_bytes())?;
        // update ctime
        bucket.put("ctime", c_time.to_be_bytes())?;
    }
    error!("dir {} size now is {}, ino is {}", dir, size - 1, ino);
    tx.commit()?;
    Ok(())
}
