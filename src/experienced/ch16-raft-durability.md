# Chapter 16: Raft — Durability & Recovery

Your Raft cluster elects leaders and replicates log entries. Then the power goes out. Every node restarts with a blank slate — no idea who the leader was, no idea what entries were committed, no idea what term it was in. All of your carefully replicated state is gone. This is not a theoretical concern. Servers crash. Disks fail. Kernels panic. Data centers lose power. A consensus protocol that cannot survive a restart is a toy. This chapter makes it real.

You will build a durable storage layer that persists Raft state to disk using a write-ahead log, implement snapshot creation and transfer so slow followers can catch up without replaying the entire history, and design recovery logic that reconstructs the node's in-memory state from what it wrote to disk before it crashed. The spotlight concept is **ownership and persistence** — how Rust's ownership model naturally maps to the question of "who is responsible for this file handle, and when does it get closed?"

By the end of this chapter, you will have:

- A write-ahead log (WAL) that persists Raft log entries with append-only writes
- Persistent storage for `current_term`, `voted_for`, and `commit_index`
- `fsync`-based durability guarantees that survive process crashes and power loss
- A recovery procedure that reads persisted state on startup and reconstructs the node
- A snapshot mechanism that compacts old log entries into a point-in-time image
- An `InstallSnapshot` RPC for transferring snapshots to lagging followers
- Crash recovery tests that verify correctness after simulated failures

---

## Spotlight: Ownership & Persistence

Every chapter has one spotlight concept. This chapter's spotlight is **ownership and persistence** — how Rust's ownership system governs file handles, buffers, and the lifecycle of persistent state.

### File handles are resources

In languages with garbage collectors, a file handle might stay open long after you stop using it — the GC cleans it up "eventually." In Rust, file handles are owned values. When the owner goes out of scope, the file is closed. This is deterministic resource management, enforced by the compiler:

```rust
use std::fs::File;
use std::io::Write;

fn write_term(path: &str, term: u64) -> std::io::Result<()> {
    let mut file = File::create(path)?;  // file is opened here
    file.write_all(&term.to_le_bytes())?;
    file.sync_all()?;
    Ok(())
    // file is CLOSED here — Drop runs automatically
}
```

There is no `file.close()` call. The `File` struct implements `Drop`, and when `file` goes out of scope at the end of the function, `Drop::drop()` runs, which closes the file descriptor. This is Rust's RAII pattern — Resource Acquisition Is Initialization. You acquire the resource (open the file) when you initialize the variable, and release it (close the file) when the variable is dropped.

### Ownership determines who controls the file

When building a WAL, someone must own the file handle. If the `RaftNode` owns it, the file lives as long as the node. If you put it in a separate `WalWriter` struct, the writer controls the file's lifetime. Ownership is not just a compiler concept — it is a design decision about responsibility:

```rust,ignore
struct WalWriter {
    file: File,        // WalWriter owns the file handle
    path: PathBuf,     // WalWriter knows where the file lives
    offset: u64,       // WalWriter tracks the write position
}

struct RaftNode {
    wal: WalWriter,    // RaftNode owns the WalWriter
    // ...
}
```

When `RaftNode` is dropped, it drops `WalWriter`, which drops `File`, which closes the file descriptor. The ownership chain is explicit, deterministic, and enforced by the compiler. No finalizers, no `try-finally`, no "remember to close the file."

### Mutable references and exclusive file access

Rust's borrow checker guarantees that only one mutable reference exists at a time. For file I/O, this maps directly to exclusive write access:

```rust,ignore
impl WalWriter {
    // Only one caller can append at a time — &mut self guarantees this
    fn append(&mut self, entry: &LogEntry) -> std::io::Result<()> {
        let bytes = entry.serialize();
        self.file.write_all(&bytes)?;
        self.offset += bytes.len() as u64;
        Ok(())
    }
}
```

The `&mut self` signature means you need exclusive access to the `WalWriter` to append. If you tried to share the writer across threads without synchronization, the compiler would refuse. This is the same guarantee that `flock()` provides at the OS level, but enforced at compile time.

### BufWriter and flush semantics

Writing to disk byte-by-byte is slow — each `write()` is a system call. `BufWriter` batches writes into a user-space buffer and flushes them in larger chunks:

```rust
use std::io::BufWriter;

let file = File::create("raft.wal")?;
let mut writer = BufWriter::new(file);

// These go to the buffer, not the disk
writer.write_all(&entry1_bytes)?;
writer.write_all(&entry2_bytes)?;

// This sends the buffer to the OS
writer.flush()?;

// This forces the OS to write to the physical disk
writer.get_ref().sync_all()?;
```

There is a critical distinction: `flush()` moves data from your process's buffer to the OS kernel's buffer. `sync_all()` forces the kernel to write its buffer to the physical disk (the `fsync` system call). For durability, you need both.

> **Coming from JS/Python/Go?**
>
> | Concept | JavaScript | Python | Go | Rust |
> |---------|-----------|--------|-----|------|
> | Open file | `fs.openSync()` | `open()` | `os.Create()` | `File::create()` |
> | Close file | `fs.closeSync(fd)` | `f.close()` | `f.Close()` | Automatic (Drop) |
> | Sync to disk | `fs.fsyncSync(fd)` | `os.fsync(fd)` | `f.Sync()` | `file.sync_all()` |
> | Buffered write | `fs.writeFileSync()` | `io.BufferedWriter` | `bufio.Writer` | `BufWriter::new()` |
> | Exclusive access | Manual locking | Manual locking | `sync.Mutex` | `&mut self` (compile-time) |
> | Resource cleanup | Manual or `finally` | `with` statement | `defer f.Close()` | `Drop` trait (automatic) |
>
> The key difference: in Go, forgetting `defer f.Close()` is a bug that compiles fine and leaks file descriptors. In Python, forgetting `with` is a bug that might or might not cause problems. In Rust, the file is always closed when the owner goes out of scope — you cannot forget, because the compiler handles it. And Rust's `&mut self` replaces mutexes for single-threaded file access — the borrow checker proves at compile time that no two writers exist simultaneously.

---

## Why Durability Matters

Without persistence, your Raft cluster is a distributed in-memory cache with extra steps. Consider what happens when a node crashes:

```
Before crash:
  Node A (Leader):  term=5, log=[1,2,3,4,5], commit_index=4
  Node B (Follower): term=5, log=[1,2,3,4,5], commit_index=4
  Node C (Follower): term=5, log=[1,2,3,4,5], commit_index=4

After Node A crashes and restarts (no persistence):
  Node A:           term=0, log=[], commit_index=0
  Node B (Follower): term=5, log=[1,2,3,4,5], commit_index=4
  Node C (Follower): term=5, log=[1,2,3,4,5], commit_index=4
```

Node A has amnesia. It does not know it was ever leader. It does not know what term the cluster is in. It will start a new election at term 1, which the other nodes will ignore because they are already at term 5. Worse, if two nodes crash simultaneously, the cluster might lose committed data — entries that a majority had acknowledged are now gone from two of three nodes.

The Raft paper is explicit about what must be persisted:

1. **`current_term`** — the latest term the server has seen
2. **`voted_for`** — the candidate this server voted for in the current term (or `None`)
3. **`log[]`** — the log entries themselves

These three pieces of state must survive crashes. Everything else — `commit_index`, `last_applied`, leader identity, match indexes — can be reconstructed from these three plus communication with the cluster.

---

## Exercise 1: The Write-Ahead Log

**Goal:** Build a `WalWriter` that persists Raft log entries to disk using an append-only file with length-prefixed records and CRC checksums.

### Step 1: Define the on-disk format

Each WAL record has a fixed header followed by a variable-length payload:

```
WAL Record Format:
┌──────────────┬──────────────┬──────────────┬─────────────────────────┐
│  4 bytes     │  4 bytes     │  4 bytes     │  N bytes                │
│  CRC32       │  Length (N)  │  Entry Index │  Serialized LogEntry    │
│  (checksum)  │  (payload)   │  (u32)       │  (term + command)       │
└──────────────┴──────────────┴──────────────┴─────────────────────────┘
```

The CRC32 checksum covers the length, index, and payload bytes. On recovery, if the checksum does not match, we know the record was partially written (torn write) and we truncate the log at that point.

### Step 2: Define the log entry serialization

```rust
// src/raft/wal.rs

use std::fs::{File, OpenOptions};
use std::io::{self, BufWriter, Read, Write, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// A single log entry as stored on disk.
#[derive(Debug, Clone, PartialEq)]
pub struct LogEntry {
    pub term: u64,
    pub index: u64,
    pub command: Vec<u8>,
}

impl LogEntry {
    /// Serialize to bytes: 8 bytes term + 8 bytes index + N bytes command.
    pub fn serialize(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(16 + self.command.len());
        bytes.extend_from_slice(&self.term.to_le_bytes());
        bytes.extend_from_slice(&self.index.to_le_bytes());
        bytes.extend_from_slice(&self.command);
        bytes
    }

    /// Deserialize from bytes.
    pub fn deserialize(bytes: &[u8]) -> Result<Self, WalError> {
        if bytes.len() < 16 {
            return Err(WalError::CorruptEntry("entry too short".to_string()));
        }
        let term = u64::from_le_bytes(
            bytes[0..8].try_into().unwrap()
        );
        let index = u64::from_le_bytes(
            bytes[8..16].try_into().unwrap()
        );
        let command = bytes[16..].to_vec();
        Ok(LogEntry { term, index, command })
    }
}
```

### Step 3: Implement CRC32

We use a simple CRC32 implementation. In a production system, you would use a crate like `crc32fast`, but for learning, a lookup-table implementation is instructive:

```rust
// src/raft/wal.rs (continued)

/// Compute CRC32 checksum (IEEE polynomial).
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFFFFFF;
    for &byte in data {
        let index = ((crc ^ byte as u32) & 0xFF) as usize;
        crc = CRC32_TABLE[index] ^ (crc >> 8);
    }
    crc ^ 0xFFFFFFFF
}

/// CRC32 lookup table (IEEE polynomial 0xEDB88320).
const CRC32_TABLE: [u32; 256] = {
    let mut table = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut crc = i as u32;
        let mut j = 0;
        while j < 8 {
            if crc & 1 == 1 {
                crc = (crc >> 1) ^ 0xEDB88320;
            } else {
                crc >>= 1;
            }
            j += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
};
```

This is a compile-time computed lookup table using `const` evaluation. The CRC32 polynomial `0xEDB88320` is the bit-reversed form of the standard IEEE polynomial. The table has 256 entries, one for each possible byte value, allowing us to process input one byte at a time with a single table lookup and XOR per byte.

### Step 4: Build the WAL writer

```rust
// src/raft/wal.rs (continued)

#[derive(Debug)]
pub enum WalError {
    Io(io::Error),
    CorruptEntry(String),
    ChecksumMismatch { expected: u32, actual: u32 },
}

impl From<io::Error> for WalError {
    fn from(e: io::Error) -> Self {
        WalError::Io(e)
    }
}

impl std::fmt::Display for WalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WalError::Io(e) => write!(f, "WAL I/O error: {}", e),
            WalError::CorruptEntry(msg) => write!(f, "corrupt WAL entry: {}", msg),
            WalError::ChecksumMismatch { expected, actual } => {
                write!(f, "checksum mismatch: expected {:#010x}, got {:#010x}", expected, actual)
            }
        }
    }
}

/// Write-ahead log for persisting Raft log entries.
///
/// The WalWriter owns the file handle. When it is dropped, the file is closed.
/// Only one WalWriter should exist per WAL file — Rust's ownership system
/// enforces this naturally.
pub struct WalWriter {
    writer: BufWriter<File>,
    path: PathBuf,
    entry_count: u64,
}

impl WalWriter {
    /// Open or create a WAL file at the given path.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, WalError> {
        let path = path.as_ref().to_path_buf();
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .append(true)
            .open(&path)?;

        let entry_count = 0; // Will be set during recovery

        Ok(WalWriter {
            writer: BufWriter::new(file),
            path,
            entry_count,
        })
    }

    /// Append a log entry to the WAL. Returns the byte offset where
    /// the entry was written.
    pub fn append(&mut self, entry: &LogEntry) -> Result<u64, WalError> {
        let payload = entry.serialize();
        let index_bytes = (entry.index as u32).to_le_bytes();

        // Build the data that the CRC covers: length + index + payload
        let length = payload.len() as u32;
        let mut crc_data = Vec::with_capacity(8 + payload.len());
        crc_data.extend_from_slice(&length.to_le_bytes());
        crc_data.extend_from_slice(&index_bytes);
        crc_data.extend_from_slice(&payload);

        let checksum = crc32(&crc_data);

        // Write: CRC (4) + length (4) + index (4) + payload (N)
        self.writer.write_all(&checksum.to_le_bytes())?;
        self.writer.write_all(&length.to_le_bytes())?;
        self.writer.write_all(&index_bytes)?;
        self.writer.write_all(&payload)?;

        self.entry_count += 1;
        Ok(self.entry_count - 1)
    }

    /// Flush the buffer to the OS and sync to disk.
    /// This is the durability guarantee — after sync() returns,
    /// the data is on the physical disk.
    pub fn sync(&mut self) -> Result<(), WalError> {
        self.writer.flush()?;
        self.writer.get_ref().sync_all()?;
        Ok(())
    }

    /// Append and immediately sync. Use this for critical state changes
    /// (term updates, votes) where losing the write means violating
    /// Raft's safety guarantees.
    pub fn append_sync(&mut self, entry: &LogEntry) -> Result<u64, WalError> {
        let offset = self.append(entry)?;
        self.sync()?;
        Ok(offset)
    }

    /// Return the number of entries written since this writer was opened.
    pub fn entry_count(&self) -> u64 {
        self.entry_count
    }
}
```

### Step 5: Build the WAL reader for recovery

```rust
// src/raft/wal.rs (continued)

/// Reads WAL entries from disk. Used during recovery.
pub struct WalReader {
    file: File,
    path: PathBuf,
}

impl WalReader {
    /// Open a WAL file for reading.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, WalError> {
        let path = path.as_ref().to_path_buf();
        let file = File::open(&path)?;
        Ok(WalReader { file, path })
    }

    /// Read all valid entries from the WAL.
    ///
    /// Stops at the first corrupt or incomplete record. This handles
    /// the case where the process crashed mid-write — the partial
    /// record at the end is silently discarded.
    pub fn read_all(&mut self) -> Result<Vec<LogEntry>, WalError> {
        let mut entries = Vec::new();
        let mut buf = Vec::new();
        self.file.read_to_end(&mut buf)?;

        let mut pos = 0;
        while pos + 12 <= buf.len() {
            // Read header: CRC (4) + length (4) + index (4)
            let stored_crc = u32::from_le_bytes(
                buf[pos..pos + 4].try_into().unwrap()
            );
            let length = u32::from_le_bytes(
                buf[pos + 4..pos + 8].try_into().unwrap()
            ) as usize;
            let _index = u32::from_le_bytes(
                buf[pos + 8..pos + 12].try_into().unwrap()
            );

            // Check if we have enough bytes for the payload
            if pos + 12 + length > buf.len() {
                // Incomplete record — torn write. Stop here.
                break;
            }

            let payload = &buf[pos + 12..pos + 12 + length];

            // Verify checksum: covers length + index + payload
            let crc_data = &buf[pos + 4..pos + 12 + length];
            let computed_crc = crc32(crc_data);

            if stored_crc != computed_crc {
                // Corrupt record. Stop here — everything after
                // this point is suspect.
                break;
            }

            // Deserialize the entry
            let entry = LogEntry::deserialize(payload)?;
            entries.push(entry);

            pos += 12 + length;
        }

        Ok(entries)
    }

    /// Return the path of the WAL file.
    pub fn path(&self) -> &Path {
        &self.path
    }
}
```

### Step 6: Test the WAL

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn test_entry(index: u64, term: u64, cmd: &str) -> LogEntry {
        LogEntry {
            term,
            index,
            command: cmd.as_bytes().to_vec(),
        }
    }

    #[test]
    fn test_wal_write_and_read() {
        let dir = tempfile::tempdir().unwrap();
        let wal_path = dir.path().join("test.wal");

        // Write entries
        {
            let mut writer = WalWriter::open(&wal_path).unwrap();
            writer.append(&test_entry(1, 1, "SET x 1")).unwrap();
            writer.append(&test_entry(2, 1, "SET y 2")).unwrap();
            writer.append(&test_entry(3, 2, "SET z 3")).unwrap();
            writer.sync().unwrap();
        } // writer is dropped here, file is closed

        // Read them back
        let mut reader = WalReader::open(&wal_path).unwrap();
        let entries = reader.read_all().unwrap();

        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].index, 1);
        assert_eq!(entries[0].term, 1);
        assert_eq!(entries[0].command, b"SET x 1");
        assert_eq!(entries[2].index, 3);
        assert_eq!(entries[2].term, 2);
    }

    #[test]
    fn test_wal_survives_torn_write() {
        let dir = tempfile::tempdir().unwrap();
        let wal_path = dir.path().join("torn.wal");

        // Write two good entries
        {
            let mut writer = WalWriter::open(&wal_path).unwrap();
            writer.append(&test_entry(1, 1, "SET x 1")).unwrap();
            writer.append(&test_entry(2, 1, "SET y 2")).unwrap();
            writer.sync().unwrap();
        }

        // Simulate a torn write by appending garbage bytes
        {
            let mut file = OpenOptions::new()
                .append(true)
                .open(&wal_path)
                .unwrap();
            file.write_all(&[0xFF; 20]).unwrap(); // partial/corrupt record
        }

        // Reader should return only the two good entries
        let mut reader = WalReader::open(&wal_path).unwrap();
        let entries = reader.read_all().unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_crc32_detects_corruption() {
        let data = b"hello world";
        let checksum = crc32(data);
        assert_ne!(checksum, 0);

        // Same data produces same checksum
        assert_eq!(crc32(data), checksum);

        // Different data produces different checksum
        let corrupted = b"hello worle";
        assert_ne!(crc32(corrupted), checksum);
    }
}
```

Notice the ownership pattern in the test. The `WalWriter` is created inside a block `{ ... }`. When the block ends, the writer is dropped, which flushes the buffer and closes the file. Only then do we open a `WalReader` for the same file. Rust's scoping rules make this ownership transfer explicit and safe — you cannot accidentally read from a file that is still being written to.

---

## Exercise 2: Persisting Raft Metadata

**Goal:** Build a `RaftState` struct that persists `current_term`, `voted_for`, and `commit_index` to a separate metadata file, with atomic updates using write-then-rename.

### Step 1: The problem with partial writes

If you write `current_term` and the process crashes before writing `voted_for`, the metadata file contains inconsistent state. The solution is **atomic file replacement**: write the complete new state to a temporary file, `fsync` it, then rename the temporary file to the real path. On POSIX systems, `rename()` is atomic — the old file is replaced in a single operation, so readers see either the old state or the new state, never a partial write.

### Step 2: Define the metadata structure

```rust
// src/raft/state.rs

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// Persistent Raft metadata.
///
/// These fields must survive crashes. The Raft paper requires
/// current_term and voted_for to be persisted before responding
/// to any RPC. commit_index is not strictly required (it can be
/// reconstructed), but persisting it avoids re-applying committed
/// entries on recovery.
#[derive(Debug, Clone, PartialEq)]
pub struct RaftState {
    pub current_term: u64,
    pub voted_for: Option<u64>,   // node ID, or None
    pub commit_index: u64,
}

