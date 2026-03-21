# Chapter 3: Persistent Storage — BitCask

Every database you have used promises the same thing: your data will survive a power outage, a crash, a restart. The in-memory storage engine from Chapter 2 breaks that promise — kill the process and everything evaporates. This chapter fixes that. You will build a BitCask-style log-structured storage engine that appends every write to a file on disk and maintains an in-memory index for fast lookups.

By the end of this chapter, you will have:

- A `LogStorage` struct that persists data to an append-only log file
- An in-memory `HashMap` index that maps keys to file offsets for O(1) reads
- Tombstone-based deletes and a full startup recovery flow
- CRC32 checksums for data integrity and truncated-record handling for crash safety
- A clear understanding of Rust's file I/O, the `?` operator, and custom error types

---

## Spotlight: File I/O & Error Handling

Every chapter has one spotlight concept. This chapter's spotlight is **file I/O and error handling** — how Rust reads and writes files, and how it forces you to deal with everything that can go wrong.

### Opening and creating files

Rust's `std::fs::File` is your handle to a file on disk. The simplest way to open one:

```rust
use std::fs::File;

let file = File::open("data.log")?;       // read-only, fails if missing
let file = File::create("data.log")?;      // write-only, truncates if exists
```

But databases need more control. `OpenOptions` lets you specify exactly what you want:

```rust
use std::fs::OpenOptions;

let file = OpenOptions::new()
    .read(true)
    .write(true)
    .create(true)       // create if missing, keep contents if exists
    .append(true)       // all writes go to end of file
    .open("data.log")?;
```

Each method call returns the same `OpenOptions` builder, letting you chain them. This is the **builder pattern** — common in Rust when a constructor has many optional parameters.

### Buffered I/O

Every `write()` call to a `File` is a system call — a round trip to the operating system kernel. Writing one record at a time is like mailing one letter at a time instead of batching them. Rust provides `BufWriter` and `BufReader` to batch I/O operations:

```rust
use std::io::{BufWriter, BufReader, Write, Read, Seek};

let file = File::create("data.log")?;
let mut writer = BufWriter::new(file);
writer.write_all(b"hello")?;   // buffered — may not hit disk yet
writer.flush()?;                // forces the buffer to disk
```

`BufWriter` collects small writes into an internal buffer (default 8 KB) and flushes them in one system call when the buffer is full or when you call `flush()`. `BufReader` does the same for reads — it reads a large chunk from disk and serves small `read()` calls from the buffer.

### The `?` operator: error propagation

Notice those `?` marks at the end of file operations. Every I/O operation in Rust can fail — the file might not exist, the disk might be full, permissions might be wrong. Rust does not use exceptions. Instead, functions return `Result<T, E>`:

```rust
enum Result<T, E> {
    Ok(T),    // success — contains the value
    Err(E),   // failure — contains the error
}
```

The `?` operator is syntactic sugar for "if this is an error, return it immediately; if it is OK, unwrap the value":

```rust
// These two are equivalent:
let file = File::open("data.log")?;

let file = match File::open("data.log") {
    Ok(f) => f,
    Err(e) => return Err(e.into()),
};
```

The `?` operator does one more thing: it calls `.into()` on the error, which converts it to the error type your function returns. This means you can use `?` with different error types as long as they can convert into your function's error type. We will use this shortly with custom errors.

### Custom error types

Real applications have multiple kinds of errors — I/O errors, data corruption, missing keys. Rust models these with enums:

```rust
use std::fmt;

#[derive(Debug)]
pub enum StorageError {
    Io(std::io::Error),
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

impl From<std::io::Error> for StorageError {
    fn from(e: std::io::Error) -> Self {
        StorageError::Io(e)
    }
}
```

That `From<std::io::Error>` impl is the bridge that makes `?` work. When a `File::open()` returns `Err(io::Error)`, the `?` operator calls `StorageError::from(e)` to convert it into your custom error type. No try/catch, no error swallowing — every error path is explicit and type-checked.

> **Coming from JS?**
>
> JavaScript uses `try/catch` with untyped exceptions — any value can be thrown, and you discover what went wrong at runtime:
>
> ```javascript
> try {
>   const data = fs.readFileSync("data.log");
> } catch (e) {
>   // e could be anything — no guarantee it has .code or .message
>   console.error(e);
> }
> ```
>
> Rust's `Result<T, E>` makes the error type part of the function signature. You cannot call `File::open()` without handling the possible `io::Error` — the compiler refuses to compile code that ignores a `Result`.

