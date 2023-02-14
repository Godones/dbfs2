#![no_std]
extern crate alloc;
mod fs_type;

use alloc::sync::Arc;
pub use jammdb;
use jammdb::DB;
use rvfs::StrResult;

pub struct SafeDb<M>(DB<M>);

unsafe impl <M> Sync for SafeDb<M> {}
unsafe impl <M> Send for SafeDb<M> {}

pub struct DbFileSystem<M>{
    db:Arc<SafeDb<M>>
}
