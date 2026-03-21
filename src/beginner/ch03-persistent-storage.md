# Chapter 3: Persistent Storage — BitCask

Your database has a problem. Close the program and all your data vanishes. Every key, every value — gone. This is because `MemoryStorage` keeps everything in RAM (your computer's temporary memory), and RAM is wiped when the program stops.

Real databases promise something stronger: your data survives restarts, crashes, and power outages. This chapter builds that promise.

You will create a **BitCask-style storage engine** — a design used by real databases (Riak, Bitcask) that writes every operation to a file on disk. It is elegant in its simplicity: append every write to the end of a file, keep an in-memory index for fast lookups, and replay the file on startup to rebuild the index.

By the end of this chapter, you will have:

- A `LogStorage` struct that persists data to an append-only log file
- An in-memory `HashMap` index that maps keys to file offsets for O(1) reads
- CRC32 checksums to detect data corruption
- Startup recovery that rebuilds the index by scanning the log file
- A deep understanding of Rust's file I/O, the `?` operator, and custom error types

---

## Spotlight: File I/O & Error Handling

Every chapter has one spotlight concept. This chapter's spotlight is **file I/O and error handling** — how Rust reads and writes files, and how it forces you to deal with everything that can go wrong.

### Why files?

When you store data in a `BTreeMap`, it lives in RAM. RAM is fast but temporary — it loses its contents when the power goes off. A file lives on your hard drive or SSD, which keeps data even without power. The trade-off: files are slower to access than RAM, but they are durable.

> **Analogy: RAM vs Disk**
>
> RAM is like a whiteboard. It is fast to write on and fast to read, but when you erase it (or the power goes out), everything is gone.
>
> A file on disk is like a notebook. Writing is slower (you need to open the notebook, find the right page, write carefully), but the words stay there until you deliberately erase them.

### Opening files in Rust

Rust's `std::fs::File` is your handle to a file on disk:

```rust
use std::fs::File;

let file = File::open("data.log");
```

But wait — what if the file does not exist? In many languages, this would throw an exception. In Rust, `File::open` returns a `Result`:

```rust
let file: Result<File, std::io::Error> = File::open("data.log");
```

This is either `Ok(file)` (success — the file was opened) or `Err(error)` (failure — maybe the file does not exist, or you do not have permission to read it).

You handle it with `match`:

```rust
match File::open("data.log") {
    Ok(file) => println!("File opened!"),
    Err(error) => println!("Failed to open: {}", error),
}
```

### The `?` operator: error propagation made simple

Writing `match` for every file operation gets tedious fast. The `?` operator is shorthand for "if this fails, return the error immediately":

```rust
fn read_config() -> Result<String, std::io::Error> {
    let mut file = File::open("config.txt")?;   // ? = return error if this fails
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;         // ? again
    Ok(contents)
}
```

Without `?`, this function would be much longer:

```rust
fn read_config() -> Result<String, std::io::Error> {
    let mut file = match File::open("config.txt") {
        Ok(f) => f,
        Err(e) => return Err(e),
    };
    let mut contents = String::new();
    match file.read_to_string(&mut contents) {
        Ok(_) => {},
        Err(e) => return Err(e),
    }
    Ok(contents)
}
```

The `?` version is much cleaner. It does exactly the same thing: if the Result is `Err`, return the error immediately. If it is `Ok`, unwrap the value and continue.

> **What just happened?**
>
> The `?` operator is one of Rust's most useful features. It says "try this, and if it fails, propagate the error to my caller." It only works in functions that return `Result` (because it needs somewhere to send the error).
>
> Think of `?` as a shortcut: instead of writing 4 lines of error handling, you write one character.

### Important rule: `?` only works in functions that return `Result`

This will not compile:

```rust
fn main() {
    let file = File::open("data.log")?;  // ERROR: main does not return Result
}
```

Fix: make `main` return a `Result`, or use `match`/`unwrap` instead:

```rust
fn main() -> Result<(), std::io::Error> {
    let file = File::open("data.log")?;  // OK now
    Ok(())
}
```

### OpenOptions: fine-grained file control

`File::open` opens a file for reading only. `File::create` creates a new file (or truncates an existing one) for writing only. But databases need more control — we want to both read and write, create the file if it does not exist, and append to it instead of overwriting:

```rust
use std::fs::OpenOptions;

let file = OpenOptions::new()
    .read(true)       // allow reading
    .write(true)      // allow writing
    .create(true)     // create if it does not exist
    .append(true)     // all writes go to the end
    .open("data.log")?;
```

Each method call configures one aspect of how the file is opened. This is called the **builder pattern** — a common Rust idiom for constructing things with many options.

> **Analogy: Builder Pattern = Ordering a sandwich**
>
> When you order a sandwich, you do not say everything at once. You say: "I want wheat bread" (`.bread(wheat)`), "add turkey" (`.meat(turkey)`), "add lettuce" (`.lettuce(true)`), "toast it" (`.toasted(true)`). Each step configures one option. At the end, you say "make it" (`.build()`). The builder pattern works the same way.

### Writing to files

Once you have a file handle, you can write bytes to it:

```rust
use std::io::Write;

let mut file = File::create("output.txt")?;
file.write_all(b"Hello, world!")?;
```

The `write_all` method writes all the bytes in the slice to the file. The `b"..."` prefix creates a byte string (`&[u8]`).

### Buffered I/O

Every `write_all` call is a **system call** — a request to the operating system to perform the write. System calls are relatively expensive. If you write one byte at a time, each byte costs a full round trip to the OS kernel.

`BufWriter` batches small writes into one big write:

```rust
use std::io::BufWriter;

let file = File::create("output.txt")?;
let mut writer = BufWriter::new(file);
writer.write_all(b"Hello")?;     // buffered — may not hit disk yet
writer.write_all(b", world!")?;  // buffered
writer.flush()?;                  // NOW it hits disk — one write instead of two
```

> **Analogy: Buffered I/O = Batch mailing**
>
> Instead of walking to the mailbox for each letter, you collect all the letters on your desk and take them all at once. `BufWriter` is your desk — it collects writes and sends them in one batch.

### Custom error types

Real applications have multiple kinds of errors. For our storage engine:
- The file might not exist (I/O error)
- A record might have corrupted data (data integrity error)
- A key might not be found (application logic error)

We model these with an enum:

```rust
#[derive(Debug)]
pub enum StorageError {
    Io(std::io::Error),
    CorruptedRecord { offset: u64, message: String },
    KeyNotFound(String),
}
```

Each variant carries different context. `Io` wraps the standard library's `io::Error`. `CorruptedRecord` carries the file offset and a description. `KeyNotFound` carries the key name.

### The `From` trait: automatic error conversion

To use `?` with our custom error type, we need to tell Rust how to convert `std::io::Error` into `StorageError`:

```rust
impl From<std::io::Error> for StorageError {
    fn from(e: std::io::Error) -> Self {
        StorageError::Io(e)
    }
}
```

Now when a file operation returns `Err(io::Error)`, the `?` operator automatically converts it to `Err(StorageError::Io(e))`. This is why `?` is so powerful — it handles both the error propagation and the type conversion in one character.

> **What just happened?**
>
> The `From` trait is a standard Rust trait that says "I know how to create myself from type X." By implementing `From<io::Error> for StorageError`, we taught Rust how to convert I/O errors into our custom error type. The `?` operator uses this conversion automatically.

### Common mistakes with error handling

**Mistake: Using `unwrap()` in non-test code**

```rust
let file = File::open("data.log").unwrap();  // Crashes if file is missing!
```

`unwrap()` panics (crashes) on errors. Use it in tests and prototypes, but in production code, use `?` or `match`.

**Mistake: Forgetting to `flush()` a BufWriter**

```rust
let mut writer = BufWriter::new(file);
writer.write_all(b"important data")?;
// Forgot flush! Data might be lost if the program crashes.
```

Always call `flush()` after writing data you cannot afford to lose.

**Mistake: Using `?` in a function that returns `()`**

```rust
fn save_data() {  // Returns () — no Result!
    let file = File::open("data.log")?;  // ERROR: ? requires Result return type
}
```

Fix: change the return type to `Result`:

```rust
fn save_data() -> Result<(), StorageError> {
    let file = File::open("data.log")?;  // OK now
    Ok(())
}
```

---

## Exercise 1: Set Up the Storage Module

**Goal:** Create the error type and file structure for our persistent storage engine.

### Step 1: Update the error type

Open `src/error.rs` and update it with additional variants for file operations:

```rust
use std::fmt;
use std::io;

#[derive(Debug)]
pub enum Error {
    /// Key was not found.
    NotFound(String),
    /// An internal error occurred.
    Internal(String),
    /// An I/O error occurred.
    Io(io::Error),
    /// A record in the log file is corrupted.
    Corrupted { offset: u64, message: String },
}
```

We added two new variants:
- `Io(io::Error)` — wraps any standard library I/O error.
- `Corrupted { offset, message }` — reports data corruption at a specific file position.

> **What just happened?**
>
> The `Corrupted` variant uses **named fields** inside the enum. Instead of `Corrupted(u64, String)` (where you have to remember which value is which), we write `Corrupted { offset: u64, message: String }`. This is clearer: `Error::Corrupted { offset: 42, message: "bad checksum".to_string() }` is easier to read than `Error::Corrupted(42, "bad checksum".to_string())`.

### Step 2: Implement Display and From

```rust
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotFound(key) => write!(f, "key not found: {}", key),
            Error::Internal(msg) => write!(f, "internal error: {}", msg),
            Error::Io(e) => write!(f, "I/O error: {}", e),
            Error::Corrupted { offset, message } => {
                write!(f, "corrupted record at offset {}: {}", offset, message)
            }
        }
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}
```

The `From<io::Error>` implementation is the bridge that makes `?` work with file operations. Without it, every `File::open(...)?` would fail to compile because the compiler would not know how to convert `io::Error` into our `Error` type.

### Step 3: Create the log_storage module

Create a new file `src/log_storage.rs` with the imports:

```rust
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{self, BufReader, BufWriter, Read, Seek, SeekFrom, Write};

use crate::error::Error;
use crate::storage::Storage;
```

That is a lot of imports. Let's understand the key ones:

- `BufReader`, `BufWriter` — Buffered wrappers for efficient I/O
- `Read`, `Write` — Traits that provide `read_exact` and `write_all` methods
- `Seek`, `SeekFrom` — Let us jump to a specific position in the file
- `crate::error::Error` — Our custom error type
- `crate::storage::Storage` — The trait from Chapter 2

### Step 4: Register the module

Add to `src/main.rs`:

```rust
mod log_storage;
```

---

## Exercise 2: The Record Format and CRC32

**Goal:** Define the binary format for records and implement the CRC32 checksum function.

### What is a binary format?

When you write text to a file, you are writing human-readable characters. But text is wasteful for storing structured data. The number `42` takes 2 characters as text but only 4 bytes as binary.

Our storage engine uses a **binary format** — data is stored as raw bytes with a fixed structure:

```
┌──────────┬──────────┬───────────┬───────────┬───────────┐
│ CRC32    │ key_len  │ value_len │ key bytes │ value     │
│ (4 bytes)│ (4 bytes)│ (4 bytes) │ (variable)│ (variable)│
└──────────┴──────────┴───────────┴───────────┴───────────┘
```

Let's understand each part:

- **CRC32 (4 bytes)** — A checksum. Think of it as a fingerprint of the data. If even one byte changes (due to disk corruption or a crash during writing), the checksum will not match, and we will know the record is damaged.

- **key_len (4 bytes)** — How many bytes the key occupies.

- **value_len (4 bytes)** — How many bytes the value occupies.

- **key bytes (variable)** — The actual key data.

- **value bytes (variable)** — The actual value data.

The first 12 bytes (CRC + key_len + value_len) are the **header**. Because the header is a fixed size, we always know how many bytes to read first. The header tells us how many more bytes to read for the key and value.

> **Analogy: Binary Format = Shipping label**
>
> Think of a shipping package. The label on the outside has fixed fields: "From:", "To:", "Weight:". These tell the postal worker what is inside without opening the box. Our header is the shipping label — it describes the contents (key length, value length, checksum) without containing the actual data.

### Step 1: Define the header constant

Add to `src/log_storage.rs`:

```rust
/// Record header size: CRC32 (4) + key_len (4) + value_len (4) = 12 bytes
const HEADER_SIZE: usize = 12;
```

### Step 2: Implement CRC32

CRC32 is a checksum algorithm. It takes any amount of data and produces a 4-byte fingerprint. If even one bit of the data changes, the fingerprint changes.

```rust
/// Compute a CRC32 checksum for the given data.
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
```

You do not need to understand the math behind CRC32. What matters is:

- It takes `&[u8]` (a slice of bytes) and returns `u32` (a 32-bit number).
- The same input always produces the same output (deterministic).
- Different inputs almost always produce different outputs.
- It is fast to compute.

> **What just happened?**
>
> We implemented a checksum function. Think of it like a seal on an envelope. Before storing data, we compute the checksum and store it alongside the data. When reading data back, we recompute the checksum and compare. If they match, the data is intact. If they don't, something went wrong.
>
> The `0xFFFF_FFFF` and `0xEDB8_8320` are hexadecimal constants used by the CRC32 algorithm. The underscores in numbers (`0xFFFF_FFFF`) are just for readability — Rust ignores them, like commas in "1,000,000".

### Step 3: Understand `to_le_bytes` and `from_le_bytes`

Before we go further, you need to understand how numbers become bytes and vice versa.

Computers store numbers as bytes. A `u32` is 4 bytes. But in what order? There are two conventions:

- **Little-endian (LE)**: least significant byte first. The number 258 (which is 256 + 2) is stored as `[2, 1, 0, 0]`.
- **Big-endian (BE)**: most significant byte first. The number 258 is stored as `[0, 0, 1, 2]`.

We use little-endian because it is the native byte order of x86 and ARM processors — the CPUs in most computers.

```rust
let n: u32 = 258;
let bytes = n.to_le_bytes();           // [2, 1, 0, 0]
let back = u32::from_le_bytes(bytes);  // 258
assert_eq!(n, back);                   // round-trip works
```

This round-trip — converting a number to bytes, writing them to disk, reading them back, and converting back to a number — is the foundation of our binary format.

### Step 4: Test the checksum

Add tests at the bottom of `src/log_storage.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_deterministic() {
        let data = b"hello world";
        assert_eq!(crc32(data), crc32(data));
    }

    #[test]
    fn crc32_different_inputs() {
        assert_ne!(crc32(b"hello"), crc32(b"world"));
    }

    #[test]
    fn crc32_detects_changes() {
        let original = crc32(b"hello world");
        let modified = crc32(b"hello World");  // capital W
        assert_ne!(original, modified);
    }
}
```

Run the tests:

```bash
cargo test crc32
```

---

## Exercise 3: Build the LogStorage Struct

**Goal:** Create the `LogStorage` struct and implement the ability to append records to the log file.

### Step 1: Define the struct

Add to `src/log_storage.rs`:

```rust
pub struct LogStorage {
    /// The open file handle for both reading and writing
    file: File,
    /// Current position where the next write will go
    write_pos: u64,
    /// In-memory index: key -> byte offset of the latest record
    index: HashMap<String, u64>,
}
```

Three fields:

- `file` — The open file. We keep it open for the lifetime of the storage engine.
- `write_pos` — Where the next record will be written. We always write to the end (append-only).
- `index` — A HashMap mapping each key to the byte offset of its latest value in the file.

> **Analogy: Index = Card catalog**
>
> The log file is like a library with books stacked in the order they arrived. Finding a specific book by searching through the entire stack takes a long time. The index is a card catalog — it tells you exactly where each book is, so you can go straight to it.

### Step 2: Implement the constructor

```rust
impl LogStorage {
    /// Open or create the log file.
    pub fn new(path: &str) -> Result<Self, Error> {
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
}
```

Let's walk through this:

1. `OpenOptions::new()...open(path)?` — Open the file for both reading and writing. Create it if it does not exist. The `?` propagates any I/O error.

2. `file.metadata()?.len()` — Get the file size. Two `?` operators in one line: `metadata()` can fail (returns Result), and the `?` after it unwraps the metadata or returns the error.

3. We create the `LogStorage` with an empty index.

4. If the file is not empty (`write_pos > 0`), we call `rebuild_index()` to scan the file and populate the index.

> **What just happened?**
>
> The constructor opens the file and checks its size. If the file already has data from a previous run, it calls `rebuild_index()` (which we will implement in Exercise 5) to reconstruct the in-memory index. This is how the database recovers after a restart.

### Step 3: Implement the append method

This is the core write operation. Every data modification goes through `append`:

```rust
    /// Append a key-value record to the end of the log file.
    /// Returns the byte offset where the record was written.
    fn append(&mut self, key: &str, value: &[u8]) -> Result<u64, Error> {
        // Convert the key to bytes
        let key_bytes = key.as_bytes();
        let key_len = key_bytes.len() as u32;
        let value_len = value.len() as u32;

        // Build the payload (everything after the CRC)
        let mut payload = Vec::new();
        payload.extend_from_slice(&key_len.to_le_bytes());
        payload.extend_from_slice(&value_len.to_le_bytes());
        payload.extend_from_slice(key_bytes);
        payload.extend_from_slice(value);

        // Compute checksum over the payload
        let checksum = crc32(&payload);

        // Seek to the write position
        let offset = self.write_pos;
        self.file.seek(SeekFrom::Start(offset))?;

        // Write: checksum first, then payload
        let mut writer = BufWriter::new(&self.file);
        writer.write_all(&checksum.to_le_bytes())?;
        writer.write_all(&payload)?;
        writer.flush()?;

        // Update write position
        self.write_pos = offset
            + HEADER_SIZE as u64
            + key_bytes.len() as u64
            + value.len() as u64;

        Ok(offset)
    }
```

Let's break this into pieces:

**Building the payload:**

```rust
let key_bytes = key.as_bytes();
```

Convert the key string to bytes. The string `"hello"` becomes `[104, 101, 108, 108, 111]`.

```rust
let mut payload = Vec::new();
payload.extend_from_slice(&key_len.to_le_bytes());
```

Create an empty byte buffer and add the key length as 4 little-endian bytes.

```rust
payload.extend_from_slice(&value_len.to_le_bytes());
payload.extend_from_slice(key_bytes);
payload.extend_from_slice(value);
```

Add the value length, the key bytes, and the value bytes.

**Computing and writing:**

```rust
let checksum = crc32(&payload);
```

Compute the CRC32 checksum of the payload.

```rust
self.file.seek(SeekFrom::Start(offset))?;
```

Move the file cursor to the write position. `SeekFrom::Start(offset)` means "go to this many bytes from the beginning of the file."

```rust
let mut writer = BufWriter::new(&self.file);
writer.write_all(&checksum.to_le_bytes())?;
writer.write_all(&payload)?;
writer.flush()?;
```

Write the checksum, then the payload, then flush to ensure everything hits disk.

> **What just happened?**
>
> We wrote a binary record to disk. The record has this structure: 4 bytes of checksum, 4 bytes of key length, 4 bytes of value length, the key bytes, and the value bytes. After writing, we advance `write_pos` so the next record goes after this one.
>
> Why append-only? Three reasons:
> 1. **Crash safety.** If the program crashes mid-write, only the last record is damaged. All previous records are intact.
> 2. **Simplicity.** No need to find free space or manage holes in the file.
> 3. **Performance.** Sequential writes are the fastest I/O pattern on both HDDs and SSDs.

### Step 4: Test the append

Add to the test module:

```rust
    fn temp_path(name: &str) -> String {
        format!("/tmp/toydb_test_{}.log", name)
    }

    fn cleanup(path: &str) {
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn append_creates_file() {
        let path = temp_path("append_creates");
        cleanup(&path);

        let mut store = LogStorage::new(&path).unwrap();
        let offset = store.append("hello", b"world").unwrap();
        assert_eq!(offset, 0);  // first record at offset 0

        // Check file size: header (12) + key (5) + value (5) = 22 bytes
        let meta = std::fs::metadata(&path).unwrap();
        assert_eq!(meta.len(), 22);

        cleanup(&path);
    }

    #[test]
    fn append_multiple_records() {
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
```

Run the tests:

```bash
cargo test append
```

---

## Exercise 4: Read Records and Build Set/Get

**Goal:** Implement `read_value_at` to read a record from a specific file offset, then build `set`, `get`, and `delete` methods.

### Step 1: Read a record from the file

Add to `impl LogStorage`:

```rust
    /// Read the value from a record at the given file offset.
    fn read_value_at(&mut self, offset: u64) -> Result<Vec<u8>, Error> {
        // Step 1: Seek to the record
        self.file.seek(SeekFrom::Start(offset))?;

        // Step 2: Read the 12-byte header
        let mut header = [0u8; HEADER_SIZE];
        let mut reader = BufReader::new(&self.file);
        reader.read_exact(&mut header)?;

        // Step 3: Parse the header
        let stored_crc = u32::from_le_bytes([
            header[0], header[1], header[2], header[3],
        ]);
        let key_len = u32::from_le_bytes([
            header[4], header[5], header[6], header[7],
        ]) as usize;
        let value_len = u32::from_le_bytes([
            header[8], header[9], header[10], header[11],
        ]) as usize;

        // Step 4: Read key + value bytes
        let mut data = vec![0u8; key_len + value_len];
        reader.read_exact(&mut data)?;

        // Step 5: Verify the checksum
        let mut payload = Vec::new();
        payload.extend_from_slice(&(key_len as u32).to_le_bytes());
        payload.extend_from_slice(&(value_len as u32).to_le_bytes());
        payload.extend_from_slice(&data);

        let computed_crc = crc32(&payload);
        if computed_crc != stored_crc {
            return Err(Error::Corrupted {
                offset,
                message: format!(
                    "CRC mismatch: expected {}, got {}",
                    stored_crc, computed_crc
                ),
            });
        }

        // Step 6: Return just the value (skip the key bytes)
        let value = data[key_len..].to_vec();
        Ok(value)
    }
```

> **What just happened?**
>
> This method reverses the process of `append`:
>
> 1. **Seek** to the position in the file where the record starts.
> 2. **Read the header** — 12 bytes that tell us the checksum, key length, and value length.
> 3. **Parse the header** by converting bytes back to numbers with `from_le_bytes`.
> 4. **Read the data** — both key and value bytes in one read.
> 5. **Verify the checksum** — recompute CRC32 and compare with the stored one. If they do not match, the data is corrupted.
> 6. **Extract the value** — the value starts after the key bytes. `data[key_len..]` is a slice from position `key_len` to the end.
>
> The `[0u8; HEADER_SIZE]` syntax creates an array of 12 zero bytes. The `vec![0u8; key_len + value_len]` creates a Vec of zeros with the specified length. Both are buffers that `read_exact` fills with data from the file.

### Step 2: Implement set, get, and delete

```rust
    /// Store a key-value pair.
    pub fn store(&mut self, key: &str, value: &[u8]) -> Result<(), Error> {
        let offset = self.append(key, value)?;
        self.index.insert(key.to_string(), offset);
        Ok(())
    }

    /// Retrieve the value for a key.
    pub fn fetch(&mut self, key: &str) -> Result<Option<Vec<u8>>, Error> {
        match self.index.get(key) {
            Some(&offset) => {
                let value = self.read_value_at(offset)?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    /// Delete a key by writing a tombstone (empty value).
    pub fn remove(&mut self, key: &str) -> Result<(), Error> {
        if self.index.contains_key(key) {
            self.append(key, b"")?;  // tombstone
            self.index.remove(key);
        }
        Ok(())
    }
```

Let's understand each one:

**`store`** — Appends the record to the file and updates the index with the new offset. If the key already exists, the old offset is replaced. The old record is still in the file (append-only — we never delete from the file), but the index now points to the new one.

**`fetch`** — Looks up the key in the index to get the file offset. If found, reads the value from the file. If not found, returns `None`.

**`remove`** — Writes a **tombstone** — a record with an empty value. Then removes the key from the index. The tombstone is critical for recovery: when replaying the log, an empty value signals "this key was deleted."

> **Analogy: Tombstone**
>
> In a graveyard, a tombstone marks where someone is buried. In our database, a tombstone (empty value) marks where a key was "buried" (deleted). During recovery, when we encounter a tombstone, we remove the key from the index.

### Step 3: Test set, get, and delete

```rust
    #[test]
    fn store_and_fetch() {
        let path = temp_path("store_fetch");
        cleanup(&path);

        let mut store = LogStorage::new(&path).unwrap();
        store.store("name", b"toydb").unwrap();

        let value = store.fetch("name").unwrap();
        assert_eq!(value, Some(b"toydb".to_vec()));

        cleanup(&path);
    }

    #[test]
    fn fetch_missing_key() {
        let path = temp_path("fetch_missing");
        cleanup(&path);

        let mut store = LogStorage::new(&path).unwrap();
        let value = store.fetch("nonexistent").unwrap();
        assert_eq!(value, None);

        cleanup(&path);
    }

    #[test]
    fn store_overwrites() {
        let path = temp_path("store_overwrite");
        cleanup(&path);

        let mut store = LogStorage::new(&path).unwrap();
        store.store("key", b"first").unwrap();
        store.store("key", b"second").unwrap();

        let value = store.fetch("key").unwrap();
        assert_eq!(value, Some(b"second".to_vec()));

        cleanup(&path);
    }

    #[test]
    fn remove_key() {
        let path = temp_path("remove_key");
        cleanup(&path);

        let mut store = LogStorage::new(&path).unwrap();
        store.store("key", b"value").unwrap();
        store.remove("key").unwrap();

        let value = store.fetch("key").unwrap();
        assert_eq!(value, None);

        cleanup(&path);
    }
```

---

## Exercise 5: Rebuild the Index on Startup

**Goal:** Implement `rebuild_index` — the method that scans the log file from the beginning and reconstructs the index.

### Why rebuild?

The index lives in memory. When the program stops, the index is lost. But the log file on disk has every record ever written. By scanning the file and replaying every record, we reconstruct the index.

### Step 1: Implement rebuild_index

```rust
    /// Scan the log file and rebuild the in-memory index.
    fn rebuild_index(&mut self) -> Result<(), Error> {
        self.index.clear();
        let mut offset: u64 = 0;

        loop {
            // Seek to the current offset
            self.file.seek(SeekFrom::Start(offset))?;

            // Try to read a header
            let mut header = [0u8; HEADER_SIZE];
            let mut reader = BufReader::new(&self.file);

            match reader.read_exact(&mut header) {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                    break;  // end of file
                }
                Err(e) => return Err(Error::Io(e)),
            }

            // Parse key_len and value_len from the header
            let key_len = u32::from_le_bytes([
                header[4], header[5], header[6], header[7],
            ]) as usize;
            let value_len = u32::from_le_bytes([
                header[8], header[9], header[10], header[11],
            ]) as usize;

            // Read the key bytes
            let mut key_bytes = vec![0u8; key_len];
            reader.read_exact(&mut key_bytes).map_err(|e| {
                Error::Corrupted {
                    offset,
                    message: format!("could not read key: {}", e),
                }
            })?;

            // Convert key bytes to string
            let key = String::from_utf8(key_bytes).map_err(|e| {
                Error::Corrupted {
                    offset,
                    message: format!("invalid UTF-8 key: {}", e),
                }
            })?;

            // Skip the value bytes (we don't need them for the index)
            // But we need to know the length to advance the offset

            // Update the index
            if value_len == 0 {
                // Tombstone — key was deleted
                self.index.remove(&key);
            } else {
                // Regular record — point to this offset
                self.index.insert(key, offset);
            }

            // Advance to the next record
            offset += HEADER_SIZE as u64
                + key_len as u64
                + value_len as u64;
        }

        Ok(())
    }
```

> **What just happened?**
>
> This method reads the log file from beginning to end, one record at a time:
>
> 1. **Read the header** to get key_len and value_len.
> 2. **Read the key** (we need it for the index).
> 3. **Skip the value** (we only need its length to know how far to advance).
> 4. **Update the index**: if value_len is 0, it is a tombstone (remove the key). Otherwise, add the key and its offset.
> 5. **Advance** to the next record.
> 6. **Repeat** until we hit the end of the file.
>
> The `match reader.read_exact(...)` handles end-of-file by checking `e.kind() == io::ErrorKind::UnexpectedEof`. This means "there were not enough bytes to fill the buffer" — i.e., we reached the end of the file.
>
> The `.map_err(...)` calls convert one error type to another. They are like `From`, but for one-off conversions.

### Step 2: Test recovery across restarts

This test proves your data survives a restart:

```rust
    #[test]
    fn survives_restart() {
        let path = temp_path("survives_restart");
        cleanup(&path);

        // Session 1: write data
        {
            let mut store = LogStorage::new(&path).unwrap();
            store.store("name", b"toydb").unwrap();
            store.store("version", b"0.1").unwrap();
            store.store("language", b"Rust").unwrap();
        }
        // LogStorage is dropped here — simulates program exit

        // Session 2: reopen and verify
        {
            let mut store = LogStorage::new(&path).unwrap();
            assert_eq!(
                store.fetch("name").unwrap(),
                Some(b"toydb".to_vec())
            );
            assert_eq!(
                store.fetch("version").unwrap(),
                Some(b"0.1".to_vec())
            );
            assert_eq!(
                store.fetch("language").unwrap(),
                Some(b"Rust".to_vec())
            );
        }

        cleanup(&path);
    }
```

> **What just happened?**
>
> The curly braces `{ ... }` create scopes. When a scope ends, all variables inside it are dropped (destroyed). This simulates closing and reopening the program. The first scope writes data. The second scope reopens the file and verifies the data is still there.
>
> This is the whole point of persistent storage. The `MemoryStorage` from Chapter 2 would fail this test — its data evaporates when the scope ends.

### Step 3: Test recovery with deletes and overwrites

```rust
    #[test]
    fn survives_restart_with_deletes() {
        let path = temp_path("restart_deletes");
        cleanup(&path);

        {
            let mut store = LogStorage::new(&path).unwrap();
            store.store("a", b"1").unwrap();
            store.store("b", b"2").unwrap();
            store.remove("a").unwrap();
        }

        {
            let mut store = LogStorage::new(&path).unwrap();
            assert_eq!(store.fetch("a").unwrap(), None);
            assert_eq!(store.fetch("b").unwrap(), Some(b"2".to_vec()));
        }

        cleanup(&path);
    }

    #[test]
    fn survives_restart_with_overwrites() {
        let path = temp_path("restart_overwrites");
        cleanup(&path);

        {
            let mut store = LogStorage::new(&path).unwrap();
            store.store("key", b"old").unwrap();
            store.store("key", b"new").unwrap();
        }

        {
            let mut store = LogStorage::new(&path).unwrap();
            assert_eq!(
                store.fetch("key").unwrap(),
                Some(b"new".to_vec())
            );
        }

        cleanup(&path);
    }
```

Run all tests:

```bash
cargo test
```

---

## Exercise 6: Implement the Storage Trait

**Goal:** Make `LogStorage` implement the `Storage` trait so it can be used with the generic `Database<S>`.

### Step 1: Implement the trait

```rust
impl Storage for LogStorage {
    fn set(&mut self, key: String, value: Vec<u8>) -> Result<(), Error> {
        self.store(&key, &value)
    }

    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, Error> {
        // Design tension: the trait says &self but file reading needs &mut self.
        // For now, we check the index only.
        // A full solution uses interior mutability (RefCell/Mutex),
        // which we will learn in later chapters.
        match self.index.get(key) {
            Some(_) => Err(Error::Internal(
                "LogStorage::get needs mutable access; use fetch() method".to_string()
            )),
            None => Ok(None),
        }
    }

    fn delete(&mut self, key: &str) -> Result<(), Error> {
        self.remove(key)
    }

    fn scan(&self) -> Result<Vec<(String, Vec<u8>)>, Error> {
        Err(Error::Internal(
            "LogStorage::scan not yet implemented".to_string()
        ))
    }
}
```

> **What just happened?**
>
> We hit a real design challenge. The `Storage` trait says `get` takes `&self` (read-only), but reading from a file requires seeking, which needs `&mut self`. This is a genuine tension between the trait design and the implementation.
>
> The proper solution uses **interior mutability** — Rust's mechanism for allowing mutation through a shared reference. We will learn about `RefCell` and `Mutex` in later chapters. For now, we note the limitation and use the `fetch` method directly when we need to read.
>
> The `set` and `delete` methods work fine because they take `&mut self`, matching our `store` and `remove` methods.

### Common mistakes

**Mistake: Thinking append-only wastes disk space**

It does use more space than in-place updates (old records stay in the file). But the benefits — crash safety, simplicity, sequential write performance — are worth it. Real BitCask implementations add a **compaction** step that rewrites the file to remove old records. We will build that in a later chapter.

---

## What You Built

This chapter covered the most complex code in the book so far:

1. **Append-only log** — Every write goes to the end of the file. No data is ever overwritten. This is crash-safe by design.

2. **In-memory index** — A `HashMap` maps keys to file offsets for O(1) lookups.

3. **CRC32 checksums** — Each record has a checksum to detect corruption.

4. **Startup recovery** — The log file is scanned from beginning to end to rebuild the index.

5. **Tombstones** — Deleted keys are marked with empty-value records.

6. **File I/O** — You learned `OpenOptions`, `BufReader`, `BufWriter`, `seek`, `read_exact`, `write_all`, and `flush`.

7. **Error handling** — You learned the `?` operator, custom error enums, and the `From` trait.

---

## Exercises

**Exercise 3.1: Add a `list_keys` method**

Return all keys in the index, sorted alphabetically.

<details>
<summary>Hint</summary>

```rust
pub fn list_keys(&self) -> Vec<String> {
    let mut keys: Vec<String> = self.index.keys().cloned().collect();
    keys.sort();
    keys
}
```

</details>

**Exercise 3.2: Track total disk usage**

Add a method `disk_usage(&self) -> u64` that returns total bytes used.

<details>
<summary>Hint</summary>

`self.write_pos` already tracks the file size. Just return it.

</details>

**Exercise 3.3: Add CRC verification to rebuild_index**

Currently, `rebuild_index` does not verify checksums. Add verification and skip corrupted records.

<details>
<summary>Hint</summary>

After reading each record's key and value, recompute the CRC and compare with the stored one. If they don't match, print a warning and skip the record:

```rust
let stored_crc = u32::from_le_bytes([
    header[0], header[1], header[2], header[3],
]);

// Read value bytes too
let mut value_bytes = vec![0u8; value_len];
reader.read_exact(&mut value_bytes)?;

// Build payload for CRC
let mut payload = Vec::new();
payload.extend_from_slice(&(key_len as u32).to_le_bytes());
payload.extend_from_slice(&(value_len as u32).to_le_bytes());
payload.extend_from_slice(&key_bytes);
payload.extend_from_slice(&value_bytes);

let computed_crc = crc32(&payload);
if computed_crc != stored_crc {
    eprintln!("Warning: corrupted record at offset {}", offset);
    break;  // stop at first corruption
}
```

</details>

---

## Key Takeaways

- **Files persist data across restarts.** RAM is fast but temporary. Disk is slower but durable.
- **`Result<T, E>`** models operations that can fail. `Ok(value)` for success, `Err(error)` for failure.
- **The `?` operator** propagates errors and converts types. It only works in functions returning `Result`.
- **`From` trait** enables automatic error conversion for `?`.
- **Append-only logs** are simple and crash-safe. Recovery means replaying the log.
- **In-memory indexes** provide fast lookups at the cost of startup time.
- **CRC32 checksums** detect data corruption.
- **Binary formats** use fixed-size headers for predictable parsing.
- **Tombstones** mark deletions in append-only storage.
