## Exercise 1: The Append-Only Log

**Goal:** Create a `LogStorage` struct that appends binary records to a file. Each record has a fixed header followed by the key and value bytes.

Think of this like an accountant's ledger. You never erase an entry — you only add new lines at the bottom. If you need to correct something, you write a new entry. The ledger is the source of truth, and you can always replay it from the beginning to reconstruct the current state.

### Step 1: Set up the project

If you are continuing from Chapter 2, add new files to your existing project. If you are starting fresh:

```bash
cargo new toydb
cd toydb
```

Create `src/storage.rs` — this will hold our persistent storage engine. Register it in `src/main.rs`:

```rust
mod storage;

fn main() {
    println!("ToyDB — Chapter 3");
}
```

### Step 2: Define the record format

Every record written to the log file has this binary layout:

```
┌──────────┬──────────┬───────────┬───────────┬──────────┐
│ CRC32    │ key_len  │ value_len │ key bytes │ value    │
│ (4 bytes)│ (4 bytes)│ (4 bytes) │ (variable)│ (variable│
└──────────┴──────────┴───────────┴───────────┴──────────┘
```

The CRC32 checksum covers everything after it — `key_len`, `value_len`, the key, and the value. If the file is truncated mid-write (crash during append), the checksum will not match, and we will know to discard that record.

The header is 12 bytes: 4 for the CRC, 4 for the key length, 4 for the value length. This fixed-size header means we can always read the header first, learn how many bytes to read next, then read the key and value.

### Step 3: Write the storage module

Add this to `src/storage.rs`:

```rust
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{self, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::fmt;

// ── Record header size: CRC32 (4) + key_len (4) + value_len (4) = 12 bytes ──
const HEADER_SIZE: usize = 12;

// ── Error type ──────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum StorageError {
    Io(io::Error),
    CorruptedRecord { offset: u64, message: String },
    KeyNotFound(String),
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StorageError::Io(e) => write!(f, "I/O error: {}", e),
            StorageError::CorruptedRecord { offset, message } =>
                write!(f, "corrupted record at offset {}: {}", offset, message),
            StorageError::KeyNotFound(key) =>
                write!(f, "key not found: {}", key),
        }
    }
}

impl std::error::Error for StorageError {}

impl From<io::Error> for StorageError {
    fn from(e: io::Error) -> Self {
        StorageError::Io(e)
    }
}

// ── CRC32 ───────────────────────────────────────────────────────────────────

/// Compute CRC32 checksum using the IEEE polynomial.
/// This is a simple, dependency-free implementation.
fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            if crc & 1 == 1 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

// ── LogStorage ──────────────────────────────────────────────────────────────

pub struct LogStorage {
    file: File,
    write_pos: u64,
    index: HashMap<String, u64>,
}
```

Let us break this down:

- **`StorageError`** is our custom error enum with three variants: I/O failures, corrupted records (detected by checksum), and missing keys. The `From<io::Error>` impl lets us use `?` with any `std::io` operation.
- **`crc32()`** computes a checksum — a 4-byte fingerprint of the data. If even one bit changes, the checksum changes. We implement it by hand to avoid external dependencies, but in production you would use the `crc32fast` crate.
- **`LogStorage`** has three fields: the open file handle, the current write position (so we know where the next append goes), and the in-memory index mapping keys to their byte offsets in the file.

### Step 4: Implement the constructor and append

Add the `impl` block:

```rust
impl LogStorage {
    /// Open or create the log file. If the file already has data,
    /// rebuild the index by scanning it.
    pub fn new(path: &str) -> Result<Self, StorageError> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)?;

        let write_pos = file.metadata()?.len();

        let mut storage = LogStorage {
            file,
            write_pos,
            index: HashMap::new(),
        };

        if write_pos > 0 {
            storage.rebuild_index()?;
        }

        Ok(storage)
    }

    /// Append a key-value record to the end of the log.
    /// Returns the byte offset where the record was written.
    pub fn append(&mut self, key: &str, value: &[u8]) -> Result<u64, StorageError> {
        let key_bytes = key.as_bytes();
        let key_len = key_bytes.len() as u32;
        let value_len = value.len() as u32;

        // Build the payload (everything after the CRC)
        let mut payload = Vec::with_capacity(8 + key_bytes.len() + value.len());
        payload.extend_from_slice(&key_len.to_le_bytes());
        payload.extend_from_slice(&value_len.to_le_bytes());
        payload.extend_from_slice(key_bytes);
        payload.extend_from_slice(value);

        // Compute CRC over the payload
        let checksum = crc32(&payload);

        // Write: CRC first, then payload
        let offset = self.write_pos;
        self.file.seek(SeekFrom::Start(offset))?;

        let mut writer = BufWriter::new(&self.file);
        writer.write_all(&checksum.to_le_bytes())?;
        writer.write_all(&payload)?;
        writer.flush()?;

        self.write_pos = offset + HEADER_SIZE as u64 + key_bytes.len() as u64 + value.len() as u64;

        Ok(offset)
    }
}
```

Why append-only? Three reasons:

1. **Crash safety.** If the process crashes mid-write, only the last record is damaged. All previous records are intact. With in-place updates, a crash could corrupt existing data.
2. **Simplicity.** No need to find free space, manage fragmentation, or update multiple locations. Just write to the end.
3. **Performance.** Sequential writes are the fastest I/O pattern on both spinning disks (no seek) and SSDs (aligned with the write unit).

The trade-off is disk space — deleted keys still occupy space in the log. We will handle that with tombstones now and compaction in later chapters.

### Step 5: Understand `to_le_bytes()`

