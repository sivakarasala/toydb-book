/// Storage Engine Layer (Ch1-3)
///
/// Ch1: Key-value fundamentals with HashMap
/// Ch2: Storage trait + generic Database
/// Ch3: Persistent BitCask engine

mod memory;

pub use memory::MemoryStorage;

use crate::error::Result;

/// The core storage trait — all engines implement this (Ch2).
pub trait Storage: Send {
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>>;
    fn set(&mut self, key: &str, value: Vec<u8>) -> Result<()>;
    fn delete(&mut self, key: &str) -> Result<bool>;
    fn scan_prefix(&self, prefix: &str) -> Result<Vec<(String, Vec<u8>)>>;
}