> **Coming from Python?**
>
> Python's `try/except` is similar to JavaScript — exceptions are untyped by default, and you learn what they are by reading docs or catching `Exception`:
>
> ```python
> try:
>     with open("data.log") as f:
>         data = f.read()
> except FileNotFoundError:
>     pass  # handle it
> except PermissionError:
>     pass  # handle it differently
> ```
>
> Rust's `match` on `Result` is the same pattern, but enforced by the compiler. You cannot accidentally forget a case because the `match` must be exhaustive.

> **Coming from Go?**
>
> Go is the closest to Rust — explicit error returns instead of exceptions:
>
> ```go
> file, err := os.Open("data.log")
> if err != nil {
>     return err
> }
> ```
>
> Rust's `?` operator is like Go's `if err != nil { return err }` compressed into a single character. But Rust adds type safety: the error type is part of the function signature, and the `From` trait handles conversions. Go's `error` interface is stringly-typed — you compare `err.Error()` strings or use `errors.Is()`. Rust's error enums let you pattern match on specific variants at compile time.

---

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

## Rust Gym

Time for reps. These drills focus on file I/O and error handling — the spotlight concept for this chapter.

### Drill 1: Line Counter

Read a file line by line, count the total lines and total bytes. Use `BufReader` and `lines()`.

```rust
use std::fs::File;
use std::io::{BufRead, BufReader};

fn count_file(path: &str) -> Result<(usize, usize), std::io::Error> {
    // Your code here
    // Return (line_count, byte_count)
    todo!()
}

fn main() {
    // Create a test file first
    std::fs::write("/tmp/drill1.txt", "hello\nworld\nrust\n").unwrap();

    match count_file("/tmp/drill1.txt") {
        Ok((lines, bytes)) => println!("{} lines, {} bytes", lines, bytes),
        Err(e) => println!("Error: {}", e),
    }
    // Expected: 3 lines, 17 bytes
}
```

<details>
<summary>Solution</summary>

```rust
use std::fs::File;
use std::io::{BufRead, BufReader};

fn count_file(path: &str) -> Result<(usize, usize), std::io::Error> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut line_count = 0;
    let mut byte_count = 0;

    for line in reader.lines() {
        let line = line?;          // each line can fail — handle with ?
        byte_count += line.len() + 1; // +1 for the newline character
        line_count += 1;
    }

    Ok((line_count, byte_count))
}

fn main() {
    std::fs::write("/tmp/drill1.txt", "hello\nworld\nrust\n").unwrap();

    match count_file("/tmp/drill1.txt") {
        Ok((lines, bytes)) => println!("{} lines, {} bytes", lines, bytes),
        Err(e) => println!("Error: {}", e),
    }
}
```

Note that `reader.lines()` returns an iterator of `Result<String, io::Error>` — each line read can independently fail. The `?` inside the loop converts each line's `Result` into the function's return type. This is fundamentally different from Python's `for line in file:`, which silently ignores encoding errors by default.

</details>

### Drill 2: Custom Error Type

Write a `ConfigError` enum with three variants: `FileNotFound(String)`, `ParseError { line: usize, message: String }`, and `MissingKey(String)`. Implement `Display` and `Error` manually (no derive macro libraries). Then write a `load_config()` function that returns `Result<HashMap<String, String>, ConfigError>` and parses a simple `key=value` config file.

```rust
use std::collections::HashMap;
use std::fmt;

#[derive(Debug)]
enum ConfigError {
    // Your variants here
}

// Implement Display and Error traits

fn load_config(path: &str) -> Result<HashMap<String, String>, ConfigError> {
    // Your code here
    // Parse lines like "key=value", error on malformed lines
    todo!()
}

fn main() {
    std::fs::write("/tmp/drill2.conf", "host=localhost\nport=5432\nname=toydb\n").unwrap();

    match load_config("/tmp/drill2.conf") {
        Ok(config) => {
            println!("host = {}", config.get("host").unwrap());
            println!("port = {}", config.get("port").unwrap());
        }
        Err(e) => println!("Config error: {}", e),
    }
    // Expected:
    // host = localhost
    // port = 5432
}
```

<details>
<summary>Solution</summary>