The `u32::to_le_bytes()` method converts a 32-bit integer into 4 bytes in **little-endian** order (least significant byte first). We use little-endian because it is the native byte order on x86 and ARM processors — the machines most likely to run this code.

```rust
let n: u32 = 258;
let bytes = n.to_le_bytes();  // [2, 1, 0, 0]
// 258 = 256 + 2 = (1 * 256) + (2 * 1)
// little-endian: least significant byte first
```

To read them back: `u32::from_le_bytes([2, 1, 0, 0])` gives `258`. This round-trip is what makes the binary format work — we write numbers as bytes, and later read those bytes back as numbers.

### Step 6: Test the append

Add a test at the bottom of `src/storage.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_path(name: &str) -> String {
        format!("/tmp/toydb_test_{}.log", name)
    }

    fn cleanup(path: &str) {
        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_append_creates_file() {
        let path = temp_path("append_creates");
        cleanup(&path);

        let mut store = LogStorage::new(&path).unwrap();
        let offset = store.append("hello", b"world").unwrap();
        assert_eq!(offset, 0);

        let meta = fs::metadata(&path).unwrap();
        // Header (12) + key (5) + value (5) = 22 bytes
        assert_eq!(meta.len(), 22);

        cleanup(&path);
    }

    #[test]
    fn test_append_multiple_records() {
        let path = temp_path("append_multi");
        cleanup(&path);

        let mut store = LogStorage::new(&path).unwrap();
        let off1 = store.append("key1", b"value1").unwrap();
        let off2 = store.append("key2", b"value2").unwrap();

        assert_eq!(off1, 0);
        // First record: header (12) + key (4) + value (6) = 22
        assert_eq!(off2, 22);

        cleanup(&path);
    }
}
```

Run the tests:

```bash
cargo test
```

Expected output:

```
running 2 tests
test storage::tests::test_append_creates_file ... ok
test storage::tests::test_append_multiple_records ... ok
```

<details>
<summary>Hint: If tests fail with "Permission denied"</summary>

Make sure `/tmp` is writable. On some systems, you may need to use a different temp directory. You can use `std::env::temp_dir()` for a cross-platform approach:

```rust
fn temp_path(name: &str) -> String {
    let dir = std::env::temp_dir();
    format!("{}/toydb_test_{}.log", dir.display(), name)
}
```

</details>

---

## Exercise 2: The In-Memory Index

**Goal:** Add `set()` and `get()` methods that combine the append-only log with a `HashMap` index for O(1) lookups.

The index is like a filing cabinet's card catalog. The actual documents (values) are stored in the ledger (the log file), but the card catalog (the `HashMap`) tells you exactly which page to flip to. Without the index, finding a value means reading the entire log — O(n). With the index, it is O(1).

### Step 1: Implement `set()` and `get()`

Add these methods to the `impl LogStorage` block:

```rust
    /// Store a key-value pair. Appends to the log and updates the index.
    pub fn set(&mut self, key: &str, value: &[u8]) -> Result<(), StorageError> {
        let offset = self.append(key, value)?;
        self.index.insert(key.to_string(), offset);
        Ok(())
    }

    /// Retrieve the value for a key. Looks up the offset in the index,
    /// then reads the record from disk.
    pub fn get(&mut self, key: &str) -> Result<Option<Vec<u8>>, StorageError> {
        let offset = match self.index.get(key) {
            Some(&off) => off,
            None => return Ok(None),
        };

        self.read_value_at(offset)
    }

    /// Read a record's value from the given file offset.
    fn read_value_at(&mut self, offset: u64) -> Result<Option<Vec<u8>>, StorageError> {
        self.file.seek(SeekFrom::Start(offset))?;
        let mut reader = BufReader::new(&self.file);

        // Read the header
        let mut header = [0u8; HEADER_SIZE];
        reader.read_exact(&mut header)?;

        let stored_crc = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
        let key_len = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;
        let value_len = u32::from_le_bytes([header[8], header[9], header[10], header[11]]) as usize;

        // Read key + value
        let mut payload_after_lens = vec![0u8; key_len + value_len];
        reader.read_exact(&mut payload_after_lens)?;

        // Verify CRC: recompute over key_len + value_len + key + value
        let mut payload_for_crc = Vec::with_capacity(8 + key_len + value_len);
        payload_for_crc.extend_from_slice(&(key_len as u32).to_le_bytes());
        payload_for_crc.extend_from_slice(&(value_len as u32).to_le_bytes());
        payload_for_crc.extend_from_slice(&payload_after_lens);

        let computed_crc = crc32(&payload_for_crc);
        if stored_crc != computed_crc {
            return Err(StorageError::CorruptedRecord {
                offset,
                message: format!(
                    "CRC mismatch: stored={:#010x}, computed={:#010x}",
                    stored_crc, computed_crc
                ),
            });
        }

        // The value is the last value_len bytes of the payload
        let value = payload_after_lens[key_len..].to_vec();

        // Tombstone check: empty value means deleted
        if value.is_empty() {
            return Ok(None);
        }

        Ok(Some(value))
    }
```

The `get()` flow is:

1. Look up the key in the `HashMap` index to get a file offset
2. Seek to that offset in the file
3. Read the 12-byte header to learn the key and value sizes
4. Read the key and value bytes
5. Verify the CRC checksum
6. Return the value (or `None` if it is a tombstone)

Notice the `Option<Vec<u8>>` return type. There are two layers here: the outer `Result` handles I/O errors, and the inner `Option` handles "key not found" vs "key exists." This is idiomatic Rust — `Result<Option<T>, E>` means "this operation can fail (Result), and if it succeeds, the value might not exist (Option)."

