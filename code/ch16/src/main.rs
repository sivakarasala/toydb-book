/// Chapter 16: Raft — Durability & Recovery
/// Exercise: Build a WAL with CRC checksums.

use std::fs::{self, File, OpenOptions};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct WalEntry { pub term: u64, pub index: u64, pub data: Vec<u8> }

pub struct WalWriter { writer: BufWriter<File> }

impl WalWriter {
    pub fn new(path: &Path) -> io::Result<Self> {
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        Ok(WalWriter { writer: BufWriter::new(file) })
    }

    /// Write an entry: [term:8][index:8][data_len:4][data][crc:4]
    pub fn append(&mut self, entry: &WalEntry) -> io::Result<()> {
        // TODO: Write term (8 bytes LE), index (8 bytes LE), data_len (4 bytes LE), data, CRC32 (4 bytes LE)
        // CRC should cover term+index+data_len+data
        // Don't forget to flush!
        todo!("Implement WalWriter::append")
    }
}

pub struct WalReader;

impl WalReader {
    /// Read all entries from a WAL file.
    pub fn read_all(path: &Path) -> io::Result<Vec<WalEntry>> {
        // TODO: Open file, loop reading entries until EOF
        // For each entry: read term, index, data_len, data, crc
        // Verify CRC matches; if not, stop reading (truncated/corrupt)
        todo!("Implement WalReader::read_all")
    }
}

fn main() {
    println!("=== Chapter 16: Raft Durability ===");
    println!("Run `cargo test --bin exercise` to check.");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("toydb_ch16_{}", name));
        let _ = fs::remove_file(&p);
        p
    }

    #[test]
    fn test_write_and_read() {
        let path = temp_path("wr");
        {
            let mut w = WalWriter::new(&path).unwrap();
            w.append(&WalEntry { term: 1, index: 1, data: b"SET x 1".to_vec() }).unwrap();
            w.append(&WalEntry { term: 1, index: 2, data: b"SET y 2".to_vec() }).unwrap();
        }
        let entries = WalReader::read_all(&path).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].data, b"SET x 1");
        assert_eq!(entries[1].index, 2);
    }

    #[test]
    fn test_recovery() {
        let path = temp_path("recover");
        {
            let mut w = WalWriter::new(&path).unwrap();
            w.append(&WalEntry { term: 1, index: 1, data: b"cmd".to_vec() }).unwrap();
        }
        // Simulate crash and reopen
        let entries = WalReader::read_all(&path).unwrap();
        assert_eq!(entries.len(), 1);
        // Append more after recovery
        {
            let mut w = WalWriter::new(&path).unwrap();
            w.append(&WalEntry { term: 2, index: 2, data: b"cmd2".to_vec() }).unwrap();
        }
        let entries = WalReader::read_all(&path).unwrap();
        assert_eq!(entries.len(), 2);
    }
}