```rust
use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::io::{BufRead, BufReader};

#[derive(Debug)]
enum ConfigError {
    FileNotFound(String),
    ParseError { line: usize, message: String },
    MissingKey(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::FileNotFound(path) =>
                write!(f, "config file not found: {}", path),
            ConfigError::ParseError { line, message } =>
                write!(f, "parse error on line {}: {}", line, message),
            ConfigError::MissingKey(key) =>
                write!(f, "missing required key: {}", key),
        }
    }
}

impl std::error::Error for ConfigError {}

fn load_config(path: &str) -> Result<HashMap<String, String>, ConfigError> {
    let file = File::open(path).map_err(|_| ConfigError::FileNotFound(path.to_string()))?;
    let reader = BufReader::new(file);
    let mut config = HashMap::new();

    for (i, line) in reader.lines().enumerate() {
        let line = line.map_err(|e| ConfigError::ParseError {
            line: i + 1,
            message: e.to_string(),
        })?;

        let line = line.trim().to_string();
        if line.is_empty() || line.starts_with('#') {
            continue; // skip blank lines and comments
        }

        let parts: Vec<&str> = line.splitn(2, '=').collect();
        if parts.len() != 2 {
            return Err(ConfigError::ParseError {
                line: i + 1,
                message: format!("expected key=value, got: {}", line),
            });
        }

        config.insert(parts[0].trim().to_string(), parts[1].trim().to_string());
    }

    Ok(config)
}

fn main() {
    std::fs::write("/tmp/drill2.conf", "host=localhost\nport=5432\nname=toydb\n").unwrap();

    match load_config("/tmp/drill2.conf") {
        Ok(config) => {
            println!("host = {}", config.get("host").unwrap());
            println!("port = {}", config.get("port").unwrap());
        }
        Err(e) => println!("Config error: {}", e),
    }
}
```

The key technique is `map_err()` — it transforms one error type into another. `File::open()` returns `io::Error`, but our function returns `ConfigError`. The `map_err()` call converts the `io::Error` into a `ConfigError::FileNotFound`. This is the manual alternative to implementing `From<io::Error>` — useful when the conversion needs extra context (like the file path).

</details>

### Drill 3: Write-Ahead Log

Build a simple write-ahead log (WAL) that records `set` and `delete` operations. On restart, replay the log to reconstruct state.

```rust
use std::collections::HashMap;

struct Wal {
    // Your fields here
}

impl Wal {
    fn new(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        // Open file, replay existing log
        todo!()
    }

    fn set(&mut self, key: &str, value: &str) -> Result<(), Box<dyn std::error::Error>> {
        // Write "SET key value\n" to log, update state
        todo!()
    }

    fn delete(&mut self, key: &str) -> Result<(), Box<dyn std::error::Error>> {
        // Write "DEL key\n" to log, update state
        todo!()
    }

    fn get(&self, key: &str) -> Option<&str> {
        // Lookup in current state
        todo!()
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = "/tmp/drill3.wal";
    let _ = std::fs::remove_file(path);

    let mut wal = Wal::new(path)?;
    wal.set("name", "ToyDB")?;
    wal.set("version", "0.1")?;
    wal.delete("version")?;
    println!("name = {:?}", wal.get("name"));       // Some("ToyDB")
    println!("version = {:?}", wal.get("version")); // None
    drop(wal);

    // Reopen — should replay the log
    let wal2 = Wal::new(path)?;
    println!("after restart: name = {:?}", wal2.get("name")); // Some("ToyDB")
    println!("after restart: version = {:?}", wal2.get("version")); // None
    Ok(())
}
```

<details>
<summary>Solution</summary>

```rust
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};

struct Wal {
    file: File,
    state: HashMap<String, String>,
}

impl Wal {
    fn new(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)?;

        let mut state = HashMap::new();

        // Replay existing log
        let reader = BufReader::new(&file);
        for line in reader.lines() {
            let line = line?;
            let parts: Vec<&str> = line.splitn(3, ' ').collect();
            match parts.as_slice() {
                ["SET", key, value] => {
                    state.insert(key.to_string(), value.to_string());
                }
                ["DEL", key] => {
                    state.remove(*key);
                }
                _ => {} // skip malformed lines
            }
        }

        // Reopen for appending (seek to end)
        let file = OpenOptions::new()
            .append(true)
            .open(path)?;

        Ok(Wal { file, state })
    }

    fn set(&mut self, key: &str, value: &str) -> Result<(), Box<dyn std::error::Error>> {
        writeln!(self.file, "SET {} {}", key, value)?;
        self.file.sync_data()?;
        self.state.insert(key.to_string(), value.to_string());
        Ok(())
    }

    fn delete(&mut self, key: &str) -> Result<(), Box<dyn std::error::Error>> {
        writeln!(self.file, "DEL {}", key)?;
        self.file.sync_data()?;
        self.state.remove(key);
        Ok(())
    }

    fn get(&self, key: &str) -> Option<&str> {
        self.state.get(key).map(|s| s.as_str())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = "/tmp/drill3.wal";
    let _ = std::fs::remove_file(path);

    let mut wal = Wal::new(path)?;
    wal.set("name", "ToyDB")?;
    wal.set("version", "0.1")?;
    wal.delete("version")?;
    println!("name = {:?}", wal.get("name"));
    println!("version = {:?}", wal.get("version"));
    drop(wal);

    let wal2 = Wal::new(path)?;
    println!("after restart: name = {:?}", wal2.get("name"));
    println!("after restart: version = {:?}", wal2.get("version"));
    Ok(())
}
```