### Step 2: Implement index rebuild

When the database restarts, the `HashMap` is empty — it lived in memory and is gone. We need to scan the entire log file and rebuild the index. Add this method:

```rust
    /// Scan the entire log file and rebuild the in-memory index.
    /// This runs once at startup.
    fn rebuild_index(&mut self) -> Result<(), StorageError> {
        self.file.seek(SeekFrom::Start(0))?;
        let file_len = self.file.metadata()?.len();
        let mut pos: u64 = 0;

        while pos < file_len {
            // Check if we have enough bytes for a header
            if pos + HEADER_SIZE as u64 > file_len {
                // Truncated header — partial write from a crash
                eprintln!(
                    "Warning: truncated header at offset {}, truncating file",
                    pos
                );
                self.file.set_len(pos)?;
                self.write_pos = pos;
                return Ok(());
            }

            self.file.seek(SeekFrom::Start(pos))?;
            let mut header = [0u8; HEADER_SIZE];
            if let Err(_) = Read::read_exact(&mut (&self.file), &mut header) {
                // Could not read header — truncate here
                eprintln!(
                    "Warning: unreadable header at offset {}, truncating file",
                    pos
                );
                self.file.set_len(pos)?;
                self.write_pos = pos;
                return Ok(());
            }

            let stored_crc = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
            let key_len = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;
            let value_len = u32::from_le_bytes([header[8], header[9], header[10], header[11]]) as usize;

            let record_size = HEADER_SIZE as u64 + key_len as u64 + value_len as u64;

            // Check if we have enough bytes for the full record
            if pos + record_size > file_len {
                eprintln!(
                    "Warning: truncated record at offset {} (need {} bytes, have {}), truncating file",
                    pos,
                    record_size,
                    file_len - pos
                );
                self.file.set_len(pos)?;
                self.write_pos = pos;
                return Ok(());
            }

            // Read key + value
            let mut payload = vec![0u8; key_len + value_len];
            Read::read_exact(&mut (&self.file), &mut payload)?;

            // Verify CRC
            let mut payload_for_crc = Vec::with_capacity(8 + key_len + value_len);
            payload_for_crc.extend_from_slice(&(key_len as u32).to_le_bytes());
            payload_for_crc.extend_from_slice(&(value_len as u32).to_le_bytes());
            payload_for_crc.extend_from_slice(&payload);

            let computed_crc = crc32(&payload_for_crc);
            if stored_crc != computed_crc {
                eprintln!(
                    "Warning: CRC mismatch at offset {}, truncating file",
                    pos
                );
                self.file.set_len(pos)?;
                self.write_pos = pos;
                return Ok(());
            }

            // Extract the key
            let key = String::from_utf8_lossy(&payload[..key_len]).to_string();

            // If value is empty, it is a tombstone — remove from index
            if value_len == 0 {
                self.index.remove(&key);
            } else {
                self.index.insert(key, pos);
            }

            pos += record_size;
        }

        self.write_pos = pos;
        Ok(())
    }
```

The rebuild logic is straightforward: start at byte 0, read each record's header, validate the CRC, and insert the key into the index. If a key appears multiple times (because it was updated), the last occurrence wins — the `HashMap::insert()` overwrites the previous offset.

The three truncation checks handle crash recovery:
1. **Truncated header** — not enough bytes for even the 12-byte header. Discard.
2. **Truncated record** — header is intact but the key/value bytes are incomplete. Discard.
3. **CRC mismatch** — the bytes are there but corrupted. Discard.

In every case, we truncate the file to the last known-good position. This is safe because we only lose the most recent, incomplete write.

### Step 3: Test the full read/write cycle

Add these tests:

```rust
    #[test]
    fn test_set_and_get() {
        let path = temp_path("set_get");
        cleanup(&path);

        let mut store = LogStorage::new(&path).unwrap();
        store.set("name", b"ToyDB").unwrap();
        store.set("version", b"0.1.0").unwrap();

        let name = store.get("name").unwrap();
        assert_eq!(name, Some(b"ToyDB".to_vec()));

        let version = store.get("version").unwrap();
        assert_eq!(version, Some(b"0.1.0".to_vec()));

        let missing = store.get("nonexistent").unwrap();
        assert_eq!(missing, None);

        cleanup(&path);
    }

    #[test]
    fn test_overwrite_key() {
        let path = temp_path("overwrite");
        cleanup(&path);

        let mut store = LogStorage::new(&path).unwrap();
        store.set("count", b"1").unwrap();
        store.set("count", b"2").unwrap();
        store.set("count", b"3").unwrap();

        let val = store.get("count").unwrap();
        assert_eq!(val, Some(b"3".to_vec()));

        cleanup(&path);
    }

    #[test]
    fn test_persistence_across_restarts() {
        let path = temp_path("persist");
        cleanup(&path);

        // Session 1: write some data
        {
            let mut store = LogStorage::new(&path).unwrap();
            store.set("city", b"Portland").unwrap();
            store.set("state", b"Oregon").unwrap();
        }
        // store is dropped here — file handle closed

        // Session 2: reopen and verify data survived
        {
            let mut store = LogStorage::new(&path).unwrap();
            let city = store.get("city").unwrap();
            assert_eq!(city, Some(b"Portland".to_vec()));

            let state = store.get("state").unwrap();
            assert_eq!(state, Some(b"Oregon".to_vec()));
        }

        cleanup(&path);
    }
```

The persistence test is the most important one. It creates a `LogStorage`, writes data, drops the struct (closing the file handle), then opens a new `LogStorage` pointing to the same file. The `new()` constructor calls `rebuild_index()`, which scans the file and repopulates the `HashMap`. The data survived.

