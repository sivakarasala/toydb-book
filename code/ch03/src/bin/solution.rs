/// Chapter 3: Persistent Storage — BitCask — SOLUTION

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
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

pub struct BitCask {
    path: PathBuf,
    writer: BufWriter<File>,
    keydir: HashMap<String, u64>,
}

impl BitCask {
    pub fn open(path: &Path) -> Result<Self> {
        fs::create_dir_all(path)?;
        let file_path = path.join("data.log");

        let keydir = if file_path.exists() {
            Self::rebuild_keydir(&file_path)?
        } else {
            HashMap::new()
        };

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_path)?;

        Ok(BitCask {
            path: file_path,
            writer: BufWriter::new(file),
            keydir,
        })
    }

    pub fn set(&mut self, key: &str, value: &[u8]) -> Result<()> {
        let offset = self.writer.stream_position()?;

        let key_bytes = key.as_bytes();
        let key_len = key_bytes.len() as u32;
        let val_len = value.len() as u32;

        // Build data for CRC: key_len + key + val_len + value
        let mut data = Vec::new();
        data.extend_from_slice(&key_len.to_le_bytes());
        data.extend_from_slice(key_bytes);
        data.extend_from_slice(&val_len.to_le_bytes());
        data.extend_from_slice(value);

        let crc = crc32fast::hash(&data);

        self.writer.write_all(&data)?;
        self.writer.write_all(&crc.to_le_bytes())?;
        self.writer.flush()?;

        self.keydir.insert(key.to_string(), offset);
        Ok(())
    }

    pub fn get(&self, key: &str) -> Result<Vec<u8>> {
        let offset = self
            .keydir
            .get(key)
            .ok_or_else(|| BitCaskError::KeyNotFound(key.to_string()))?;

        let mut file = File::open(&self.path)?;
        file.seek(SeekFrom::Start(*offset))?;
        let mut reader = BufReader::new(file);

        let mut buf4 = [0u8; 4];

        // Read key
        reader.read_exact(&mut buf4)?;
        let key_len = u32::from_le_bytes(buf4) as usize;
        let mut key_buf = vec![0u8; key_len];
        reader.read_exact(&mut key_buf)?;

        // Read value
        reader.read_exact(&mut buf4)?;
        let val_len = u32::from_le_bytes(buf4) as usize;
        let mut val_buf = vec![0u8; val_len];
        reader.read_exact(&mut val_buf)?;

        // Read and verify CRC
        reader.read_exact(&mut buf4)?;
        let stored_crc = u32::from_le_bytes(buf4);

        let mut data = Vec::new();
        data.extend_from_slice(&(key_len as u32).to_le_bytes());
        data.extend_from_slice(&key_buf);
        data.extend_from_slice(&(val_len as u32).to_le_bytes());
        data.extend_from_slice(&val_buf);

        if crc32fast::hash(&data) != stored_crc {
            return Err(BitCaskError::CorruptEntry);
        }

        Ok(val_buf)
    }

    pub fn delete(&mut self, key: &str) -> Result<()> {
        // Write empty value as tombstone
        self.set(key, b"")?;
        self.keydir.remove(key);
        Ok(())
    }

    fn rebuild_keydir(path: &Path) -> Result<HashMap<String, u64>> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        let mut keydir = HashMap::new();
        let mut buf4 = [0u8; 4];

        loop {
            let offset = reader.stream_position()?;

            // Try reading key_len
            if reader.read_exact(&mut buf4).is_err() {
                break; // EOF
            }
            let key_len = u32::from_le_bytes(buf4) as usize;
            let mut key_buf = vec![0u8; key_len];
            reader.read_exact(&mut key_buf)?;
            let key = String::from_utf8_lossy(&key_buf).to_string();

            // Read value
            reader.read_exact(&mut buf4)?;
            let val_len = u32::from_le_bytes(buf4) as usize;
            let mut val_buf = vec![0u8; val_len];
            reader.read_exact(&mut val_buf)?;

            // Skip CRC
            reader.read_exact(&mut buf4)?;

            if val_len == 0 {
                keydir.remove(&key); // tombstone
            } else {
                keydir.insert(key, offset);
            }
        }

        Ok(keydir)
    }
}

fn main() {
    println!("=== Chapter 3: BitCask — Solution ===");
    let dir = std::env::temp_dir().join("toydb_ch03_demo");
    let _ = fs::remove_dir_all(&dir);

    let mut bc = BitCask::open(&dir).unwrap();
    bc.set("name", b"ToyDB").unwrap();
    bc.set("version", b"0.1").unwrap();

    println!("name = {}", String::from_utf8_lossy(&bc.get("name").unwrap()));
    println!(
        "version = {}",
        String::from_utf8_lossy(&bc.get("version").unwrap())
    );

    drop(bc);
    println!("-- Reopened --");
    let bc = BitCask::open(&dir).unwrap();
    println!("name = {}", String::from_utf8_lossy(&bc.get("name").unwrap()));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("toydb_ch03_sol_{}", name));
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
        let bc = BitCask::open(&dir).unwrap();
        assert_eq!(bc.get("persist").unwrap(), b"yes");
    }
}