impl RaftState {
    pub fn new() -> Self {
        RaftState {
            current_term: 0,
            voted_for: None,
            commit_index: 0,
        }
    }

    /// Serialize to bytes.
    ///
    /// Format: term (8 bytes) + voted_for flag (1 byte) +
    ///         voted_for value (8 bytes) + commit_index (8 bytes)
    /// Total: 25 bytes, fixed size.
    pub fn serialize(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(25);
        bytes.extend_from_slice(&self.current_term.to_le_bytes());

        match self.voted_for {
            Some(id) => {
                bytes.push(1); // flag: voted
                bytes.extend_from_slice(&id.to_le_bytes());
            }
            None => {
                bytes.push(0); // flag: not voted
                bytes.extend_from_slice(&0u64.to_le_bytes());
            }
        }

        bytes.extend_from_slice(&self.commit_index.to_le_bytes());
        bytes
    }

    /// Deserialize from bytes.
    pub fn deserialize(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < 25 {
            return Err(format!(
                "state file too short: {} bytes, expected 25", bytes.len()
            ));
        }

        let current_term = u64::from_le_bytes(
            bytes[0..8].try_into().unwrap()
        );

        let voted_for = match bytes[8] {
            1 => Some(u64::from_le_bytes(
                bytes[9..17].try_into().unwrap()
            )),
            0 => None,
            flag => return Err(format!("invalid voted_for flag: {}", flag)),
        };

        let commit_index = u64::from_le_bytes(
            bytes[17..25].try_into().unwrap()
        );

        Ok(RaftState {
            current_term,
            voted_for,
            commit_index,
        })
    }
}
```

### Step 3: Implement atomic persistence

```rust
// src/raft/state.rs (continued)

/// Manages persistent Raft state on disk.
///
/// Owns the file path and provides atomic read/write operations.
/// Uses write-then-rename for atomicity.
pub struct StatePersister {
    path: PathBuf,
}

impl StatePersister {
    pub fn new(path: impl AsRef<Path>) -> Self {
        StatePersister {
            path: path.as_ref().to_path_buf(),
        }
    }

    /// Atomically write state to disk.
    ///
    /// 1. Write to a temporary file (path + ".tmp")
    /// 2. fsync the temporary file
    /// 3. Rename the temporary file to the real path (atomic on POSIX)
    /// 4. fsync the directory (ensures the rename is durable)
    pub fn save(&self, state: &RaftState) -> std::io::Result<()> {
        let tmp_path = self.path.with_extension("tmp");
        let bytes = state.serialize();

        // Step 1: Write to temporary file
        {
            let mut file = File::create(&tmp_path)?;
            file.write_all(&bytes)?;
            // Step 2: fsync the file
            file.sync_all()?;
        } // file is closed here (Drop)

        // Step 3: Atomic rename
        fs::rename(&tmp_path, &self.path)?;

        // Step 4: fsync the directory
        // This ensures the directory entry (the rename) is durable.
        if let Some(parent) = self.path.parent() {
            let dir = File::open(parent)?;
            dir.sync_all()?;
        }

        Ok(())
    }

    /// Load state from disk. Returns default state if the file
    /// does not exist (first boot).
    pub fn load(&self) -> Result<RaftState, String> {
        match fs::read(&self.path) {
            Ok(bytes) => RaftState::deserialize(&bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Ok(RaftState::new())
            }
            Err(e) => Err(format!("failed to read state file: {}", e)),
        }
    }
}
```

### Step 4: Why fsync the directory?

This is a subtlety that most tutorials skip. When you rename a file, the old name is removed from the directory and the new name is added. These are changes to the **directory**, not the file. If you only `fsync` the file, the rename might not be durable — a crash could leave the directory pointing to the old file (or no file at all). `fsync`-ing the parent directory ensures the rename is persisted.

On Linux, `ext4` with `data=ordered` mode (the default) guarantees that file data is written before metadata, which makes the directory fsync less critical. But other filesystems (`btrfs`, `xfs`, `zfs`) have different guarantees. Always fsync the directory if you need portable durability.

### Step 5: Test atomic persistence

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("raft.state");
        let persister = StatePersister::new(&path);

        let state = RaftState {
            current_term: 42,
            voted_for: Some(3),
            commit_index: 17,
        };

        persister.save(&state).unwrap();
        let loaded = persister.load().unwrap();

        assert_eq!(loaded, state);
    }

    #[test]
    fn test_state_default_on_first_boot() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nonexistent.state");
        let persister = StatePersister::new(&path);

        let state = persister.load().unwrap();
        assert_eq!(state, RaftState::new());
    }

    #[test]
    fn test_state_update_is_atomic() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("raft.state");
        let persister = StatePersister::new(&path);

        // Write initial state
        let state1 = RaftState {
            current_term: 1,
            voted_for: None,
            commit_index: 0,
        };
        persister.save(&state1).unwrap();

        // Update to new state
        let state2 = RaftState {
            current_term: 5,
            voted_for: Some(2),
            commit_index: 10,
        };
        persister.save(&state2).unwrap();

        // Should see the new state, not the old one
        let loaded = persister.load().unwrap();
        assert_eq!(loaded, state2);

        // Temporary file should not exist
        assert!(!path.with_extension("tmp").exists());
    }
}
```

> **Coming from JS/Python/Go?**
>
> Atomic file writes are a cross-language concern, but the patterns differ:
>
> - **Node.js:** `fs.writeFileSync()` does not fsync by default. You need `fs.fdatasyncSync()` or `fs.fsyncSync()`. For atomic writes, use `write-file-atomic` package.
> - **Python:** `os.fsync(fd)` for durability, `os.rename()` for atomicity. The `tempfile` module helps with temporary files.
> - **Go:** `f.Sync()` for fsync, `os.Rename()` for atomic replace. Same pattern as Rust, but you must remember `defer f.Close()` — forgetting it is a silent bug.
> - **Rust:** `file.sync_all()` for fsync, `fs::rename()` for atomic replace. The file is closed automatically when the `File` goes out of scope. No `defer` needed, no resource leak possible.

---

## Exercise 3: Recovery — Rebuilding State from Disk

**Goal:** Build a `RecoveryManager` that reads the WAL and metadata file on startup and reconstructs the node's in-memory state.

### Step 1: What recovery looks like

When a Raft node starts, it does not know if this is a first boot (empty state) or a restart after a crash. The recovery procedure is the same either way:

```
Recovery procedure:
1. Load RaftState from metadata file
   → If file missing: first boot, use defaults (term=0, voted_for=None)
   → If file exists: read current_term, voted_for, commit_index

2. Read WAL entries
   → If WAL missing: first boot, empty log
   → If WAL exists: read all valid entries (stop at first corrupt record)

3. Reconstruct in-memory state
   → Populate the in-memory log from WAL entries
   → Set term, voted_for from metadata
   → Start as Follower (never start as Leader — must win a new election)
```

### Step 2: Build the recovery manager

```rust
// src/raft/recovery.rs

use crate::raft::wal::{LogEntry, WalReader, WalWriter, WalError};
use crate::raft::state::{RaftState, StatePersister};
use std::path::{Path, PathBuf};

/// The result of recovery: everything needed to initialize a RaftNode.
pub struct RecoveredState {
    pub state: RaftState,
    pub log: Vec<LogEntry>,
    pub wal_writer: WalWriter,
}

/// Manages crash recovery for a Raft node.
pub struct RecoveryManager {
    data_dir: PathBuf,
}

impl RecoveryManager {
    pub fn new(data_dir: impl AsRef<Path>) -> Self {
        RecoveryManager {
            data_dir: data_dir.as_ref().to_path_buf(),
        }
    }

    fn wal_path(&self) -> PathBuf {
        self.data_dir.join("raft.wal")
    }

    fn state_path(&self) -> PathBuf {
        self.data_dir.join("raft.state")
    }

    /// Recover state from disk. Called once at startup.
    ///
    /// Returns the recovered state and an open WAL writer
    /// (positioned at the end for appending new entries).
    pub fn recover(&self) -> Result<RecoveredState, String> {
        // Ensure data directory exists
        std::fs::create_dir_all(&self.data_dir)
            .map_err(|e| format!("failed to create data dir: {}", e))?;

        // Step 1: Load metadata
        let persister = StatePersister::new(self.state_path());
        let state = persister.load()?;

        // Step 2: Read WAL
        let log = if self.wal_path().exists() {
            let mut reader = WalReader::open(self.wal_path())
                .map_err(|e| format!("failed to open WAL: {}", e))?;
            reader.read_all()
                .map_err(|e| format!("failed to read WAL: {}", e))?
        } else {
            Vec::new()
        };

        // Step 3: Open WAL writer for appending
        let wal_writer = WalWriter::open(self.wal_path())
            .map_err(|e| format!("failed to open WAL writer: {}", e))?;

        println!(
            "Recovery complete: term={}, voted_for={:?}, \
             commit_index={}, log_entries={}",
            state.current_term,
            state.voted_for,
            state.commit_index,
            log.len()
        );

        Ok(RecoveredState {
            state,
            log,
            wal_writer,
        })
    }
}
```

### Step 3: Integrate recovery into the Raft node

