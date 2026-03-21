/// Chapter 3: Persistent Storage — BitCask
/// Exercise: Build a log-structured storage engine with crash recovery.
///
/// Run tests: cargo test --bin exercise
/// Run:       cargo run --bin exercise

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum BitCaskError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("Corrupt entry: CRC mismatch")]
    CorruptEntry,
    #[error("Key not found: {0}")]
    KeyNotFound(String),
}

type Result<T> = std::result::Result<T, BitCaskError>;

/// An entry in the log file.
#[derive(Debug)]
pub struct LogEntry {
    pub key: String,
    pub value: Option<Vec<u8>>, // None = tombstone (deleted)
}

/// The BitCask storage engine.
pub struct BitCask {
    path: PathBuf,
    writer: BufWriter<File>,
    keydir: HashMap<String, u64>, // key → file offset
}

impl BitCask {
    /// Open or create a BitCask store at the given path.
    pub fn open(path: &Path) -> Result<Self> {
        // TODO:
        // 1. Create directory if it doesn't exist (fs::create_dir_all)
        // 2. Open the data file (append mode) at path/data.log
        // 3. Build keydir by scanning existing entries (call rebuild_keydir)
        // 4. Return BitCask { path, writer, keydir }
        todo!("Implement open")
    }

    /// Write a key-value pair to the log.
    pub fn set(&mut self, key: &str, value: &[u8]) -> Result<()> {
        // TODO:
        // 1. Get current file position (this is the offset for keydir)
        // 2. Write: key_len (4 bytes) + key + value_len (4 bytes) + value + crc32 (4 bytes)
        // 3. Flush the writer
        // 4. Update keydir with the offset
        todo!("Implement set")
    }

    /// Read a value by key.
    pub fn get(&self, key: &str) -> Result<Vec<u8>> {
        // TODO:
        // 1. Look up offset in keydir
        // 2. Open file, seek to offset
        // 3. Read the entry and return the value
        todo!("Implement get")
    }

    /// Delete a key by writing a tombstone.
    pub fn delete(&mut self, key: &str) -> Result<()> {
        // TODO: Write a tombstone entry (value = None) and remove from keydir
        todo!("Implement delete")
    }

    /// Rebuild keydir by scanning all entries in the log file.
    fn rebuild_keydir(path: &Path) -> Result<HashMap<String, u64>> {
        // TODO:
        // 1. Open the data file for reading
        // 2. Read entries sequentially: key_len, key, value_len, value, crc
        // 3. For each entry: if value present, add to keydir; if tombstone, remove
        // 4. Return the final keydir
        todo!("Implement rebuild_keydir")
    }
}

fn main() {
    println!("=== Chapter 3: Persistent Storage — BitCask ===");
    println!("Exercise: Implement a log-structured storage engine.");
    println!("Run `cargo test --bin exercise` to check your implementation.");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_dir(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("toydb_ch03_{}", name));
        let _ = fs::remove_dir_all(&p);
        p
    }

    #[test]
    fn test_set_and_get() {
        let dir = temp_dir("set_get");
        let mut bc = BitCask::open(&dir).unwrap();
        bc.set("hello", b"world").unwrap();
        assert_eq!(bc.get("hello").unwrap(), b"world");
    }

    #[test]
    fn test_overwrite() {
        let dir = temp_dir("overwrite");
        let mut bc = BitCask::open(&dir).unwrap();
        bc.set("k", b"v1").unwrap();
        bc.set("k", b"v2").unwrap();
        assert_eq!(bc.get("k").unwrap(), b"v2");
    }

    #[test]
    fn test_delete() {
        let dir = temp_dir("delete");
        let mut bc = BitCask::open(&dir).unwrap();
        bc.set("k", b"v").unwrap();
        bc.delete("k").unwrap();
        assert!(bc.get("k").is_err());
    }

    #[test]
    fn test_persistence() {
        let dir = temp_dir("persist");
        {
            let mut bc = BitCask::open(&dir).unwrap();
            bc.set("persist", b"yes").unwrap();
        }
        // Reopen
        let bc = BitCask::open(&dir).unwrap();
        assert_eq!(bc.get("persist").unwrap(), b"yes");
    }
}
