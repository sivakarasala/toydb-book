/// Chapter 16: Raft Durability — SOLUTION

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

    pub fn append(&mut self, entry: &WalEntry) -> io::Result<()> {
        let mut payload = Vec::new();
        payload.extend_from_slice(&entry.term.to_le_bytes());
        payload.extend_from_slice(&entry.index.to_le_bytes());
        payload.extend_from_slice(&(entry.data.len() as u32).to_le_bytes());
        payload.extend_from_slice(&entry.data);
        let crc = crc32fast::hash(&payload);
        self.writer.write_all(&payload)?;
        self.writer.write_all(&crc.to_le_bytes())?;
        self.writer.flush()
    }
}

pub struct WalReader;

impl WalReader {
    pub fn read_all(path: &Path) -> io::Result<Vec<WalEntry>> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        let mut entries = Vec::new();
        loop {
            let mut buf8 = [0u8; 8];
            if reader.read_exact(&mut buf8).is_err() { break; }
            let term = u64::from_le_bytes(buf8);
            reader.read_exact(&mut buf8)?;
            let index = u64::from_le_bytes(buf8);
            let mut buf4 = [0u8; 4];
            reader.read_exact(&mut buf4)?;
            let data_len = u32::from_le_bytes(buf4) as usize;
            let mut data = vec![0u8; data_len];
            reader.read_exact(&mut data)?;
            reader.read_exact(&mut buf4)?;
            let stored_crc = u32::from_le_bytes(buf4);

            let mut payload = Vec::new();
            payload.extend_from_slice(&term.to_le_bytes());
            payload.extend_from_slice(&index.to_le_bytes());
            payload.extend_from_slice(&(data_len as u32).to_le_bytes());
            payload.extend_from_slice(&data);
            if crc32fast::hash(&payload) != stored_crc { break; }

            entries.push(WalEntry { term, index, data });
        }
        Ok(entries)
    }
}

fn main() {
    println!("=== Chapter 16: WAL — Solution ===");
    let path = std::env::temp_dir().join("toydb_ch16_demo");
    let _ = fs::remove_file(&path);
    let mut w = WalWriter::new(&path).unwrap();
    w.append(&WalEntry { term: 1, index: 1, data: b"SET x 1".to_vec() }).unwrap();
    w.append(&WalEntry { term: 1, index: 2, data: b"SET y 2".to_vec() }).unwrap();
    drop(w);
    for e in WalReader::read_all(&path).unwrap() {
        println!("  [{}/{}] {}", e.term, e.index, String::from_utf8_lossy(&e.data));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn temp_path(name: &str) -> PathBuf { let p = std::env::temp_dir().join(format!("toydb_ch16_s_{}", name)); let _ = fs::remove_file(&p); p }

    #[test] fn test_write_read() {
        let p = temp_path("wr");
        { let mut w = WalWriter::new(&p).unwrap(); w.append(&WalEntry { term: 1, index: 1, data: b"c1".to_vec() }).unwrap(); w.append(&WalEntry { term: 1, index: 2, data: b"c2".to_vec() }).unwrap(); }
        let e = WalReader::read_all(&p).unwrap(); assert_eq!(e.len(), 2); assert_eq!(e[0].data, b"c1");
    }

    #[test] fn test_recovery() {
        let p = temp_path("rec");
        { let mut w = WalWriter::new(&p).unwrap(); w.append(&WalEntry { term: 1, index: 1, data: b"c".to_vec() }).unwrap(); }
        assert_eq!(WalReader::read_all(&p).unwrap().len(), 1);
        { let mut w = WalWriter::new(&p).unwrap(); w.append(&WalEntry { term: 2, index: 2, data: b"c2".to_vec() }).unwrap(); }
        assert_eq!(WalReader::read_all(&p).unwrap().len(), 2);
    }
}