```rust,ignore
// src/raft/node.rs (modified)

use crate::raft::recovery::RecoveryManager;
use crate::raft::state::StatePersister;
use crate::raft::wal::WalWriter;

pub struct RaftNode {
    // Persistent state (also on disk)
    current_term: u64,
    voted_for: Option<u64>,
    log: Vec<LogEntry>,

    // Volatile state (rebuilt on recovery)
    commit_index: u64,
    last_applied: u64,
    role: Role,

    // Persistence handles (owned by the node)
    wal: WalWriter,
    state_persister: StatePersister,

    // Node identity
    id: u64,
}

impl RaftNode {
    /// Create a new node, recovering state from disk if available.
    pub fn new(id: u64, data_dir: &str) -> Result<Self, String> {
        let recovery = RecoveryManager::new(data_dir);
        let recovered = recovery.recover()?;

        Ok(RaftNode {
            current_term: recovered.state.current_term,
            voted_for: recovered.state.voted_for,
            log: recovered.log,
            commit_index: recovered.state.commit_index,
            last_applied: 0,  // will re-apply from commit_index
            role: Role::Follower,  // always start as follower
            wal: recovered.wal_writer,
            state_persister: StatePersister::new(
                format!("{}/raft.state", data_dir)
            ),
            id,
        })
    }

    /// Append an entry and persist it before acknowledging.
    pub fn append_entry(&mut self, entry: LogEntry) -> Result<(), String> {
        // Write to WAL first (durability guarantee)
        self.wal.append_sync(&entry)
            .map_err(|e| format!("WAL write failed: {}", e))?;

        // Only then add to in-memory log
        self.log.push(entry);

        Ok(())
    }

    /// Update term and persist before responding to any RPC.
    pub fn update_term(&mut self, new_term: u64) -> Result<(), String> {
        self.current_term = new_term;
        self.voted_for = None;  // new term clears previous vote
        self.persist_state()
    }

    /// Vote for a candidate and persist before responding.
    pub fn vote_for(&mut self, candidate_id: u64) -> Result<(), String> {
        self.voted_for = Some(candidate_id);
        self.persist_state()
    }

    fn persist_state(&self) -> Result<(), String> {
        let state = RaftState {
            current_term: self.current_term,
            voted_for: self.voted_for,
            commit_index: self.commit_index,
        };
        self.state_persister.save(&state)
            .map_err(|e| format!("state persist failed: {}", e))?;
        Ok(())
    }
}
```

Notice the pattern: **write to disk first, then update in-memory state**. If the disk write fails, we return an error and the in-memory state is unchanged. If the process crashes after the disk write but before updating memory, recovery will read the persisted state and reconstruct the correct in-memory state. This is the fundamental invariant of write-ahead logging: the log on disk is always at least as up-to-date as the state in memory.

### Step 4: Test crash recovery

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recovery_from_empty_state() {
        let dir = tempfile::tempdir().unwrap();
        let recovery = RecoveryManager::new(dir.path());
        let recovered = recovery.recover().unwrap();

        assert_eq!(recovered.state.current_term, 0);
        assert_eq!(recovered.state.voted_for, None);
        assert_eq!(recovered.state.commit_index, 0);
        assert_eq!(recovered.log.len(), 0);
    }

    #[test]
    fn test_recovery_after_writes() {
        let dir = tempfile::tempdir().unwrap();

        // Simulate a node that wrote some state and entries
        {
            let persister = StatePersister::new(dir.path().join("raft.state"));
            persister.save(&RaftState {
                current_term: 3,
                voted_for: Some(1),
                commit_index: 5,
            }).unwrap();

            let mut wal = WalWriter::open(dir.path().join("raft.wal")).unwrap();
            for i in 1..=7 {
                let entry = LogEntry {
                    term: if i <= 3 { 1 } else { 2 },
                    index: i,
                    command: format!("cmd-{}", i).into_bytes(),
                };
                wal.append(&entry).unwrap();
            }
            wal.sync().unwrap();
        }

        // Recover
        let recovery = RecoveryManager::new(dir.path());
        let recovered = recovery.recover().unwrap();

        assert_eq!(recovered.state.current_term, 3);
        assert_eq!(recovered.state.voted_for, Some(1));
        assert_eq!(recovered.state.commit_index, 5);
        assert_eq!(recovered.log.len(), 7);
        assert_eq!(recovered.log[0].index, 1);
        assert_eq!(recovered.log[6].index, 7);
    }

    #[test]
    fn test_recovery_with_corrupt_wal_tail() {
        let dir = tempfile::tempdir().unwrap();

        // Write 5 entries, then corrupt the end
        {
            let mut wal = WalWriter::open(dir.path().join("raft.wal")).unwrap();
            for i in 1..=5 {
                wal.append(&LogEntry {
                    term: 1,
                    index: i,
                    command: format!("cmd-{}", i).into_bytes(),
                }).unwrap();
            }
            wal.sync().unwrap();
        }

        // Append garbage (simulates crash during write)
        {
            use std::io::Write;
            let mut file = std::fs::OpenOptions::new()
                .append(true)
                .open(dir.path().join("raft.wal"))
                .unwrap();
            file.write_all(&[0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x00]).unwrap();
        }

        // Recovery should return the 5 good entries
        let recovery = RecoveryManager::new(dir.path());
        let recovered = recovery.recover().unwrap();
        assert_eq!(recovered.log.len(), 5);
    }
}
```

---

## Exercise 4: Snapshots and Log Compaction

**Goal:** Build a snapshot mechanism that captures the state machine's state at a given log index, allowing old log entries to be discarded. Implement `InstallSnapshot` RPC for transferring snapshots to lagging followers.

### Step 1: Why snapshots exist

As the Raft cluster runs, the log grows without bound. After millions of commands, the WAL might be gigabytes. Recovery would require replaying millions of entries. And if a follower is far behind (perhaps it was offline for hours), the leader would need to send millions of entries to catch it up.

Snapshots solve both problems. A snapshot captures the state machine's state at a specific log index. Once a snapshot exists, all log entries up to that index can be discarded — the snapshot contains their cumulative effect.

```
Before snapshot:
Log: [1][2][3][4][5][6][7][8][9][10][11][12]...
     ↑ these entries represent the state at index 12

After snapshot at index 10:
Snapshot: { full state machine state at index 10 }
Log: [11][12]...
     ↑ only entries after the snapshot are kept
```

### Step 2: Define the snapshot structure

```rust
// src/raft/snapshot.rs

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// A point-in-time snapshot of the state machine.
#[derive(Debug, Clone)]
pub struct Snapshot {
    /// The index of the last log entry included in this snapshot.
    pub last_included_index: u64,
    /// The term of the last log entry included in this snapshot.
    pub last_included_term: u64,
    /// The serialized state machine state.
    pub data: Vec<u8>,
}

impl Snapshot {
    /// Serialize the snapshot to bytes for storage or transfer.
    ///
    /// Format:
    /// - 8 bytes: last_included_index (le)
    /// - 8 bytes: last_included_term (le)
    /// - 4 bytes: data length (le)
    /// - N bytes: data
    pub fn serialize(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(20 + self.data.len());
        bytes.extend_from_slice(&self.last_included_index.to_le_bytes());
        bytes.extend_from_slice(&self.last_included_term.to_le_bytes());
        bytes.extend_from_slice(&(self.data.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&self.data);
        bytes
    }

    /// Deserialize a snapshot from bytes.
    pub fn deserialize(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < 20 {
            return Err("snapshot too short".to_string());
        }
        let last_included_index = u64::from_le_bytes(
            bytes[0..8].try_into().unwrap()
        );
        let last_included_term = u64::from_le_bytes(
            bytes[8..16].try_into().unwrap()
        );
        let data_len = u32::from_le_bytes(
            bytes[16..20].try_into().unwrap()
        ) as usize;

        if bytes.len() < 20 + data_len {
            return Err(format!(
                "snapshot data truncated: expected {} bytes, got {}",
                data_len, bytes.len() - 20
            ));
        }

        let data = bytes[20..20 + data_len].to_vec();
        Ok(Snapshot {
            last_included_index,
            last_included_term,
            data,
        })
    }
}
```

### Step 3: Snapshot storage

```rust
// src/raft/snapshot.rs (continued)

/// Manages snapshot files on disk.
///
/// Snapshots are stored as individual files named by their last
/// included index. Only the most recent snapshot is kept — older
/// ones are deleted after a new snapshot is successfully written.
pub struct SnapshotStore {
    dir: PathBuf,
}

impl SnapshotStore {
    pub fn new(dir: impl AsRef<Path>) -> Result<Self, std::io::Error> {
        let dir = dir.as_ref().to_path_buf();
        fs::create_dir_all(&dir)?;
        Ok(SnapshotStore { dir })
    }

    /// Save a snapshot to disk. Deletes older snapshots.
    pub fn save(&self, snapshot: &Snapshot) -> Result<(), std::io::Error> {
        let path = self.snapshot_path(snapshot.last_included_index);
        let tmp_path = path.with_extension("tmp");

        // Write to temp file, then atomic rename
        {
            let mut file = File::create(&tmp_path)?;
            file.write_all(&snapshot.serialize())?;
            file.sync_all()?;
        }

        fs::rename(&tmp_path, &path)?;

        // Delete older snapshots
        self.delete_older_than(snapshot.last_included_index)?;

        Ok(())
    }

    /// Load the most recent snapshot, if one exists.
    pub fn load_latest(&self) -> Result<Option<Snapshot>, String> {
        let mut snapshots: Vec<u64> = Vec::new();

        let entries = fs::read_dir(&self.dir)
            .map_err(|e| format!("failed to read snapshot dir: {}", e))?;

        for entry in entries {
            let entry = entry.map_err(|e| format!("dir entry error: {}", e))?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if let Some(index_str) = name.strip_prefix("snapshot-")
                .and_then(|s| s.strip_suffix(".bin"))
            {
                if let Ok(index) = index_str.parse::<u64>() {
                    snapshots.push(index);
                }
            }
        }

        if snapshots.is_empty() {
            return Ok(None);
        }

        snapshots.sort();
        let latest = *snapshots.last().unwrap();
        let path = self.snapshot_path(latest);
        let bytes = fs::read(&path)
            .map_err(|e| format!("failed to read snapshot: {}", e))?;
        let snapshot = Snapshot::deserialize(&bytes)?;
        Ok(Some(snapshot))
    }