Run the tests:

```bash
cargo test
```

Expected output:

```
running 5 tests
test storage::tests::test_append_creates_file ... ok
test storage::tests::test_append_multiple_records ... ok
test storage::tests::test_set_and_get ... ok
test storage::tests::test_overwrite_key ... ok
test storage::tests::test_persistence_across_restarts ... ok
```

<details>
<summary>Hint: If the persistence test fails with "CRC mismatch"</summary>

Make sure the CRC computation in `append()` and the CRC verification in `rebuild_index()` and `read_value_at()` use the exact same payload layout. The CRC must cover: `key_len (4 bytes) + value_len (4 bytes) + key + value`. If you include or exclude the CRC bytes themselves, the checksums will not match.

</details>

---

## Exercise 3: Implementing the Storage Trait

**Goal:** Make `LogStorage` implement a `Storage` trait so it is interchangeable with the in-memory storage engine from Chapter 2.

In Chapter 2, you built a `MemoryStorage` that stores everything in a `HashMap`. Now you have a `LogStorage` that persists to disk. Both provide `set`, `get`, and `delete` — they should share a common interface. This is what traits are for.

### Step 1: Define the Storage trait

Create `src/traits.rs`:

```rust
/// The Storage trait defines the interface for all storage engines.
/// Any type that implements this trait can be used as the backend
/// for our database.
pub trait Storage {
    /// Store a key-value pair.
    fn set(&mut self, key: &str, value: &[u8]) -> Result<(), Box<dyn std::error::Error>>;

    /// Retrieve the value for a key. Returns None if the key does not exist.
    fn get(&mut self, key: &str) -> Result<Option<Vec<u8>>, Box<dyn std::error::Error>>;

    /// Delete a key. Returns true if the key existed.
    fn delete(&mut self, key: &str) -> Result<bool, Box<dyn std::error::Error>>;

    /// Return all keys in the store (no ordering guarantee).
    fn keys(&self) -> Vec<String>;
}
```

We use `Box<dyn std::error::Error>` as the error type so that each implementation can return its own error type (io::Error, StorageError, etc.) without forcing them all to use the same one. The `dyn` keyword means "dynamic dispatch" — the error type is determined at runtime. This is a trade-off: it is flexible but slightly slower than static dispatch. For a storage engine, the I/O cost dwarfs the dispatch overhead.

Register the module in `src/main.rs`:

```rust
mod storage;
mod traits;

fn main() {
    println!("ToyDB — Chapter 3");
}
```

### Step 2: Implement `delete()` and `keys()` on LogStorage

Before we implement the trait, we need `delete()` and `keys()` methods. A delete in a log-structured store is a **tombstone** — a record with an empty value. When we encounter a tombstone during index rebuild, we remove the key from the index.

Add these methods to `impl LogStorage`:

```rust
    /// Delete a key by writing a tombstone (empty value).
    /// Returns true if the key existed.
    pub fn delete(&mut self, key: &str) -> Result<bool, StorageError> {
        let existed = self.index.contains_key(key);
        if existed {
            self.append(key, b"")?;
            self.index.remove(key);
        }
        Ok(existed)
    }

    /// Return all live keys (excludes deleted keys).
    pub fn keys(&self) -> Vec<String> {
        self.index.keys().cloned().collect()
    }
```

The tombstone approach is elegant: `delete("user:42")` appends a record `[key="user:42", value=b""]` to the log. The index drops the key. If we rebuild the index later, we scan that tombstone record, see the empty value, and remove the key — maintaining consistency.

The downside: tombstones take disk space. A key that was set 100 times and deleted has 101 records in the log, all but the last being dead weight. This is why log-structured stores need **compaction** — a background process that rewrites the log, keeping only the latest record for each live key. We will tackle compaction in a later chapter.

### Step 3: Implement the trait

Add the trait implementation:

```rust
use crate::traits::Storage;

impl Storage for LogStorage {
    fn set(&mut self, key: &str, value: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
        self.set(key, value)?;
        Ok(())
    }

    fn get(&mut self, key: &str) -> Result<Option<Vec<u8>>, Box<dyn std::error::Error>> {
        let result = self.get(key)?;
        Ok(result)
    }

    fn delete(&mut self, key: &str) -> Result<bool, Box<dyn std::error::Error>> {
        let result = self.delete(key)?;
        Ok(result)
    }

    fn keys(&self) -> Vec<String> {
        self.keys()
    }
}
```

Wait — we are calling `self.set()` inside the trait's `set()` implementation. Is this infinite recursion? No. The trait impl calls the **inherent** `set()` method (the one defined directly on `LogStorage`). Rust resolves method calls by preferring inherent methods over trait methods when the receiver type is known. If this feels ambiguous, you can write `LogStorage::set(self, key, value)` to be explicit.

The `?` operator converts `StorageError` into `Box<dyn std::error::Error>` automatically because `StorageError` implements the `Error` trait. This is the `From` conversion at work again.

### Step 4: Write a generic function

Now we can write code that works with any storage backend:

```rust
// In src/main.rs
mod storage;
mod traits;

use storage::LogStorage;
use traits::Storage;

fn populate(store: &mut dyn Storage) -> Result<(), Box<dyn std::error::Error>> {
    store.set("language", b"Rust")?;
    store.set("project", b"ToyDB")?;
    store.set("chapter", b"3")?;
    Ok(())
}

fn dump(store: &mut dyn Storage) {
    for key in store.keys() {
        match store.get(&key) {
            Ok(Some(value)) => {
                let text = String::from_utf8_lossy(&value);
                println!("  {} = {}", key, text);
            }
            Ok(None) => println!("  {} = (deleted)", key),
            Err(e) => println!("  {} = ERROR: {}", key, e),
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = "/tmp/toydb_ch03.log";
    let mut store = LogStorage::new(path)?;

    println!("Writing data...");
    populate(&mut store)?;

    println!("Reading back:");
    dump(&mut store);

    println!("Deleting 'chapter'...");
    store.delete("chapter")?;

    println!("After delete:");
    dump(&mut store);

    Ok(())
}
```

Run it:

```bash
cargo run
```

Expected output (key order may vary — HashMap is unordered):

```
Writing data...
Reading back:
  language = Rust
  project = ToyDB
  chapter = 3
Deleting 'chapter'...
After delete:
  language = Rust
  project = ToyDB
```

Run it again without deleting the file. Because of persistence, the second run will rebuild the index and start with the data from the first run. The `set()` calls will overwrite the existing values (appending new records), and the index will point to the latest offsets.

### Step 5: Test tombstones and persistence together

Add this test:

```rust
    #[test]
    fn test_delete_persists() {
        let path = temp_path("delete_persist");
        cleanup(&path);

        // Session 1: write and delete
        {
            let mut store = LogStorage::new(&path).unwrap();
            store.set("keep", b"yes").unwrap();
            store.set("remove", b"no").unwrap();
            store.delete("remove").unwrap();
        }

        // Session 2: verify delete survived
        {
            let mut store = LogStorage::new(&path).unwrap();
            assert_eq!(store.get("keep").unwrap(), Some(b"yes".to_vec()));
            assert_eq!(store.get("remove").unwrap(), None);
            assert!(!store.keys().contains(&"remove".to_string()));
        }

        cleanup(&path);
    }
```

---

## Exercise 4: Crash Recovery

**Goal:** Make the storage engine survive crashes. Add `fsync()` for durability and verify that the index rebuild handles corrupted records gracefully.

### Step 1: Understanding the durability gap

There is a subtle problem with our current implementation. When you call `write_all()` and `flush()`, the data goes from our `BufWriter` to the operating system's page cache — but it may not be on the physical disk yet. The OS batches disk writes for performance. If the power goes out between `flush()` and the OS's next disk write, your data is lost.

The fix is `fsync()` — a system call that forces the OS to write its page cache to physical media. In Rust:

```rust
self.file.sync_data()?;  // fsync — waits until data is on disk
```

Add `sync_data()` to the `append()` method, right after the `writer.flush()?;` call:

```rust
    pub fn append(&mut self, key: &str, value: &[u8]) -> Result<u64, StorageError> {
        let key_bytes = key.as_bytes();
        let key_len = key_bytes.len() as u32;
        let value_len = value.len() as u32;

        // Build the payload (everything after the CRC)
        let mut payload = Vec::with_capacity(8 + key_bytes.len() + value.len());
        payload.extend_from_slice(&key_len.to_le_bytes());
        payload.extend_from_slice(&value_len.to_le_bytes());
        payload.extend_from_slice(key_bytes);
        payload.extend_from_slice(value);

        // Compute CRC over the payload
        let checksum = crc32(&payload);

        // Write: CRC first, then payload
        let offset = self.write_pos;
        self.file.seek(SeekFrom::Start(offset))?;

        let mut writer = BufWriter::new(&self.file);
        writer.write_all(&checksum.to_le_bytes())?;
        writer.write_all(&payload)?;
        writer.flush()?;

        // Force data to disk — survive power loss
        self.file.sync_data()?;

        self.write_pos = offset + HEADER_SIZE as u64 + key_bytes.len() as u64 + value.len() as u64;

        Ok(offset)
    }
```

The trade-off: `fsync()` is slow. On a spinning disk, it can take 10+ milliseconds. On an SSD, 0.1-1 ms. Databases handle this in several ways:

- **Batch writes.** Group multiple operations into one append, call `fsync()` once. This is what write-ahead logs do.
- **Configurable durability.** Let the user choose: `fsync()` every write (safe, slow), every N writes (compromise), or never (fast, risky). Redis calls these "appendfsync always/everysec/no."
- **Group commit.** Collect writes from multiple clients, flush and sync once. PostgreSQL does this.

For our single-user database, syncing every write is fine. In production, you would make this configurable.

### Step 2: Simulate a crash

The best way to test crash recovery is to write a partial record and verify the database handles it. We cannot easily crash the process mid-write in a test, but we can simulate it by manually truncating the file:

