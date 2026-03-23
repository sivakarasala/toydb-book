/// Write-Ahead Log (Ch16)
///
/// Binary format per entry: [term:8][index:8][data_len:4][data][crc32:4]
/// CRC covers term + index + data_len + data.

use std::fs::{File, OpenOptions};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::Path;

use super::LogEntry;

pub struct WalWriter {
    writer: BufWriter<File>,
}

impl WalWriter {
    pub fn new(path: &Path) -> io::Result<Self> {
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        Ok(WalWriter { writer: BufWriter::new(file) })
    }

    pub fn append(&mut self, entry: &LogEntry) -> io::Result<()> {
        let data = entry.command.as_bytes();
        let mut payload = Vec::new();
        payload.extend_from_slice(&entry.term.to_le_bytes());
        payload.extend_from_slice(&entry.index.to_le_bytes());
        payload.extend_from_slice(&(data.len() as u32).to_le_bytes());
        payload.extend_from_slice(data);
        let crc = crc32fast::hash(&payload);
        self.writer.write_all(&payload)?;
        self.writer.write_all(&crc.to_le_bytes())?;
        self.writer.flush()
    }
}

pub struct WalReader;

impl WalReader {
    pub fn read_all(path: &Path) -> io::Result<Vec<LogEntry>> {
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

            // Verify CRC
            let mut payload = Vec::new();
            payload.extend_from_slice(&term.to_le_bytes());
            payload.extend_from_slice(&index.to_le_bytes());
            payload.extend_from_slice(&(data_len as u32).to_le_bytes());
            payload.extend_from_slice(&data);
            if crc32fast::hash(&payload) != stored_crc { break; }

            let command = String::from_utf8(data).unwrap_or_default();
            entries.push(LogEntry { term, index, command });
        }
        Ok(entries)
    }
}