    fn snapshot_path(&self, index: u64) -> PathBuf {
        self.dir.join(format!("snapshot-{:020}.bin", index))
    }

    fn delete_older_than(&self, index: u64) -> Result<(), std::io::Error> {
        let entries = fs::read_dir(&self.dir)?;
        for entry in entries {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if let Some(index_str) = name.strip_prefix("snapshot-")
                .and_then(|s| s.strip_suffix(".bin"))
            {
                if let Ok(snap_index) = index_str.parse::<u64>() {
                    if snap_index < index {
                        fs::remove_file(entry.path())?;
                    }
                }
            }
        }
        Ok(())
    }
}
```

### Step 4: Log compaction after snapshot

When a snapshot is created, the WAL entries up to the snapshot's `last_included_index` can be discarded. This is called **log compaction**. The procedure:

1. Create the snapshot (serialize state machine)
2. Save the snapshot to disk
3. Create a new WAL containing only entries after the snapshot index
4. Replace the old WAL with the new one (atomic rename)

```rust,ignore
// src/raft/node.rs (snapshot integration)

impl RaftNode {
    /// Create a snapshot at the current commit_index.
    pub fn create_snapshot(
        &mut self,
        state_machine_data: Vec<u8>,
    ) -> Result<(), String> {
        if self.commit_index == 0 {
            return Err("nothing to snapshot".to_string());
        }

        // Find the term of the entry at commit_index
        let last_term = self.log.iter()
            .find(|e| e.index == self.commit_index)
            .map(|e| e.term)
            .ok_or("commit_index entry not found in log")?;

        let snapshot = Snapshot {
            last_included_index: self.commit_index,
            last_included_term: last_term,
            data: state_machine_data,
        };

        // Save snapshot to disk
        self.snapshot_store.save(&snapshot)
            .map_err(|e| format!("snapshot save failed: {}", e))?;

        // Compact the log: remove entries up to and including
        // the snapshot index
        self.log.retain(|e| e.index > self.commit_index);

        // Rewrite the WAL with only the remaining entries
        self.rewrite_wal()?;

        Ok(())
    }

    /// Rewrite the WAL with only the current in-memory log entries.
    /// Used after log compaction to reclaim disk space.
    fn rewrite_wal(&mut self) -> Result<(), String> {
        let wal_path = format!("{}/raft.wal", self.data_dir);
        let tmp_path = format!("{}/raft.wal.compact", self.data_dir);

        // Write remaining entries to a new WAL
        {
            let mut new_wal = WalWriter::open(&tmp_path)
                .map_err(|e| format!("failed to create compact WAL: {}", e))?;
            for entry in &self.log {
                new_wal.append(entry)
                    .map_err(|e| format!("compact WAL write failed: {}", e))?;
            }
            new_wal.sync()
                .map_err(|e| format!("compact WAL sync failed: {}", e))?;
        }

        // Atomic replace
        std::fs::rename(&tmp_path, &wal_path)
            .map_err(|e| format!("WAL rename failed: {}", e))?;

        // Reopen the WAL writer
        self.wal = WalWriter::open(&wal_path)
            .map_err(|e| format!("WAL reopen failed: {}", e))?;

        Ok(())
    }
}
```

### Step 5: InstallSnapshot RPC

When a follower is so far behind that the leader has already compacted the entries it needs, the leader sends its snapshot instead:

```rust,ignore
// src/raft/rpc.rs (new message types)

/// InstallSnapshot RPC — sent by the leader to a follower that
/// is too far behind to catch up via AppendEntries.
#[derive(Debug, Clone)]
pub struct InstallSnapshotRequest {
    pub term: u64,
    pub leader_id: u64,
    pub last_included_index: u64,
    pub last_included_term: u64,
    /// Byte offset where chunk is positioned in the snapshot file.
    pub offset: u64,
    /// Raw bytes of the snapshot chunk.
    pub data: Vec<u8>,
    /// True if this is the last chunk.
    pub done: bool,
}

#[derive(Debug, Clone)]
pub struct InstallSnapshotResponse {
    pub term: u64,
}
```

The Raft paper specifies that large snapshots can be sent in chunks. The `offset` and `done` fields support this — the leader sends the snapshot in pieces, and the follower reassembles them. For simplicity, our implementation sends the entire snapshot in a single chunk:

```rust,ignore
impl RaftNode {
    /// Handle an InstallSnapshot RPC from the leader.
    pub fn handle_install_snapshot(
        &mut self,
        req: InstallSnapshotRequest,
    ) -> Result<InstallSnapshotResponse, String> {
        // Step 1: Reply false if term < currentTerm
        if req.term < self.current_term {
            return Ok(InstallSnapshotResponse {
                term: self.current_term,
            });
        }

        // Step 2: Update term if needed
        if req.term > self.current_term {
            self.update_term(req.term)?;
            self.role = Role::Follower;
        }

        // Step 3: Save the snapshot
        let snapshot = Snapshot {
            last_included_index: req.last_included_index,
            last_included_term: req.last_included_term,
            data: req.data,
        };
        self.snapshot_store.save(&snapshot)
            .map_err(|e| format!("snapshot save failed: {}", e))?;

        // Step 4: Discard log entries covered by the snapshot
        self.log.retain(|e| e.index > req.last_included_index);

        // Step 5: Reset state machine using snapshot data
        // (the caller must apply the snapshot to the state machine)

        // Step 6: Update commit_index
        if req.last_included_index > self.commit_index {
            self.commit_index = req.last_included_index;
            self.last_applied = req.last_included_index;
            self.persist_state()?;
        }

        // Step 7: Rewrite WAL
        self.rewrite_wal()?;

        Ok(InstallSnapshotResponse {
            term: self.current_term,
        })
    }
}
```

### Step 6: When to trigger a snapshot

A common policy is to snapshot when the WAL exceeds a size threshold:

```rust,ignore
impl RaftNode {
    /// Check if the log is large enough to warrant a snapshot.
    pub fn should_snapshot(&self) -> bool {
        // Snapshot when the log has more than 10,000 entries
        // beyond the last snapshot.
        self.log.len() > 10_000
    }