```rust
    #[test]
    fn test_crash_recovery_truncated_record() {
        let path = temp_path("crash_trunc");
        cleanup(&path);

        // Write two valid records
        {
            let mut store = LogStorage::new(&path).unwrap();
            store.set("first", b"safe").unwrap();
            store.set("second", b"also_safe").unwrap();
        }

        // Simulate crash: append garbage (partial third record)
        {
            let mut file = OpenOptions::new().append(true).open(&path).unwrap();
            // Write a partial header (only 6 bytes instead of 12)
            file.write_all(&[0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x00]).unwrap();
            file.sync_data().unwrap();
        }

        // Reopen — should recover first two records, discard partial third
        {
            let mut store = LogStorage::new(&path).unwrap();
            assert_eq!(store.get("first").unwrap(), Some(b"safe".to_vec()));
            assert_eq!(store.get("second").unwrap(), Some(b"also_safe".to_vec()));

            // The file should be truncated to remove the garbage
            let meta = std::fs::metadata(&path).unwrap();
            let expected_len = (HEADER_SIZE + 5 + 4) + (HEADER_SIZE + 6 + 9); // first + second
            assert_eq!(meta.len() as usize, expected_len);
        }

        cleanup(&path);
    }

    #[test]
    fn test_crash_recovery_bad_crc() {
        let path = temp_path("crash_crc");
        cleanup(&path);

        // Write one valid record
        {
            let mut store = LogStorage::new(&path).unwrap();
            store.set("good", b"data").unwrap();
        }

        let good_file_len = std::fs::metadata(&path).unwrap().len();

        // Append a record with a bad CRC
        {
            let mut file = OpenOptions::new().append(true).open(&path).unwrap();
            // Write a complete but corrupted record
            let bad_crc: u32 = 0xDEADBEEF;
            let key_len: u32 = 3;
            let value_len: u32 = 3;
            file.write_all(&bad_crc.to_le_bytes()).unwrap();
            file.write_all(&key_len.to_le_bytes()).unwrap();
            file.write_all(&value_len.to_le_bytes()).unwrap();
            file.write_all(b"bad").unwrap();
            file.write_all(b"crc").unwrap();
            file.sync_data().unwrap();
        }

        // Reopen — should recover "good" and discard the corrupted record
        {
            let mut store = LogStorage::new(&path).unwrap();
            assert_eq!(store.get("good").unwrap(), Some(b"data".to_vec()));

            // File should be truncated back to just the good record
            let meta = std::fs::metadata(&path).unwrap();
            assert_eq!(meta.len(), good_file_len);
        }

        cleanup(&path);
    }
```

Run all tests:

```bash
cargo test
```

Expected output:

```
running 7 tests
test storage::tests::test_append_creates_file ... ok
test storage::tests::test_append_multiple_records ... ok
test storage::tests::test_set_and_get ... ok
test storage::tests::test_overwrite_key ... ok
test storage::tests::test_persistence_across_restarts ... ok
test storage::tests::test_crash_recovery_truncated_record ... ok
test storage::tests::test_crash_recovery_bad_crc ... ok
```

### Step 3: Why checksums matter

The CRC32 checksum is a 4-byte fingerprint of the record's content. Here is what it catches:

| Failure mode | Without CRC | With CRC |
|-------------|-------------|----------|
| Power loss mid-write | Read garbage, return wrong value | Detect mismatch, discard record |
| Bit rot (disk degrades over time) | Silently return corrupted data | Detect and report corruption |
| Buggy code writes wrong offset | Read wrong record, return wrong key's value | CRC might catch it (depends on corruption pattern) |

Checksums do not prevent data loss — they prevent **silent** data loss. If a record is corrupted, we know it and can report an error or discard it. Without checksums, we would happily return garbage and the application would never know.

Real databases use stronger checksums (CRC32c, xxHash) or even cryptographic hashes (SHA-256) for critical data. CRC32 is good enough for our purposes — it catches all single-bit errors and most multi-bit errors, with negligible computational cost.

<details>
<summary>Hint: Understanding the CRC32 algorithm</summary>

The CRC32 implementation uses bit manipulation. Here is a step-by-step breakdown:

```rust
fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;        // Start with all bits set
    for &byte in data {
        crc ^= byte as u32;                 // XOR the next byte in
        for _ in 0..8 {                      // Process each bit
            if crc & 1 == 1 {                // If the low bit is set
                crc = (crc >> 1) ^ 0xEDB8_8320; // Shift right and XOR with polynomial
            } else {
                crc >>= 1;                   // Just shift right
            }
        }
    }
    !crc                                     // Invert all bits
}
```

The magic number `0xEDB88320` is the IEEE CRC32 polynomial in reversed bit order. Each byte of input "mixes" into the running CRC value. The final inversion ensures that an all-zero input does not produce an all-zero CRC.

You do not need to memorize this — in production, use the `crc32fast` crate which uses hardware-accelerated instructions (CRC32C on x86). But understanding that it is deterministic and sensitive to any change in the input is the key insight.

</details>

---

## The Complete `storage.rs`

Here is the full storage module after all four exercises:

