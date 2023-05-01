#![feature(error_in_core)]
#![cfg_attr(not(test), no_std)]
#![allow(unused)]
extern crate alloc;

mod dir;
mod file;
mod fs_type;
mod inode;

use alloc::sync::Arc;
use core::ops::{Deref, DerefMut};
use jammdb::DB;

use spin::Once;

pub use fs_type::DBFS;
pub mod extend;

#[cfg(feature = "fuse")]
pub mod fuse;

mod attr;
mod common;
mod link;

struct SafeDb(DB);

impl Deref for SafeDb {
    type Target = DB;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for SafeDb {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

unsafe impl Sync for SafeDb {}
unsafe impl Send for SafeDb {}

static DB: Once<Arc<SafeDb>> = Once::new();

/// Initialize the global DBFS database
pub fn init_dbfs(db: DB) {
    DB.call_once(|| Arc::new(SafeDb(db)));
}

fn clone_db() -> Arc<SafeDb> {
    DB.get().unwrap().clone()
}

#[macro_export]
macro_rules! u32 {
    ($x:expr) => {
        u32::from_be_bytes($x.try_into().unwrap())
    };
}

#[macro_export]
macro_rules! u16 {
    ($x:expr) => {
        u16::from_be_bytes($x.try_into().unwrap())
    };
}

#[macro_export]
macro_rules! usize {
    ($x:expr) => {
        usize::from_be_bytes($x.try_into().unwrap())
    };
}
#[macro_export]
macro_rules! u64 {
    ($x:expr) => {
        u64::from_be_bytes($x.try_into().unwrap())
    };
}

#[macro_export]
macro_rules! dbfs_time_spec {
    ($x:expr) => {
        crate::common::DbfsTimeSpec::from($x)
    };
}


#[cfg(feature = "sli512")]
pub const SLICE_SIZE:usize = 512;

#[cfg(feature = "sli1k")]
pub const SLICE_SIZE:usize = 1024;

#[cfg(feature = "sli4k")]
pub const SLICE_SIZE:usize = 4096;

#[cfg(feature = "sli8k")]
pub const SLICE_SIZE:usize = 8192;