This text-based WAL is simpler than our binary `LogStorage` but demonstrates the same principle: append operations to a log, replay them on startup. The binary format is more efficient (no parsing overhead, no delimiter escaping issues), but the text format is easier to debug — you can `cat` the file and read it.

</details>

---

## DSA in Context: Log-Structured Storage

You just built a log-structured storage engine. Let us analyze its complexity:

| Operation | Time | Why |
|-----------|------|-----|
| `set(key, value)` | O(1) | Append to end of file, update HashMap |
| `get(key)` | O(1) | HashMap lookup + one file seek + read |
| `delete(key)` | O(1) | Append tombstone, remove from HashMap |
| `rebuild_index()` | O(n) | Scan entire file on startup |
| Space usage | O(n * updates) | Every update adds a new record; old records are dead weight |

The key insight is the **trade-off between write performance and space efficiency.** By making writes O(1) append-only, we pay for it in disk space — every update to a key creates a new record while the old one sits unused in the log.

Compare this to an in-place update model (like a B-tree):

| | Log-structured (BitCask) | In-place (B-tree) |
|---|---|---|
| Write | O(1) — sequential append | O(log n) — find position, possibly split nodes |
| Read | O(1) — HashMap + seek | O(log n) — tree traversal |
| Startup | O(n) — scan the log | O(1) — tree is always up to date |
| Space | Grows with updates | Stable — updates overwrite |

Neither is universally better. Log-structured storage shines when writes dominate reads (event logs, metrics, time-series data). B-trees shine when reads dominate and disk space matters (OLTP databases, file systems).

The startup cost — O(n) index rebuild — is the Achilles' heel of pure BitCask. If the log file grows to 10 GB, startup takes minutes. Real implementations solve this with **hint files** (a snapshot of the index saved periodically) and **compaction** (rewriting the log to remove dead records). We will address compaction in a later chapter.

---

## System Design Corner: Designing a Durable Key-Value Store

In a system design interview, you might hear: *"Design a persistent key-value store that survives crashes."* Here is how to structure your answer using what you built in this chapter.

### The durability spectrum

Not all "persistent" means the same thing:

| Level | Guarantee | Mechanism | Latency |
|-------|-----------|-----------|---------|
| No durability | Data lost on crash | In-memory only | Nanoseconds |
| OS-buffered | Data survives process crash, not power loss | `write()` + `flush()` | Microseconds |
| Disk-durable | Data survives power loss | `write()` + `flush()` + `fsync()` | Milliseconds |
| Replicated | Data survives disk failure | fsync + network replication | 10s of ms |

Our `LogStorage` with `sync_data()` is at level 3 — disk-durable. Level 4 requires replication, which we tackle in Chapters 14-16 with Raft consensus.

### Write-Ahead Logging (WAL)

The pattern we implemented — "write to a durable log before updating any in-memory state" — is called a **write-ahead log (WAL)**. It is the foundation of crash recovery in almost every database:

- **PostgreSQL** writes WAL records before modifying data pages
- **SQLite** uses either WAL mode or rollback journal mode
- **Redis** uses an append-only file (AOF) — nearly identical to our approach
- **LevelDB/RocksDB** write to a WAL before inserting into their in-memory memtable

The principle is simple: if the log is durable, you can always reconstruct the state by replaying it. The in-memory index is an optimization — it avoids scanning the log for every read — but the log is the source of truth.

### BitCask in the real world

Our implementation closely follows **BitCask**, the storage engine created by Basho for the Riak database. The original BitCask paper (2010) describes exactly what we built:

1. Append-only log for writes
2. In-memory hash index for reads
3. Tombstones for deletes
4. Periodic compaction to reclaim space

BitCask's constraint is that the entire key set must fit in memory (the index is a `HashMap`). For a dataset with billions of small keys, this can use gigabytes of RAM. Databases like LevelDB and RocksDB solve this with **LSM trees** (Log-Structured Merge trees), which keep parts of the index on disk using sorted files. LSM trees trade read performance for write performance — we will explore them in later chapters.

### Recovery Time Objective (RTO)