```rust
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{self, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::fmt;

use crate::traits::Storage;

const HEADER_SIZE: usize = 12;

#[derive(Debug)]
pub enum StorageError {
    Io(io::Error),
    CorruptedRecord { offset: u64, message: String },
    KeyNotFound(String),
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StorageError::Io(e) => write!(f, "I/O error: {}", e),
            StorageError::CorruptedRecord { offset, message } =>
                write!(f, "corrupted record at offset {}: {}", offset, message),
            StorageError::KeyNotFound(key) =>
                write!(f, "key not found: {}", key),
        }
    }
}

impl std::error::Error for StorageError {}

impl From<io::Error> for StorageError {
    fn from(e: io::Error) -> Self {
        StorageError::Io(e)
    }
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            if crc & 1 == 1 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

pub struct LogStorage {
    file: File,
    write_pos: u64,
    index: HashMap<String, u64>,
}

impl LogStorage {
    pub fn new(path: &str) -> Result<Self, StorageError> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)?;

        let write_pos = file.metadata()?.len();

        let mut storage = LogStorage {
            file,
            write_pos,
            index: HashMap::new(),
        };

        if write_pos > 0 {
            storage.rebuild_index()?;
        }

        Ok(storage)
    }

    pub fn append(&mut self, key: &str, value: &[u8]) -> Result<u64, StorageError> {
        let key_bytes = key.as_bytes();
        let key_len = key_bytes.len() as u32;
        let value_len = value.len() as u32;

        let mut payload = Vec::with_capacity(8 + key_bytes.len() + value.len());
        payload.extend_from_slice(&key_len.to_le_bytes());
        payload.extend_from_slice(&value_len.to_le_bytes());
        payload.extend_from_slice(key_bytes);
        payload.extend_from_slice(value);

        let checksum = crc32(&payload);

        let offset = self.write_pos;
        self.file.seek(SeekFrom::Start(offset))?;

        let mut writer = BufWriter::new(&self.file);
        writer.write_all(&checksum.to_le_bytes())?;
        writer.write_all(&payload)?;
        writer.flush()?;
        self.file.sync_data()?;

        self.write_pos = offset + HEADER_SIZE as u64 + key_bytes.len() as u64 + value.len() as u64;

        Ok(offset)
    }

    pub fn set(&mut self, key: &str, value: &[u8]) -> Result<(), StorageError> {
        let offset = self.append(key, value)?;
        self.index.insert(key.to_string(), offset);
        Ok(())
    }

    pub fn get(&mut self, key: &str) -> Result<Option<Vec<u8>>, StorageError> {
        let offset = match self.index.get(key) {
            Some(&off) => off,
            None => return Ok(None),
        };

        self.read_value_at(offset)
    }

    pub fn delete(&mut self, key: &str) -> Result<bool, StorageError> {
        let existed = self.index.contains_key(key);
        if existed {
            self.append(key, b"")?;
            self.index.remove(key);
        }
        Ok(existed)
    }

    pub fn keys(&self) -> Vec<String> {
        self.index.keys().cloned().collect()
    }

    fn read_value_at(&mut self, offset: u64) -> Result<Option<Vec<u8>>, StorageError> {
        self.file.seek(SeekFrom::Start(offset))?;
        let mut reader = BufReader::new(&self.file);

        let mut header = [0u8; HEADER_SIZE];
        reader.read_exact(&mut header)?;

        let stored_crc = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
        let key_len = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;
        let value_len = u32::from_le_bytes([header[8], header[9], header[10], header[11]]) as usize;

        let mut payload_after_lens = vec![0u8; key_len + value_len];
        reader.read_exact(&mut payload_after_lens)?;

        let mut payload_for_crc = Vec::with_capacity(8 + key_len + value_len);
        payload_for_crc.extend_from_slice(&(key_len as u32).to_le_bytes());
        payload_for_crc.extend_from_slice(&(value_len as u32).to_le_bytes());
        payload_for_crc.extend_from_slice(&payload_after_lens);

        let computed_crc = crc32(&payload_for_crc);
        if stored_crc != computed_crc {
            return Err(StorageError::CorruptedRecord {
                offset,
                message: format!(
                    "CRC mismatch: stored={:#010x}, computed={:#010x}",
                    stored_crc, computed_crc
                ),
            });
        }

        let value = payload_after_lens[key_len..].to_vec();

        if value.is_empty() {
            return Ok(None);
        }

        Ok(Some(value))
    }

    fn rebuild_index(&mut self) -> Result<(), StorageError> {
        self.file.seek(SeekFrom::Start(0))?;
        let file_len = self.file.metadata()?.len();
        let mut pos: u64 = 0;

        while pos < file_len {
            if pos + HEADER_SIZE as u64 > file_len {
                eprintln!(
                    "Warning: truncated header at offset {}, truncating file",
                    pos
                );
                self.file.set_len(pos)?;
                self.write_pos = pos;
                return Ok(());
            }

            self.file.seek(SeekFrom::Start(pos))?;
            let mut header = [0u8; HEADER_SIZE];
            if let Err(_) = Read::read_exact(&mut (&self.file), &mut header) {
                eprintln!(
                    "Warning: unreadable header at offset {}, truncating file",
                    pos
                );
                self.file.set_len(pos)?;
                self.write_pos = pos;
                return Ok(());
            }

            let stored_crc = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
            let key_len = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;
            let value_len = u32::from_le_bytes([header[8], header[9], header[10], header[11]]) as usize;

            let record_size = HEADER_SIZE as u64 + key_len as u64 + value_len as u64;

            if pos + record_size > file_len {
                eprintln!(
                    "Warning: truncated record at offset {} (need {} bytes, have {}), truncating file",
                    pos,
                    record_size,
                    file_len - pos
                );
                self.file.set_len(pos)?;
                self.write_pos = pos;
                return Ok(());
            }

            let mut payload = vec![0u8; key_len + value_len];
            Read::read_exact(&mut (&self.file), &mut payload)?;

            let mut payload_for_crc = Vec::with_capacity(8 + key_len + value_len);
            payload_for_crc.extend_from_slice(&(key_len as u32).to_le_bytes());
            payload_for_crc.extend_from_slice(&(value_len as u32).to_le_bytes());
            payload_for_crc.extend_from_slice(&payload);

            let computed_crc = crc32(&payload_for_crc);
            if stored_crc != computed_crc {
                eprintln!(
                    "Warning: CRC mismatch at offset {}, truncating file",
                    pos
                );
                self.file.set_len(pos)?;
                self.write_pos = pos;
                return Ok(());
            }

            let key = String::from_utf8_lossy(&payload[..key_len]).to_string();

            if value_len == 0 {
                self.index.remove(&key);
            } else {
                self.index.insert(key, pos);
            }

            pos += record_size;
        }

        self.write_pos = pos;
        Ok(())
    }
}

impl Storage for LogStorage {
    fn set(&mut self, key: &str, value: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
        self.set(key, value)?;
        Ok(())
    }

    fn get(&mut self, key: &str) -> Result<Option<Vec<u8>>, Box<dyn std::error::Error>> {
        let result = self.get(key)?;
        Ok(result)
    }

    fn delete(&mut self, key: &str) -> Result<bool, Box<dyn std::error::Error>> {
        let result = self.delete(key)?;
        Ok(result)
    }

    fn keys(&self) -> Vec<String> {
        self.keys()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_path(name: &str) -> String {
        format!("/tmp/toydb_test_{}.log", name)
    }

    fn cleanup(path: &str) {
        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_append_creates_file() {
        let path = temp_path("append_creates");
        cleanup(&path);

        let mut store = LogStorage::new(&path).unwrap();
        let offset = store.append("hello", b"world").unwrap();
        assert_eq!(offset, 0);

        let meta = fs::metadata(&path).unwrap();
        assert_eq!(meta.len(), 22);

        cleanup(&path);
    }

    #[test]
    fn test_append_multiple_records() {
        let path = temp_path("append_multi");
        cleanup(&path);

        let mut store = LogStorage::new(&path).unwrap();
        let off1 = store.append("key1", b"value1").unwrap();
        let off2 = store.append("key2", b"value2").unwrap();

        assert_eq!(off1, 0);
        assert_eq!(off2, 22);

        cleanup(&path);
    }

    #[test]
    fn test_set_and_get() {
        let path = temp_path("set_get");
        cleanup(&path);

        let mut store = LogStorage::new(&path).unwrap();
        store.set("name", b"ToyDB").unwrap();
        store.set("version", b"0.1.0").unwrap();

        let name = store.get("name").unwrap();
        assert_eq!(name, Some(b"ToyDB".to_vec()));

        let version = store.get("version").unwrap();
        assert_eq!(version, Some(b"0.1.0".to_vec()));

        let missing = store.get("nonexistent").unwrap();
        assert_eq!(missing, None);

        cleanup(&path);
    }

    #[test]
    fn test_overwrite_key() {
        let path = temp_path("overwrite");
        cleanup(&path);

        let mut store = LogStorage::new(&path).unwrap();
        store.set("count", b"1").unwrap();
        store.set("count", b"2").unwrap();
        store.set("count", b"3").unwrap();

        let val = store.get("count").unwrap();
        assert_eq!(val, Some(b"3".to_vec()));

        cleanup(&path);
    }

    #[test]
    fn test_persistence_across_restarts() {
        let path = temp_path("persist");
        cleanup(&path);

        {
            let mut store = LogStorage::new(&path).unwrap();
            store.set("city", b"Portland").unwrap();
            store.set("state", b"Oregon").unwrap();
        }

        {
            let mut store = LogStorage::new(&path).unwrap();
            let city = store.get("city").unwrap();
            assert_eq!(city, Some(b"Portland".to_vec()));

            let state = store.get("state").unwrap();
            assert_eq!(state, Some(b"Oregon".to_vec()));
        }

        cleanup(&path);
    }

    #[test]
    fn test_delete_persists() {
        let path = temp_path("delete_persist");
        cleanup(&path);

        {
            let mut store = LogStorage::new(&path).unwrap();
            store.set("keep", b"yes").unwrap();
            store.set("remove", b"no").unwrap();
            store.delete("remove").unwrap();
        }

        {
            let mut store = LogStorage::new(&path).unwrap();
            assert_eq!(store.get("keep").unwrap(), Some(b"yes".to_vec()));
            assert_eq!(store.get("remove").unwrap(), None);
            assert!(!store.keys().contains(&"remove".to_string()));
        }

        cleanup(&path);
    }

    #[test]
    fn test_crash_recovery_truncated_record() {
        let path = temp_path("crash_trunc");
        cleanup(&path);

        {
            let mut store = LogStorage::new(&path).unwrap();
            store.set("first", b"safe").unwrap();
            store.set("second", b"also_safe").unwrap();
        }

        {
            let mut file = OpenOptions::new().append(true).open(&path).unwrap();
            file.write_all(&[0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x00]).unwrap();
            file.sync_data().unwrap();
        }

        {
            let mut store = LogStorage::new(&path).unwrap();
            assert_eq!(store.get("first").unwrap(), Some(b"safe".to_vec()));
            assert_eq!(store.get("second").unwrap(), Some(b"also_safe".to_vec()));

            let meta = std::fs::metadata(&path).unwrap();
            let expected_len = (HEADER_SIZE + 5 + 4) + (HEADER_SIZE + 6 + 9);
            assert_eq!(meta.len() as usize, expected_len);
        }

        cleanup(&path);
    }

    #[test]
    fn test_crash_recovery_bad_crc() {
        let path = temp_path("crash_crc");
        cleanup(&path);

        {
            let mut store = LogStorage::new(&path).unwrap();
            store.set("good", b"data").unwrap();
        }

        let good_file_len = std::fs::metadata(&path).unwrap().len();

        {
            let mut file = OpenOptions::new().append(true).open(&path).unwrap();
            let bad_crc: u32 = 0xDEADBEEF;
            let key_len: u32 = 3;
            let value_len: u32 = 3;
            file.write_all(&bad_crc.to_le_bytes()).unwrap();
            file.write_all(&key_len.to_le_bytes()).unwrap();
            file.write_all(&value_len.to_le_bytes()).unwrap();
            file.write_all(b"bad").unwrap();
            file.write_all(b"crc").unwrap();
            file.sync_data().unwrap();
        }

        {
            let mut store = LogStorage::new(&path).unwrap();
            assert_eq!(store.get("good").unwrap(), Some(b"data".to_vec()));

            let meta = std::fs::metadata(&path).unwrap();
            assert_eq!(meta.len(), good_file_len);
        }

        cleanup(&path);
    }
}
```

---
