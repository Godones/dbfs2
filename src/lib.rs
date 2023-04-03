#![no_std]
#![allow(unused)]
extern crate alloc;

mod dir;
mod file;
mod fs_type;
mod inode;

use alloc::sync::Arc;
use core::ops::{Deref, DerefMut};
use jammdb::DB;
use rvfs::StrResult;
use spin::Once;

pub use fs_type::DBFS;
pub mod extend;

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
macro_rules! usize {
    ($x:expr) => {
        usize::from_be_bytes($x.try_into().unwrap())
    };
}