RTO is how long it takes to recover after a failure. For our `LogStorage`:

- **Process crash:** Restart the process, call `rebuild_index()`, done. RTO = time to scan the log file.
- **Disk failure:** Data is gone. RTO = time to restore from backup (if one exists).

The O(n) startup cost directly affects RTO. A 1 GB log file with 10 million records might take 5-10 seconds to scan. This is why hint files and compaction matter in production — they keep the log small and the recovery fast.

> **Interview talking point:** *"We use an append-only log for writes because it provides crash safety — if the process dies mid-write, only the last record is damaged, and we detect it with a CRC32 checksum. We trade disk space for write throughput, and manage space with periodic compaction. The in-memory hash index gives us O(1) reads, with the caveat that all keys must fit in memory. For datasets larger than memory, we would move to an LSM tree approach like LevelDB."*

---

## Design Insight: Pull Complexity Downward

In *A Philosophy of Software Design*, John Ousterhout advises: **"Pull complexity downward."** When a module has unavoidable complexity, push it into the implementation rather than leaking it to callers.

Look at the `LogStorage` API from the caller's perspective:

```rust
let mut store = LogStorage::new("data.log")?;
store.set("name", b"ToyDB")?;
let value = store.get("name")?;
store.delete("name")?;
```

Four lines. Simple. The caller knows nothing about:

- Binary record formats (CRC, key_len, value_len, payload)
- File seeking and buffered I/O
- Tombstone-based deletes
- Index rebuilding on startup
- Crash recovery and truncated record handling
- fsync for durability

All of that complexity is **pulled downward** into the `LogStorage` implementation. The caller's mental model is just "set, get, delete" — the same interface as a `HashMap`. Whether the data lives in memory or on disk, uses checksums or not, syncs every write or batches them — these are implementation details that the caller should never need to think about.

This is why we defined a `Storage` trait with a minimal interface. The trait is the contract: "you give me keys and values, I store them and give them back." The how is entirely the responsibility of the implementation. Tomorrow you could swap `LogStorage` for a `RocksDBStorage` or a `PostgresStorage`, and as long as it implements the `Storage` trait, every caller works unchanged.

The temptation is to do the opposite — expose configuration knobs, require callers to call `fsync()` manually, force them to handle partial writes. This pushes complexity **upward** and makes every caller deal with the same hard problems. Pull it down. Handle it once, correctly, inside the module. Let callers focus on their own problems.

---

## What You Built

In this chapter, you:

1. **Built an append-only log** — a binary file format with CRC32 checksums, key/value encoding, and sequential writes
2. **Added an in-memory index** — a `HashMap<String, u64>` mapping keys to file offsets for O(1) reads
3. **Implemented the Storage trait** — making `LogStorage` interchangeable with any other storage backend
4. **Handled crash recovery** — truncated records, CRC mismatches, and fsync for durability
5. **Practiced file I/O and error handling** — `File`, `OpenOptions`, `BufWriter`, `BufReader`, `Result<T, E>`, the `?` operator, and custom error types

Your data now survives restarts. The `LogStorage` engine is a simplified version of BitCask — the same architecture that powers production databases handling millions of writes per second. In Chapter 4, we will tackle serialization — converting complex Rust types into bytes and back, so our database can store more than raw byte arrays.

---

### DS Deep Dive

Ready to go deeper? This chapter's data structure deep dive explores log-structured storage from first principles — why appending is the most natural way to write to magnetic platters and flash cells, and how LSM trees evolved from the same idea.

**-> [Log-Structured Storage — "Append only, ask questions later"](../ds-narratives/ch03-log-structured-storage.md)**

---

### Reference

The files you built in this chapter:

| Your file | Purpose |
|-----------|---------|
| `src/storage.rs` | `LogStorage` struct — append-only log, in-memory index, crash recovery |
| `src/traits.rs` | `Storage` trait — common interface for all storage backends |
| `src/main.rs` | Main entry point with demo usage |

Key Rust standard library types used:

| Type | Module | Purpose |
|------|--------|---------|
| `File` | `std::fs` | File handle for reading and writing |
| `OpenOptions` | `std::fs` | Builder for opening files with specific permissions |
| `BufWriter` | `std::io` | Buffered writes — batches small writes into large ones |
| `BufReader` | `std::io` | Buffered reads — reads large chunks, serves small ones |
| `SeekFrom` | `std::io` | Enum for file seek positions (Start, End, Current) |
| `HashMap` | `std::collections` | Hash map for the in-memory key-to-offset index |
| `Result<T, E>` | `std::result` | Return type for operations that can fail |