    /// Called periodically or after committing entries.
    pub fn maybe_snapshot(
        &mut self,
        state_machine: &dyn StateMachine,
    ) -> Result<(), String> {
        if self.should_snapshot() {
            let data = state_machine.serialize()?;
            self.create_snapshot(data)?;
        }
        Ok(())
    }
}

/// Trait for state machines that support snapshotting.
pub trait StateMachine {
    /// Serialize the current state to bytes.
    fn serialize(&self) -> Result<Vec<u8>, String>;
    /// Restore state from a serialized snapshot.
    fn restore(&mut self, data: &[u8]) -> Result<(), String>;
}
```

The `StateMachine` trait uses `&self` for `serialize` (snapshot is a read operation) and `&mut self` for `restore` (restoring state is a mutation). This matches the ownership semantics perfectly — you can take a snapshot while the state machine is being read, but restoring requires exclusive access.

---

## fsync: The Durability Contract

The gap between "data written" and "data durable" is the most commonly misunderstood concept in systems programming. Understanding it is essential for building reliable storage.

### The write path: where bytes go

When you call `write()`, the bytes travel through multiple layers:

```
Your process     →  OS page cache     →  Disk controller cache  →  Physical disk
(user space)        (kernel memory)       (hardware buffer)          (platters/flash)

write()          →  data in page cache  (NOT on disk yet)
flush()          →  data sent to disk controller (might be in controller cache)
fsync()/sync_all →  data confirmed on physical media
```

Each layer adds a buffer for performance. Each buffer is a crash risk:

- **Process crash** (segfault, `kill -9`): Page cache survives. Data that reached the kernel is safe. Data in `BufWriter` that was not flushed is lost.
- **OS crash** (kernel panic): Disk controller cache might survive (if it has battery-backed write cache). Page cache is lost.
- **Power loss**: Everything volatile is lost. Only data on the physical disk survives. Battery-backed controller caches might survive (enterprise drives) or might not (consumer drives).

### What fsync guarantees

`fsync()` (Rust's `file.sync_all()`) tells the OS: "Make sure all data for this file is on the physical disk before returning." The OS flushes its page cache for this file and asks the disk controller to flush its cache too.

After `sync_all()` returns, the data is durable — it will survive power loss (assuming the disk is not lying about having actually written the data, which is a real issue with some consumer SSDs).

### The performance cost

`fsync` is expensive. It forces the disk to do actual I/O:

| Operation | Latency (SSD) | Latency (HDD) |
|-----------|---------------|----------------|
| `write()` to page cache | ~1 microsecond | ~1 microsecond |
| `fsync()` to SSD | ~100 microseconds | - |
| `fsync()` to HDD | - | ~5-10 milliseconds |

For an HDD, `fsync` limits you to about 100-200 durable writes per second. This is why databases batch multiple entries per `fsync` — write 100 entries, then sync once, amortizing the cost.

### Batching syncs

Our WAL provides both `append()` (buffered, no sync) and `append_sync()` (append + sync). For entries received via `AppendEntries` RPC, we can batch:

```rust,ignore
impl RaftNode {
    /// Handle a batch of entries from the leader.
    fn handle_append_entries(
        &mut self,
        entries: Vec<LogEntry>,
    ) -> Result<(), String> {
        // Append all entries without syncing each one
        for entry in &entries {
            self.wal.append(entry)
                .map_err(|e| format!("WAL append failed: {}", e))?;
        }

        // Single sync for the entire batch
        self.wal.sync()
            .map_err(|e| format!("WAL sync failed: {}", e))?;

        // Now add to in-memory log
        self.log.extend(entries);
        Ok(())
    }
}
```

This is a common pattern in database WALs. SQLite calls it "WAL mode" — multiple transactions can be batched into a single sync. PostgreSQL calls it "synchronous commit" vs "asynchronous commit" — the tradeoff between durability and throughput.

> **Coming from JS/Python/Go?**
>
> The `fsync` issue is language-independent, but awareness varies:
>
> - **Node.js:** `fs.writeFileSync()` does NOT fsync. Your "saved" file might be lost on power failure. Use `fs.fdatasyncSync()` explicitly.
> - **Python:** `f.write()` goes to the OS buffer. Call `os.fsync(f.fileno())` for durability. Most Python programs never do this.
> - **Go:** `f.Write()` goes to the OS buffer. Call `f.Sync()` for durability. The Go documentation is clear about this.
> - **Rust:** `file.write_all()` goes through `BufWriter` (if used) to the OS buffer. Call `file.sync_all()` for durability. `sync_data()` is a lighter alternative that syncs data but not metadata (like file modification time).
>
> The universal rule: if you call `write()` and the power goes out one millisecond later, your data is probably gone — unless you called `fsync()` first.

---

## Recovery Correctness: What Can Go Wrong

Let us walk through several failure scenarios to verify that our recovery logic is correct.

### Scenario 1: Crash after WAL write, before metadata update

```
1. Node receives AppendEntries with entry at index=10, term=3
2. WAL append succeeds (entry 10 is on disk)
3. — CRASH —
4. Metadata still says commit_index=8

After recovery:
- WAL has entries 1-10
- Metadata says commit_index=8
- Node starts as follower with entries 1-10, commit_index=8
- Leader will resend entries and update commit_index
→ CORRECT: uncommitted entries in the log are harmless. The leader
  will either confirm them (if they match) or overwrite them.
```

### Scenario 2: Crash during metadata write (partial write)

```
1. Node votes for candidate 3 in term 5
2. Metadata write starts (writing to temp file)
3. — CRASH — (temp file partially written)

After recovery:
- Temp file exists but is incomplete
- Real metadata file still has term=4, voted_for=None
- Node starts with term=4, voted_for=None
→ CORRECT: the old state is consistent. The node might vote again
  in term 5, but that is safe — it did not complete the vote before.
```

This is why we use write-then-rename. If the crash happens before the rename, the old file is intact. If the crash happens after the rename, the new file is intact. The rename itself is atomic on POSIX — there is no in-between state.

### Scenario 3: Crash during snapshot creation

```
1. Node creates snapshot at index=100
2. Snapshot file written and synced
3. — CRASH — (before WAL compaction)

After recovery:
- Snapshot exists at index 100
- WAL still has all entries (including 1-100)
- Recovery loads snapshot AND WAL entries
- Entries 1-100 from WAL are redundant but harmless
→ CORRECT: the extra entries do not cause inconsistency.
  They will be compacted on the next snapshot.
```

### Scenario 4: Crash during WAL compaction

```
1. Node creates snapshot at index=100 (saved)
2. New WAL written with entries 101+ (saved to .compact file)
3. — CRASH — (before rename of .compact to .wal)

After recovery:
- Old WAL still in place (entries 1-150)
- Snapshot exists at index 100
- .compact file exists but is not the active WAL
- Recovery uses the old WAL, ignoring .compact
→ CORRECT: same as scenario 3. Extra entries are harmless.
```

The general principle: every operation is designed so that a crash at any point leaves the system in a recoverable state. This is not accidental — it requires careful ordering of writes and renames.

---

## The Complete Durability Layer

Here is how all the pieces fit together:

```
┌────────────────────────────────────────────────────────────┐
│                        RaftNode                            │
│                                                            │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐ │
│  │  In-memory   │  │  In-memory   │  │  In-memory       │ │
│  │  log[]       │  │  term/vote   │  │  commit_index    │ │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────────┘ │
│         │                 │                 │              │
│  ┌──────▼───────┐  ┌──────▼───────┐  ┌──────▼───────────┐ │
│  │  WalWriter   │  │ StatePersist │  │  SnapshotStore   │ │
│  │  (raft.wal)  │  │ (raft.state) │  │  (snapshots/)    │ │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────────┘ │
│         │                 │                 │              │
└─────────┼─────────────────┼─────────────────┼──────────────┘
          │                 │                 │
          ▼                 ▼                 ▼
    ┌───────────────────────────────────────────────┐
    │                 File System                    │
    │  raft.wal    raft.state    snapshots/*.bin     │
    └───────────────────────────────────────────────┘
```

The ownership is clear:
- `RaftNode` owns `WalWriter`, `StatePersister`, and `SnapshotStore`
- Each of these structs owns its file handle or path
- When `RaftNode` is dropped, everything is cleaned up automatically
- `&mut self` on write methods ensures exclusive access at compile time

---

## Rust Gym

Time for reps. These drills focus on ownership and persistence — the spotlight concepts for this chapter.

### Drill 1: WAL with CRC Checksums (Medium)

Implement a simplified WAL that stores string messages with CRC32 checksums. The WAL should detect corruption when reading.

```rust
use std::io::{Read, Write};

struct SimpleWal {
    data: Vec<u8>,  // in-memory buffer simulating a file
}

impl SimpleWal {
    fn new() -> Self {
        SimpleWal { data: Vec::new() }
    }

    /// Append a message with a CRC32 checksum.
    fn append(&mut self, message: &str) {
        // Format: [4 bytes CRC][4 bytes length][N bytes message]
        todo!()
    }

    /// Read all messages, returning only those with valid checksums.
    fn read_all(&self) -> Vec<String> {
        todo!()
    }

    /// Corrupt a byte at the given offset (for testing).
    fn corrupt(&mut self, offset: usize) {
        if offset < self.data.len() {
            self.data[offset] ^= 0xFF;
        }
    }
}

fn main() {
    let mut wal = SimpleWal::new();
    wal.append("hello");
    wal.append("world");
    wal.append("test");

    let messages = wal.read_all();
    assert_eq!(messages, vec!["hello", "world", "test"]);

    // Corrupt the second message's data
    wal.corrupt(20); // somewhere in "world"
    let messages = wal.read_all();
    assert_eq!(messages.len(), 1); // only "hello" survives
    assert_eq!(messages[0], "hello");

    println!("All checks passed!");
}
```

<details>
<summary>Solution</summary>

```rust
fn crc32_simple(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFFFFFF;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            if crc & 1 == 1 {
                crc = (crc >> 1) ^ 0xEDB88320;
            } else {
                crc >>= 1;
            }
        }
    }
    crc ^ 0xFFFFFFFF
}

struct SimpleWal {
    data: Vec<u8>,
}

impl SimpleWal {
    fn new() -> Self {
        SimpleWal { data: Vec::new() }
    }

    fn append(&mut self, message: &str) {
        let msg_bytes = message.as_bytes();
        let length = msg_bytes.len() as u32;

        // CRC covers length + message
        let mut crc_input = Vec::new();
        crc_input.extend_from_slice(&length.to_le_bytes());
        crc_input.extend_from_slice(msg_bytes);
        let checksum = crc32_simple(&crc_input);

        self.data.extend_from_slice(&checksum.to_le_bytes());
        self.data.extend_from_slice(&length.to_le_bytes());
        self.data.extend_from_slice(msg_bytes);
    }

    fn read_all(&self) -> Vec<String> {
        let mut messages = Vec::new();
        let mut pos = 0;

        while pos + 8 <= self.data.len() {
            let stored_crc = u32::from_le_bytes(
                self.data[pos..pos + 4].try_into().unwrap()
            );
            let length = u32::from_le_bytes(
                self.data[pos + 4..pos + 8].try_into().unwrap()
            ) as usize;

            if pos + 8 + length > self.data.len() {
                break;
            }

            let crc_data = &self.data[pos + 4..pos + 8 + length];
            let computed_crc = crc32_simple(crc_data);

            if stored_crc != computed_crc {
                break; // Stop at first corruption
            }

            let msg = String::from_utf8_lossy(
                &self.data[pos + 8..pos + 8 + length]
            ).to_string();
            messages.push(msg);

            pos += 8 + length;
        }

        messages
    }

    fn corrupt(&mut self, offset: usize) {
        if offset < self.data.len() {
            self.data[offset] ^= 0xFF;
        }
    }
}

fn main() {
    let mut wal = SimpleWal::new();
    wal.append("hello");
    wal.append("world");
    wal.append("test");

    let messages = wal.read_all();
    assert_eq!(messages, vec!["hello", "world", "test"]);

    wal.corrupt(20);
    let messages = wal.read_all();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0], "hello");

    println!("All checks passed!");
}
```

The key insight: we stop reading at the first corrupt record. This is a deliberate design choice — records after a corrupt one might be valid, but we cannot be sure they are in the correct position (the corrupt record's length field might be wrong, causing us to misalign). Stopping at the first error is the safe choice.

</details>

### Drill 2: Periodic Snapshots (Medium)

Build a `KeyValueStore` that takes periodic snapshots. After restoring from a snapshot, applying remaining log entries should produce the same state as before the snapshot.

```rust
use std::collections::HashMap;

struct KeyValueStore {
    data: HashMap<String, String>,
    log: Vec<(String, String)>,  // (key, value) pairs
    snapshot: Option<(usize, HashMap<String, String>)>,  // (log_index, state)
}

impl KeyValueStore {
    fn new() -> Self {
        todo!()
    }

    fn set(&mut self, key: String, value: String) {
        todo!()
    }

    fn get(&self, key: &str) -> Option<&String> {
        todo!()
    }

    /// Take a snapshot at the current log position.
    fn snapshot(&mut self) {
        todo!()
    }

    /// Compact: remove log entries covered by the snapshot.
    fn compact(&mut self) {
        todo!()
    }

    /// Restore from snapshot + remaining log entries.
    fn restore(&mut self) {
        todo!()
    }

    fn log_len(&self) -> usize {
        self.log.len()
    }
}

fn main() {
    let mut store = KeyValueStore::new();

    // Apply some operations
    store.set("a".into(), "1".into());
    store.set("b".into(), "2".into());
    store.set("c".into(), "3".into());
    assert_eq!(store.log_len(), 3);

    // Take snapshot
    store.snapshot();

    // Apply more operations
    store.set("d".into(), "4".into());
    store.set("a".into(), "updated".into());

    // Compact (removes entries covered by snapshot)
    store.compact();
    assert_eq!(store.log_len(), 2); // only entries after snapshot

    // Restore from snapshot + remaining log
    store.restore();
    assert_eq!(store.get("a"), Some(&"updated".to_string()));
    assert_eq!(store.get("b"), Some(&"2".to_string()));
    assert_eq!(store.get("d"), Some(&"4".to_string()));

    println!("All checks passed!");
}
```

<details>
<summary>Solution</summary>

```rust
use std::collections::HashMap;

struct KeyValueStore {
    data: HashMap<String, String>,
    log: Vec<(String, String)>,
    snapshot: Option<(usize, HashMap<String, String>)>,
}

impl KeyValueStore {
    fn new() -> Self {
        KeyValueStore {
            data: HashMap::new(),
            log: Vec::new(),
            snapshot: None,
        }
    }

    fn set(&mut self, key: String, value: String) {
        self.data.insert(key.clone(), value.clone());
        self.log.push((key, value));
    }

    fn get(&self, key: &str) -> Option<&String> {
        self.data.get(key)
    }

    fn snapshot(&mut self) {
        self.snapshot = Some((self.log.len(), self.data.clone()));
    }

    fn compact(&mut self) {
        if let Some((index, _)) = &self.snapshot {
            let index = *index;
            self.log = self.log[index..].to_vec();
        }
    }

    fn restore(&mut self) {
        self.data.clear();

        // Start from snapshot if available
        if let Some((_, ref snapshot_data)) = self.snapshot {
            self.data = snapshot_data.clone();
        }

        // Replay remaining log entries
        for (key, value) in &self.log {
            self.data.insert(key.clone(), value.clone());
        }
    }

    fn log_len(&self) -> usize {
        self.log.len()
    }
}

fn main() {
    let mut store = KeyValueStore::new();

    store.set("a".into(), "1".into());
    store.set("b".into(), "2".into());
    store.set("c".into(), "3".into());
    assert_eq!(store.log_len(), 3);

    store.snapshot();

    store.set("d".into(), "4".into());
    store.set("a".into(), "updated".into());

    store.compact();
    assert_eq!(store.log_len(), 2);

    store.restore();
    assert_eq!(store.get("a"), Some(&"updated".to_string()));
    assert_eq!(store.get("b"), Some(&"2".to_string()));
    assert_eq!(store.get("d"), Some(&"4".to_string()));

    println!("All checks passed!");
}
```

The snapshot captures the state at a point in time. After compaction, only the log entries after the snapshot remain. Restore rebuilds the full state by starting from the snapshot and replaying the remaining entries. This is exactly what Raft does — the snapshot is the base state, and the log entries after it are the incremental updates.

</details>

### Drill 3: Crash Recovery Test Harness (Hard)

Build a test harness that simulates crashes at different points in a write sequence and verifies that recovery always produces consistent state.

```rust
use std::collections::HashMap;

/// Simulates a durable store that can crash at any point.
struct CrashableStore {
    /// The "disk" — persisted state.
    disk: Vec<(String, String)>,
    /// The "memory" — current state.
    memory: HashMap<String, String>,
    /// If set, crash after this many disk writes.
    crash_after: Option<usize>,
    /// Number of disk writes so far.
    write_count: usize,
}

#[derive(Debug)]
struct CrashError;

impl CrashableStore {
    fn new() -> Self {
        todo!()
    }

    fn set_crash_point(&mut self, after_n_writes: usize) {
        todo!()
    }

    /// Write to "disk" then update memory. May crash.
    fn set(&mut self, key: String, value: String) -> Result<(), CrashError> {
        todo!()
    }

    /// Recover: rebuild memory from disk.
    fn recover(&mut self) {
        todo!()
    }

    fn get(&self, key: &str) -> Option<&String> {
        self.memory.get(key)
    }

    fn disk_entries(&self) -> usize {
        self.disk.len()
    }
}

fn main() {
    // Test: crash after 2 writes, then recover
    let mut store = CrashableStore::new();
    store.set_crash_point(2);

    store.set("a".into(), "1".into()).unwrap();
    store.set("b".into(), "2".into()).unwrap();
    let result = store.set("c".into(), "3".into()); // should crash
    assert!(result.is_err());

    // Memory might be inconsistent, but disk has 2 entries
    assert_eq!(store.disk_entries(), 2);

    // Recover rebuilds memory from disk
    store.recover();
    assert_eq!(store.get("a"), Some(&"1".to_string()));
    assert_eq!(store.get("b"), Some(&"2".to_string()));
    assert_eq!(store.get("c"), None); // was not persisted

    println!("All checks passed!");
}
```

<details>
<summary>Solution</summary>

```rust
use std::collections::HashMap;

struct CrashableStore {
    disk: Vec<(String, String)>,
    memory: HashMap<String, String>,
    crash_after: Option<usize>,
    write_count: usize,
}

#[derive(Debug)]
struct CrashError;

impl CrashableStore {
    fn new() -> Self {
        CrashableStore {
            disk: Vec::new(),
            memory: HashMap::new(),
            crash_after: None,
            write_count: 0,
        }
    }

    fn set_crash_point(&mut self, after_n_writes: usize) {
        self.crash_after = Some(after_n_writes);
        self.write_count = 0;
    }

    fn set(&mut self, key: String, value: String) -> Result<(), CrashError> {
        // Check if we should crash BEFORE this write
        if let Some(limit) = self.crash_after {
            if self.write_count >= limit {
                return Err(CrashError);
            }
        }

        // Write to disk first (WAL principle)
        self.disk.push((key.clone(), value.clone()));
        self.write_count += 1;

        // Then update memory
        self.memory.insert(key, value);

        Ok(())
    }

    fn recover(&mut self) {
        // Clear potentially inconsistent memory state
        self.memory.clear();
        self.crash_after = None;
        self.write_count = 0;

        // Rebuild from disk (the source of truth)
        for (key, value) in &self.disk {
            self.memory.insert(key.clone(), value.clone());
        }
    }

    fn get(&self, key: &str) -> Option<&String> {
        self.memory.get(key)
    }

    fn disk_entries(&self) -> usize {
        self.disk.len()
    }
}

fn main() {
    let mut store = CrashableStore::new();
    store.set_crash_point(2);

    store.set("a".into(), "1".into()).unwrap();
    store.set("b".into(), "2".into()).unwrap();
    let result = store.set("c".into(), "3".into());
    assert!(result.is_err());

    assert_eq!(store.disk_entries(), 2);

    store.recover();
    assert_eq!(store.get("a"), Some(&"1".to_string()));
    assert_eq!(store.get("b"), Some(&"2".to_string()));
    assert_eq!(store.get("c"), None);

    println!("All checks passed!");
}
```

The key invariant: the disk is the source of truth. Memory is a derived cache that can always be rebuilt from disk. If the process crashes at any point, recovery reads the disk and reconstructs the memory state. Writes that did not reach the disk are lost — which is correct, because they were never acknowledged to the caller.

</details>

### Drill 4: Log Truncation After Snapshot (Medium)

Implement a log that supports truncation. After taking a snapshot, entries up to the snapshot index should be removable, and all index-based operations should still work correctly with the offset.

```rust
struct IndexedLog {
    entries: Vec<String>,
    base_index: usize,  // index of the first entry in `entries`
}

impl IndexedLog {
    fn new() -> Self {
        todo!()
    }

    fn append(&mut self, entry: String) -> usize {
        // Returns the logical index of the appended entry
        todo!()
    }

    fn get(&self, logical_index: usize) -> Option<&String> {
        todo!()
    }

    /// Remove all entries up to and including `through_index`.
    fn truncate_prefix(&mut self, through_index: usize) {
        todo!()
    }

    fn len(&self) -> usize {
        self.entries.len()
    }

    fn first_index(&self) -> usize {
        self.base_index
    }

    fn last_index(&self) -> usize {
        self.base_index + self.entries.len() - 1
    }
}

fn main() {
    let mut log = IndexedLog::new();

    // Append entries (indices 0, 1, 2, 3, 4)
    for i in 0..5 {
        log.append(format!("entry-{}", i));
    }
    assert_eq!(log.len(), 5);
    assert_eq!(log.get(0), Some(&"entry-0".to_string()));
    assert_eq!(log.get(4), Some(&"entry-4".to_string()));

    // Truncate through index 2 (simulating snapshot at index 2)
    log.truncate_prefix(2);
    assert_eq!(log.len(), 2);  // only entries 3 and 4 remain
    assert_eq!(log.first_index(), 3);
    assert_eq!(log.last_index(), 4);
    assert_eq!(log.get(3), Some(&"entry-3".to_string()));
    assert_eq!(log.get(4), Some(&"entry-4".to_string()));
    assert_eq!(log.get(2), None);  // truncated
    assert_eq!(log.get(0), None);  // truncated

    // Append more entries after truncation
    let idx = log.append("entry-5".to_string());
    assert_eq!(idx, 5);
    assert_eq!(log.get(5), Some(&"entry-5".to_string()));

    println!("All checks passed!");
}
```

<details>
<summary>Solution</summary>

```rust
struct IndexedLog {
    entries: Vec<String>,
    base_index: usize,
}

impl IndexedLog {
    fn new() -> Self {
        IndexedLog {
            entries: Vec::new(),
            base_index: 0,
        }
    }

    fn append(&mut self, entry: String) -> usize {
        let index = self.base_index + self.entries.len();
        self.entries.push(entry);
        index
    }

    fn get(&self, logical_index: usize) -> Option<&String> {
        if logical_index < self.base_index {
            return None; // truncated
        }
        let physical = logical_index - self.base_index;
        self.entries.get(physical)
    }

    fn truncate_prefix(&mut self, through_index: usize) {
        if through_index < self.base_index {
            return; // already truncated past this point
        }
        let remove_count = through_index - self.base_index + 1;
        if remove_count >= self.entries.len() {
            self.base_index = through_index + 1;
            self.entries.clear();
        } else {
            self.entries = self.entries[remove_count..].to_vec();
            self.base_index = through_index + 1;
        }
    }

    fn len(&self) -> usize {
        self.entries.len()
    }

    fn first_index(&self) -> usize {
        self.base_index
    }

    fn last_index(&self) -> usize {
        self.base_index + self.entries.len() - 1
    }
}

fn main() {
    let mut log = IndexedLog::new();

    for i in 0..5 {
        log.append(format!("entry-{}", i));
    }
    assert_eq!(log.len(), 5);
    assert_eq!(log.get(0), Some(&"entry-0".to_string()));
    assert_eq!(log.get(4), Some(&"entry-4".to_string()));

    log.truncate_prefix(2);
    assert_eq!(log.len(), 2);
    assert_eq!(log.first_index(), 3);
    assert_eq!(log.last_index(), 4);
    assert_eq!(log.get(3), Some(&"entry-3".to_string()));
    assert_eq!(log.get(4), Some(&"entry-4".to_string()));
    assert_eq!(log.get(2), None);
    assert_eq!(log.get(0), None);

    let idx = log.append("entry-5".to_string());
    assert_eq!(idx, 5);
    assert_eq!(log.get(5), Some(&"entry-5".to_string()));

    println!("All checks passed!");
}
```

The `base_index` is the key abstraction. Physical index 0 in the `Vec` corresponds to logical index `base_index`. When we truncate, we advance `base_index` and remove the front of the `Vec`. All external code uses logical indices — the truncation is invisible to callers. This is the same pattern that etcd's Raft implementation uses for its in-memory log.

</details>

---

## DSA in Context: Snapshot Isolation

You just built snapshots for log compaction. The same concept — capturing a point-in-time view — appears throughout database systems under the name **snapshot isolation**.

### The newspaper analogy

Imagine a newspaper. The morning edition captures the state of the world at a specific moment. After printing, reporters continue gathering news — but the printed edition does not change. Readers of the morning edition all see the same consistent view, even as the world evolves around them.

A database snapshot works the same way. At time T, you "print" the database state. Readers using that snapshot see the database as it was at time T, even as other transactions write new data. This is snapshot isolation — every transaction sees a consistent snapshot, not a mix of old and new data.

### How it connects to Raft snapshots

Raft snapshots serve a different purpose (log compaction, not transaction isolation), but the mechanism is identical:

| Concept | MVCC Snapshot Isolation | Raft Snapshot |
|---------|------------------------|---------------|
| What it captures | Database state at a transaction timestamp | State machine state at a log index |
| Why it exists | Consistent reads without locking | Log compaction + follower catch-up |
| How it is created | Record the current version number | Serialize state machine + note the log index |
| What it replaces | Nothing (readers just use the snapshot) | All log entries up to the snapshot index |

In Chapter 5, you built MVCC (multi-version concurrency control), which provides snapshot isolation for readers. Raft snapshots are the distributed version of the same idea — freezing state at a point in time so you can discard the history that led to it.

### Log-structured storage and compaction

The WAL + snapshot pattern mirrors log-structured merge trees (LSM-trees), used by LevelDB, RocksDB, and Cassandra:

1. **Write path:** Append to a log (fast, sequential writes)
2. **Accumulation:** The log grows without bound
3. **Compaction:** Merge/snapshot to reclaim space
4. **Read path:** Check the most recent compacted state, then check the log for newer entries

The tradeoff is always the same: fast writes (append-only) at the cost of periodic compaction work. The art is in choosing when and how aggressively to compact.

---

## System Design Corner: Durability Guarantees and Recovery

Durability is one of the ACID properties (Atomicity, Consistency, Isolation, **Durability**). It means: once a transaction is committed, its effects will not be lost, even if the system crashes. Let us examine how real systems achieve this.

### Levels of durability

| Level | Guarantee | Mechanism | Example |
|-------|-----------|-----------|---------|
| None | Data lost on crash | In-memory only | Redis without persistence |
| Process crash | Survives process crash | OS page cache | Most file writes without fsync |
| OS crash | Survives kernel panic | fsync to disk | PostgreSQL with `synchronous_commit=on` |
| Power loss | Survives power failure | fsync + battery-backed write cache | Enterprise databases on enterprise hardware |
| Disk failure | Survives disk death | Replication to multiple disks | Raft/Paxos with 3+ nodes |
| Datacenter failure | Survives site loss | Cross-datacenter replication | CockroachDB multi-region |

Our Raft cluster with fsync provides "disk failure" level durability — if any minority of nodes lose their disks, the remaining majority still has the data. This is why Raft requires a majority to commit: with 3 nodes, any 1 can fail; with 5 nodes, any 2 can fail.

### Write-ahead logging in production databases

PostgreSQL's WAL is the gold standard for single-node durability:

1. **Transaction writes:** All changes go to the WAL first, not to the data files
2. **Commit:** `fsync` the WAL (the write is now durable)
3. **Checkpoint:** Periodically apply WAL changes to the actual data files
4. **Recovery:** On crash, replay the WAL from the last checkpoint

This is exactly our pattern: WAL for durability, snapshots (checkpoints) for compaction. PostgreSQL's `pg_xlog` directory is its WAL. `pg_basebackup` creates snapshots. `pg_dump` is a logical snapshot.

### The fsync controversy

In 2018, PostgreSQL developers discovered that Linux's `fsync()` behavior had a dangerous edge case: if the kernel fails to write a dirty page to disk (due to a disk error), it marks the page as clean anyway. A subsequent `fsync()` call returns success because the page is "clean" — even though the data never reached the disk. This means **a single fsync failure can silently lose data**.

PostgreSQL 12 added handling for this case (retry the write rather than trusting the cached page). The lesson: even `fsync` is not as simple as it appears. Durability is a system-wide property that depends on the application, the OS, the filesystem, the disk controller, and the physical disk all behaving correctly.

> **Interview talking point:** *"Our Raft implementation provides durability through a write-ahead log with CRC checksums and fsync. State metadata uses atomic writes via temp file + rename + directory fsync. Snapshots compact old log entries to bound recovery time and disk usage. For crash recovery, we read the metadata file for term/vote, replay the WAL for log entries, and start as a follower. The key invariant is that the disk state is always at least as recent as any acknowledgment we sent — we write to disk before responding to RPCs."*

---

## Design Insight: Design It Twice

> *"Designing software is hard, so it's unlikely that your first idea will be the best one. You'll end up with a much better result if you consider multiple options."*
> — John Ousterhout, *A Philosophy of Software Design*

We could have designed the WAL differently. Here are two alternatives:

### Design A: Single-file WAL (our approach)

```
One file: raft.wal
Entries appended sequentially.
Compaction: rewrite the entire file.

Pros: Simple. One file to manage. Easy to reason about.
Cons: Compaction requires rewriting everything. Large WAL means
      slow compaction.
```

### Design B: Segmented WAL

```
Multiple files: wal-0001.log, wal-0002.log, wal-0003.log
Each segment holds up to N entries.
New segment created when current one is full.
Compaction: delete old segment files.

Pros: Compaction is O(1) — just delete files. No rewriting.
Cons: More files to manage. Reads span multiple files.
      Need to track which segment contains which indices.
```

### Design C: Embedded key-value store for WAL

```
Use sled or RocksDB as the WAL backend.
Each entry stored with its index as the key.
Compaction: delete keys by range.

Pros: Proven, crash-safe, handles all the file management.
Cons: Heavy dependency. Hides the learning. Hard to debug.
```

We chose Design A because it is the simplest and most instructive. A production system would likely use Design B (segmented WAL) — it is what etcd, CockroachDB, and TiKV use. Design C trades understanding for convenience, which is the wrong tradeoff for a learning project.

The point of "design it twice" is not to implement all three — it is to *think through* all three before committing to one. The act of comparing designs reveals tradeoffs that you would not see by diving straight into implementation.

---

## What You Built

In this chapter, you:

1. **Built a write-ahead log** — append-only file with CRC32 checksums, length-prefixed records, and torn write detection
2. **Persisted Raft metadata** — atomic file writes using temp file + rename + directory fsync for `current_term`, `voted_for`, and `commit_index`
3. **Implemented crash recovery** — reading the WAL and metadata file on startup, reconstructing in-memory state, always starting as a follower
4. **Built a snapshot system** — capturing state machine state at a log index, discarding old log entries, atomic snapshot storage
5. **Implemented InstallSnapshot RPC** — transferring snapshots to lagging followers, truncating their logs, restoring state
6. **Practiced ownership patterns** — file handles as owned resources, `&mut self` for exclusive write access, `Drop` for deterministic cleanup, `BufWriter` for batched I/O

Your Raft cluster can now survive crashes. Stop a node, restart it, and it recovers its state from disk. Kill all nodes, restart them, and they re-elect a leader and resume operation with all committed data intact. This is the difference between a protocol implementation and a production system.

Chapter 17 connects everything: SQL parsing, query planning, execution, MVCC storage, and Raft consensus — a complete distributed SQL database.

---

### DS Deep Dive

Our WAL uses a simple single-file format with sequential writes. Production systems like etcd and CockroachDB use segmented WALs with pre-allocated files and direct I/O to bypass the OS page cache entirely. This deep dive explores WAL design space — segment sizing, pre-allocation for predictable latency, group commit for batching fsync across concurrent writers, and the tradeoffs between mmap-based and read/write-based WAL implementations.
