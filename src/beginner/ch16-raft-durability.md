# Chapter 16: Raft -- Durability & Recovery

Your Raft cluster elects leaders and replicates log entries. Everything works beautifully -- until you pull the plug. Turn off the power, restart the server, and what happens?

Nothing. The server wakes up with a blank memory. It does not know what term it was in, who it voted for, or what entries were in its log. All that carefully replicated data? Gone.

This is not a theoretical concern. Servers crash. Hard drives fail. Operating systems kernel-panic. Data centers lose power. A database that cannot survive a restart is a toy. This chapter makes it real.

You will build a **write-ahead log (WAL)** that persists Raft state to disk, a recovery procedure that reads everything back on startup, and snapshots that summarize old history so the log does not grow forever.

By the end of this chapter, you will have:

- An understanding of why durability matters and what must be persisted
- A write-ahead log that writes entries to disk with checksums
- Persistent storage for `current_term` and `voted_for`
- A recovery procedure that reconstructs state from disk on startup
- Snapshots for compacting old log entries
- A solid understanding of file ownership in Rust

---

## Spotlight: Ownership & Persistence

Every chapter has one **spotlight concept**. This chapter's spotlight is **ownership and persistence** -- how Rust's ownership model naturally maps to questions about file handles, resource lifetimes, and who is responsible for what.

### File handles are owned resources

In languages with garbage collectors (JavaScript, Python, Java), a file handle might stay open long after you stop using it. The garbage collector "eventually" cleans it up, but "eventually" might be too late -- file descriptors are limited, and leaving files open can cause data corruption.

In Rust, a file handle is an owned value. When the owner goes out of scope, the file is closed. This happens immediately and deterministically:

```rust
use std::fs::File;
use std::io::Write;

fn save_term(path: &str, term: u64) -> std::io::Result<()> {
    let mut file = File::create(path)?;   // file opened here
    file.write_all(&term.to_le_bytes())?;
    file.sync_all()?;
    Ok(())
    // file closed here automatically -- Drop trait runs
}
```

There is no `file.close()` call. When `file` goes out of scope at the end of the function, Rust calls its `Drop` implementation, which closes the file descriptor. This is called **RAII** (Resource Acquisition Is Initialization) -- you acquire the resource when you create the variable, and release it when the variable is dropped.

> **Programming Concept: The Drop Trait**
>
> When a value goes out of scope in Rust, the compiler automatically calls `drop()` on it. The `File` type implements `Drop` to close the file descriptor. You can implement `Drop` for your own types too -- for example, to flush a buffer or release a lock. This is how Rust guarantees that resources are always cleaned up, even if your function returns early due to an error (the `?` operator).

### Ownership determines responsibility

When building a WAL (Write-Ahead Log), someone must own the file handle. This is not just a compiler concept -- it is a design decision about responsibility:

```rust,ignore
/// The WAL writer owns the file handle.
/// When the writer is dropped, the file is closed.
struct WalWriter {
    file: File,         // WalWriter owns this
    path: PathBuf,      // WalWriter knows where the file lives
    entry_count: u64,   // WalWriter tracks its position
}

/// The RaftNode owns the WalWriter.
/// When the node is dropped, the writer is dropped, which closes the file.
struct RaftNode {
    wal: WalWriter,     // ownership chain: Node -> Writer -> File
    // ... other fields
}
```

The ownership chain is explicit: `RaftNode` owns `WalWriter`, which owns `File`. When `RaftNode` is dropped, `WalWriter` is dropped, which drops `File`, which closes the file descriptor. No manual cleanup needed, no finalizers, no "remember to close the file."

### Mutable references and exclusive file access

Rust's borrow checker guarantees that only one mutable reference to a value exists at a time. For file I/O, this naturally enforces exclusive write access:

```rust,ignore
impl WalWriter {
    // &mut self means: only one caller can write at a time.
    // The compiler enforces this!
    fn append(&mut self, entry: &LogEntry) -> std::io::Result<()> {
        let bytes = entry.serialize();
        self.file.write_all(&bytes)?;
        self.entry_count += 1;
        Ok(())
    }
}
```

The `&mut self` signature means you need exclusive access to the writer. If two threads tried to write simultaneously, the compiler would refuse to compile the code. This is the same guarantee that file locks provide at the operating system level, but enforced at compile time.

### BufWriter: batching writes for performance

Writing to disk byte-by-byte is slow because each `write()` call is a system call (a round trip to the operating system kernel). `BufWriter` collects writes in a memory buffer and sends them in batches:

```rust,ignore
use std::io::BufWriter;

let file = File::create("raft.wal")?;
let mut writer = BufWriter::new(file);

// These go to the buffer, not the disk
writer.write_all(&entry1_bytes)?;
writer.write_all(&entry2_bytes)?;

// This sends the buffer to the OS kernel
writer.flush()?;

// This forces the OS to write to the PHYSICAL DISK
writer.get_ref().sync_all()?;
```

There is an important distinction:
- **`flush()`** moves data from your program's buffer to the operating system's buffer. The OS "has" the data but has not written it to disk yet.
- **`sync_all()`** forces the OS to write its buffer to the actual physical disk. This is the `fsync` system call. After it returns, the data survives a power loss.

For durability, you need both: flush your buffer, then sync to disk.

> **What Just Happened?**
>
> We explored how Rust's ownership model maps to file I/O:
> - **Ownership** determines who holds the file handle and when it is closed
> - **Mutable references** (`&mut self`) prevent concurrent writes at compile time
> - **RAII/Drop** ensures files are closed even when errors occur
> - **BufWriter** batches writes for performance
> - **`sync_all()`** guarantees data reaches the physical disk

---

## Why Durability Matters

Without persistence, your Raft cluster is a distributed in-memory cache with extra complexity. Consider what happens when a node crashes:

```
Before crash:
  Node A (Leader):   term=5, log=[1,2,3,4,5], commit_index=4
  Node B (Follower): term=5, log=[1,2,3,4,5], commit_index=4
  Node C (Follower): term=5, log=[1,2,3,4,5], commit_index=4

After Node A crashes and restarts (WITHOUT persistence):
  Node A:            term=0, log=[], commit_index=0    <-- amnesia!
  Node B (Follower): term=5, log=[1,2,3,4,5], commit_index=4
  Node C (Follower): term=5, log=[1,2,3,4,5], commit_index=4
```

Node A has amnesia. It does not know it was ever leader. It does not know what term the cluster is in. It will try to start a new election at term 1, which the other nodes will ignore because they are at term 5.

Worse: if two nodes crash and restart simultaneously, the cluster might lose committed data. Entries that a majority had acknowledged are now gone from two of three nodes.

### What must be persisted

The Raft paper is explicit about three things that must survive crashes:

1. **`current_term`** -- the latest term the server has seen
2. **`voted_for`** -- who it voted for in the current term (prevents double-voting)
3. **`log[]`** -- the log entries themselves

Everything else -- `commit_index`, `last_applied`, `leader_id`, `match_index` -- can be reconstructed from these three plus communication with the cluster.

---

## Exercise 1: The Write-Ahead Log (WAL)

**Goal:** Build a `WalWriter` that persists Raft log entries to disk.

### Step 1: The on-disk format

Each WAL record has a header followed by the entry data:

```
WAL Record Format:
+----------+----------+----------+------------------+
| 4 bytes  | 4 bytes  | 4 bytes  | N bytes          |
| CRC32    | Length   | Entry    | Serialized       |
| checksum | (N)     | Index    | LogEntry         |
+----------+----------+----------+------------------+
```

The **CRC32 checksum** is a number computed from the rest of the record. On recovery, we recompute the checksum and compare. If they do not match, we know the record was partially written (the server crashed mid-write) and we discard it.

> **Programming Concept: What is a Checksum?**
>
> A checksum is a small number computed from a larger piece of data. Think of it like a "summary" of the data. If even one byte of the data changes, the checksum changes too. By storing the checksum alongside the data, we can later verify that the data was not corrupted.
>
> CRC32 is a specific checksum algorithm that produces a 32-bit (4-byte) number. It is fast, widely used, and good at detecting the kind of corruption that happens with partial disk writes.

### Step 2: Serialize log entries

```rust,ignore
// src/raft/wal.rs

use std::fs::{File, OpenOptions};
use std::io::{self, BufWriter, Read, Write, Seek, SeekFrom};
use std::path::{Path, PathBuf};

impl LogEntry {
    /// Convert a log entry to bytes for storage.
    /// Format: 8 bytes term + 8 bytes index + N bytes command
    pub fn serialize(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(16 + self.command.len());
        bytes.extend_from_slice(&self.term.to_le_bytes());
        bytes.extend_from_slice(&self.index.to_le_bytes());
        bytes.extend_from_slice(&self.command);
        bytes
    }

    /// Reconstruct a log entry from bytes.
    pub fn deserialize(bytes: &[u8]) -> Result<Self, WalError> {
        if bytes.len() < 16 {
            return Err(WalError::CorruptEntry(
                "entry too short".to_string()
            ));
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

> **Programming Concept: `to_le_bytes()` and `from_le_bytes()`**
>
> These methods convert numbers to and from byte arrays using **little-endian** byte order. "Little-endian" means the least significant byte comes first. For example, the number 256 (which is 0x0100 in hex) is stored as `[0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]` in 8 bytes.
>
> Why do we need to convert at all? Because files store bytes, not numbers. To write a `u64` to disk, we must decide the byte order and stick with it. Little-endian is the convention on most modern CPUs.

### Step 3: Implement CRC32

```rust,ignore
/// Compute CRC32 checksum.
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFFFFFF;
    for &byte in data {
        let index = ((crc ^ byte as u32) & 0xFF) as usize;
        crc = CRC32_TABLE[index] ^ (crc >> 8);
    }
    crc ^ 0xFFFFFFFF
}

/// CRC32 lookup table, computed at compile time.
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

> **What Just Happened?**
>
> The `CRC32_TABLE` is a lookup table with 256 entries, one for each possible byte value. Instead of doing 8 bit operations per input byte, we do one table lookup. This is a classic time-space tradeoff: we use 1 KB of memory (256 * 4 bytes) to make the checksum computation much faster.
>
> The `const` keyword means this table is computed at compile time. Rust evaluates the `while` loops when compiling your program, and the result is baked directly into the binary. At runtime, the table is just sitting in memory, ready to use.

### Step 4: Define the error type

```rust,ignore
#[derive(Debug)]
pub enum WalError {
    /// An I/O error (file not found, permission denied, etc.)
    Io(io::Error),
    /// A corrupt entry in the WAL
    CorruptEntry(String),
    /// The checksum did not match
    ChecksumMismatch { expected: u32, actual: u32 },
}

/// This lets us use the ? operator with io::Error
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
                write!(f, "checksum mismatch: expected {:#010x}, got {:#010x}",
                    expected, actual)
            }
        }
    }
}
```

> **Programming Concept: The `From` Trait and `?` Operator**
>
> When you write `file.write_all(&data)?`, the `?` operator does two things:
> 1. If the result is `Ok`, unwrap the value and continue
> 2. If the result is `Err`, convert the error to the function's return type and return early
>
> The conversion in step 2 uses the `From` trait. By implementing `From<io::Error> for WalError`, we tell Rust: "whenever you need to convert an `io::Error` into a `WalError`, use this function." This lets us use `?` with `io::Error` in functions that return `Result<T, WalError>`.

### Step 5: Build the WAL writer

```rust,ignore
/// Write-ahead log for persisting Raft log entries.
/// The WalWriter owns the file handle -- when it is dropped,
/// the file is closed.
pub struct WalWriter {
    writer: BufWriter<File>,
    path: PathBuf,
    entry_count: u64,
}

impl WalWriter {
    /// Open or create a WAL file.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, WalError> {
        let path = path.as_ref().to_path_buf();
        let file = OpenOptions::new()
            .create(true)    // create the file if it does not exist
            .read(true)      // we might need to read during recovery
            .append(true)    // always write at the end
            .open(&path)?;

        Ok(WalWriter {
            writer: BufWriter::new(file),
            path,
            entry_count: 0,
        })
    }

    /// Write a log entry to the WAL.
    pub fn append(&mut self, entry: &LogEntry) -> Result<(), WalError> {
        let payload = entry.serialize();
        let index_bytes = (entry.index as u32).to_le_bytes();

        // Build the data that the CRC covers
        let length = payload.len() as u32;
        let mut crc_data = Vec::with_capacity(8 + payload.len());
        crc_data.extend_from_slice(&length.to_le_bytes());
        crc_data.extend_from_slice(&index_bytes);
        crc_data.extend_from_slice(&payload);

        let checksum = crc32(&crc_data);

        // Write the record: CRC + length + index + payload
        self.writer.write_all(&checksum.to_le_bytes())?;
        self.writer.write_all(&length.to_le_bytes())?;
        self.writer.write_all(&index_bytes)?;
        self.writer.write_all(&payload)?;

        self.entry_count += 1;
        Ok(())
    }

    /// Flush the buffer and sync to disk.
    /// After this returns, the data is guaranteed to be on
    /// the physical disk and will survive a power loss.
    pub fn sync(&mut self) -> Result<(), WalError> {
        self.writer.flush()?;
        self.writer.get_ref().sync_all()?;
        Ok(())
    }

    /// Append and immediately sync (for critical writes).
    pub fn append_sync(&mut self, entry: &LogEntry) -> Result<(), WalError> {
        self.append(entry)?;
        self.sync()?;
        Ok(())
    }
}
```

> **What Just Happened?**
>
> We built a WAL writer that:
> 1. Opens a file in append mode (new writes go at the end)
> 2. Wraps it in a `BufWriter` for performance (batches small writes)
> 3. Writes each entry with a CRC32 checksum for corruption detection
> 4. Provides a `sync` method that guarantees data reaches the physical disk
>
> The `append_sync` method is for critical state changes (like term updates and votes). Losing these would violate Raft's safety guarantees, so we sync after every write.

---

## Exercise 2: The WAL Reader (Recovery)

**Goal:** Build a reader that recovers log entries from the WAL file on startup.

### Step 1: The recovery process

When a server starts up, it reads the WAL file and reconstructs its in-memory state:

```rust,ignore
/// Reads WAL entries from disk. Used during recovery.
pub struct WalReader {
    path: PathBuf,
}

impl WalReader {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, WalError> {
        let path = path.as_ref().to_path_buf();
        Ok(WalReader { path })
    }

    /// Read all valid entries from the WAL.
    /// Stops at the first corrupt or incomplete record.
    pub fn read_all(&self) -> Result<Vec<LogEntry>, WalError> {
        // If the file does not exist, there is nothing to recover
        if !self.path.exists() {
            return Ok(Vec::new());
        }

        let mut file = File::open(&self.path)?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)?;

        let mut entries = Vec::new();
        let mut pos = 0;

        while pos + 12 <= buf.len() {
            // Read header: CRC (4) + length (4) + index (4)
            let stored_crc = u32::from_le_bytes(
                buf[pos..pos + 4].try_into().unwrap()
            );
            let length = u32::from_le_bytes(
                buf[pos + 4..pos + 8].try_into().unwrap()
            ) as usize;

            // Check if we have enough bytes for the payload
            if pos + 12 + length > buf.len() {
                // Incomplete record -- the server crashed mid-write.
                // This is normal! We just stop here.
                println!("WAL: incomplete record at byte {}, truncating", pos);
                break;
            }

            let payload = &buf[pos + 12..pos + 12 + length];

            // Verify the checksum
            let crc_data = &buf[pos + 4..pos + 12 + length];
            let computed_crc = crc32(crc_data);

            if stored_crc != computed_crc {
                // Corrupt record. Stop here -- everything after
                // this point is suspect.
                println!(
                    "WAL: corrupt record at byte {} (checksum mismatch), truncating",
                    pos
                );
                break;
            }

            // Deserialize the entry
            let entry = LogEntry::deserialize(payload)?;
            entries.push(entry);

            pos += 12 + length;
        }

        println!("WAL: recovered {} entries", entries.len());
        Ok(entries)
    }
}
```

> **What Just Happened?**
>
> The recovery reader scans the WAL file from beginning to end. For each record:
> 1. Read the header (CRC, length, index)
> 2. Check if the full payload is present (handles incomplete writes)
> 3. Verify the CRC checksum (detects corruption)
> 4. If everything checks out, deserialize the entry
>
> If a record is incomplete or corrupt, we stop. Everything before that point is good; the partial record at the end is discarded. This handles the case where the server crashed mid-write -- the partial write is simply ignored.

### Step 2: Understand torn writes

What happens if the server crashes in the middle of writing a record?

```
Normal write:
  [CRC] [LEN] [IDX] [PAYLOAD]    <-- complete record

Crash during write:
  [CRC] [LEN] [IDX] [PAY---      <-- incomplete payload

Crash before CRC is written:
  [garbage bytes]                 <-- checksum will not match
```

Our reader handles both cases:
- **Incomplete payload:** `pos + 12 + length > buf.len()` catches this. We stop.
- **Corrupt CRC:** `stored_crc != computed_crc` catches this. We stop.

In both cases, the partial record is discarded, and all complete records before it are recovered. This is the beauty of append-only logs with checksums -- recovery is simple and robust.

---

## Exercise 3: Persisting Term and Vote

**Goal:** Save `current_term` and `voted_for` to disk so they survive crashes.

### Step 1: Why these matter

If a node forgets its `current_term`, it might vote in a term it has already participated in. If it forgets `voted_for`, it might vote for two different candidates in the same term. Both violations can lead to two leaders being elected, which is the one thing Raft must prevent.

### Step 2: Simple file-based persistence

```rust,ignore
use std::path::Path;

/// Persist the current term and voted_for to a file.
pub fn save_raft_state(
    path: &Path,
    current_term: Term,
    voted_for: Option<NodeId>,
) -> Result<(), WalError> {
    let mut data = Vec::with_capacity(17);

    // 8 bytes: current term
    data.extend_from_slice(&current_term.to_le_bytes());

    // 1 byte: whether voted_for is Some or None
    // 8 bytes: the voted_for value (or zeros)
    match voted_for {
        Some(id) => {
            data.push(1);  // 1 = Some
            data.extend_from_slice(&id.to_le_bytes());
        }
        None => {
            data.push(0);  // 0 = None
            data.extend_from_slice(&0u64.to_le_bytes());
        }
    }

    // Write atomically: write to a temp file, then rename.
    // This ensures we never have a half-written state file.
    let temp_path = path.with_extension("tmp");
    let mut file = File::create(&temp_path)?;
    file.write_all(&data)?;
    file.sync_all()?;
    std::fs::rename(&temp_path, path)?;

    Ok(())
}

/// Load the current term and voted_for from a file.
pub fn load_raft_state(
    path: &Path,
) -> Result<(Term, Option<NodeId>), WalError> {
    if !path.exists() {
        return Ok((0, None));  // fresh start
    }

    let data = std::fs::read(path)?;
    if data.len() < 17 {
        return Err(WalError::CorruptEntry(
            "state file too short".to_string()
        ));
    }

    let current_term = u64::from_le_bytes(
        data[0..8].try_into().unwrap()
    );

    let voted_for = if data[8] == 1 {
        let id = u64::from_le_bytes(
            data[9..17].try_into().unwrap()
        );
        Some(id)
    } else {
        None
    };

    Ok((current_term, voted_for))
}
```

> **Programming Concept: Atomic File Writes**
>
> We write to a temporary file first, sync it, then rename it over the real file. Why? Because `rename` is atomic on most file systems -- it either completes fully or not at all. If we wrote directly to the state file and crashed mid-write, we would have a corrupted file. The temp-file-then-rename pattern ensures the state file is always complete and valid.

---

## Exercise 4: Full Recovery

**Goal:** Put it all together -- recover a RaftNode from disk on startup.

### Step 1: The recovery function

```rust,ignore
impl RaftNode {
    /// Create a RaftNode by recovering state from disk.
    /// If no state exists on disk, create a fresh node.
    pub fn recover(
        id: NodeId,
        peers: Vec<NodeId>,
        data_dir: &Path,
    ) -> Result<Self, WalError> {
        let state_path = data_dir.join("raft_state");
        let wal_path = data_dir.join("raft.wal");

        // Step 1: Load persisted term and vote
        let (current_term, voted_for) = load_raft_state(&state_path)?;
        println!(
            "[Node {}] Recovered: term={}, voted_for={:?}",
            id, current_term, voted_for
        );

        // Step 2: Load the log from the WAL
        let reader = WalReader::open(&wal_path)?;
        let entries = reader.read_all()?;
        let mut log = RaftLog::new();
        for entry in entries {
            log.append(entry.term, entry.command);
        }
        println!(
            "[Node {}] Recovered {} log entries",
            id, log.len()
        );

        // Step 3: Create the node with recovered state
        let election_timeout = Self::random_election_timeout();
        let node = RaftNode {
            id,
            peers,
            state: NodeState::Follower,  // always start as follower
            current_term,
            voted_for,
            leader_id: None,  // will learn from heartbeats
            election_deadline: Instant::now() + election_timeout,
            election_timeout,
            votes_received: HashSet::new(),
            log,
            commit_index: 0,  // will be updated by leader
            last_applied: 0,  // will catch up after recovery
            next_index: HashMap::new(),
            match_index: HashMap::new(),
        };

        println!("[Node {}] Recovery complete", id);
        Ok(node)
    }
}
```

Notice that the recovered node always starts as a Follower. Even if it was the Leader before the crash, it does not know the current state of the cluster. It will learn who the leader is from heartbeats, or it will start a new election if the timeout fires.

Also notice that `commit_index` and `last_applied` start at 0. The leader will inform the node of the correct `commit_index` via `AppendEntries`. The node will then apply committed entries to the database.

> **What Just Happened?**
>
> Recovery is a three-step process:
> 1. **Load term and vote** from the state file -- restores election safety
> 2. **Load log entries** from the WAL -- restores the replicated data
> 3. **Create the node** as a Follower -- safely rejoin the cluster
>
> The node does not try to resume where it left off. It starts fresh as a Follower and lets the Raft protocol guide it to the correct state. This simplicity is one of Raft's strengths -- recovery is just "read the state, start as follower, let the protocol handle the rest."

---

## Exercise 5: Snapshots

**Goal:** Implement snapshots to compact old log entries.

### The problem

The log grows forever. If the database has been running for a year with millions of writes, the log has millions of entries. Recovery means replaying all of them, which could take hours. And new followers that join the cluster need to receive the entire log.

### The solution: snapshots

A **snapshot** captures the current state of the database at a point in time. Once a snapshot is taken, all log entries before that point can be discarded -- the snapshot contains the result of applying them.

Think of it like a textbook. The textbook (snapshot) contains everything students need to know. The teacher does not need to repeat every lesson from the beginning of the year -- they just hand out the textbook and continue from the current chapter.

```
Before snapshot:
  Log: [1] [2] [3] [4] [5] [6] [7] [8] [9] [10]
                                           ^
                                    commit_index = 10

After snapshot at index 7:
  Snapshot: {complete database state at index 7}
  Log: [8] [9] [10]
```

### Step 1: Define the snapshot

```rust,ignore
/// A snapshot of the database state at a specific log index.
pub struct Snapshot {
    /// The last log index included in this snapshot.
    pub last_included_index: u64,
    /// The term of the last log index included.
    pub last_included_term: Term,
    /// The serialized database state.
    pub data: Vec<u8>,
}

impl Snapshot {
    /// Save the snapshot to a file.
    pub fn save(&self, path: &Path) -> Result<(), WalError> {
        let mut file = File::create(path)?;

        // Header: index (8) + term (8) + data length (8)
        file.write_all(&self.last_included_index.to_le_bytes())?;
        file.write_all(&self.last_included_term.to_le_bytes())?;
        file.write_all(&(self.data.len() as u64).to_le_bytes())?;

        // Data
        file.write_all(&self.data)?;

        file.sync_all()?;
        Ok(())
    }

    /// Load a snapshot from a file.
    pub fn load(path: &Path) -> Result<Option<Self>, WalError> {
        if !path.exists() {
            return Ok(None);
        }

        let data = std::fs::read(path)?;
        if data.len() < 24 {
            return Err(WalError::CorruptEntry(
                "snapshot file too short".to_string()
            ));
        }

        let last_included_index = u64::from_le_bytes(
            data[0..8].try_into().unwrap()
        );
        let last_included_term = u64::from_le_bytes(
            data[8..16].try_into().unwrap()
        );
        let data_len = u64::from_le_bytes(
            data[16..24].try_into().unwrap()
        ) as usize;

        let snapshot_data = data[24..24 + data_len].to_vec();

        Ok(Some(Snapshot {
            last_included_index,
            last_included_term,
            data: snapshot_data,
        }))
    }
}
```

### Step 2: Trigger snapshots

```rust,ignore
impl RaftNode {
    /// Check if it is time to take a snapshot.
    /// We snapshot when the log exceeds a threshold.
    pub fn should_snapshot(&self, max_log_entries: u64) -> bool {
        self.log.len() as u64 > max_log_entries
    }

    /// Create a snapshot from the current state.
    /// The caller provides the serialized database state.
    pub fn create_snapshot(
        &mut self,
        database_state: Vec<u8>,
    ) -> Snapshot {
        let snapshot = Snapshot {
            last_included_index: self.last_applied,
            last_included_term: self.log.term_at(self.last_applied),
            data: database_state,
        };

        // Discard log entries up to the snapshot
        println!(
            "[Node {}] Creating snapshot at index {}, discarding {} entries",
            self.id,
            self.last_applied,
            self.last_applied,
        );

        snapshot
    }
}
```

> **Common Mistake: Snapshotting Uncommitted Entries**
>
> Only snapshot up to `last_applied`, not `commit_index` or `last_index`. The snapshot must reflect the actual database state, which only includes applied entries. Snapshotting beyond what has been applied would capture an inconsistent state.

---

## Exercises

### Exercise A: WAL Write-and-Read Test

Write a test that creates a WAL, writes 5 entries, closes the writer, opens a reader, and verifies all 5 entries are recovered correctly.

<details>
<summary>Hint</summary>

Use `tempfile::tempdir()` for a temporary directory. Write entries with `append_sync`, drop the writer, create a reader, and `read_all()`.

```rust,ignore
#[test]
fn test_wal_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.wal");

    // Write
    {
        let mut writer = WalWriter::open(&path).unwrap();
        for i in 1..=5 {
            let entry = LogEntry {
                term: 1,
                index: i,
                command: format!("cmd-{}", i).into_bytes(),
            };
            writer.append_sync(&entry).unwrap();
        }
    }  // writer dropped, file closed

    // Read
    let reader = WalReader::open(&path).unwrap();
    let entries = reader.read_all().unwrap();
    assert_eq!(entries.len(), 5);
    assert_eq!(entries[0].index, 1);
    assert_eq!(entries[4].index, 5);
}
```

</details>

### Exercise B: Simulated Crash Recovery

Simulate a crash by writing 3 entries to the WAL, then appending random garbage bytes (simulating a partial write). Read the WAL back and verify that exactly 3 good entries are recovered.

<details>
<summary>Hint</summary>

After closing the WAL writer, open the file in append mode with `OpenOptions` and write some garbage bytes. Then open the reader and verify the count.

</details>

### Exercise C: State File Roundtrip

Test `save_raft_state` and `load_raft_state` with various combinations of term and voted_for (including `None`).

<details>
<summary>Hint</summary>

```rust,ignore
#[test]
fn test_state_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state");

    save_raft_state(&path, 42, Some(3)).unwrap();
    let (term, voted_for) = load_raft_state(&path).unwrap();
    assert_eq!(term, 42);
    assert_eq!(voted_for, Some(3));

    save_raft_state(&path, 99, None).unwrap();
    let (term, voted_for) = load_raft_state(&path).unwrap();
    assert_eq!(term, 99);
    assert_eq!(voted_for, None);
}
```

</details>

---

## Summary

You made your Raft cluster durable:

- **Write-ahead log (WAL)** persists log entries to disk before they are committed
- **CRC32 checksums** detect corruption from partial writes
- **`sync_all()`** guarantees data reaches the physical disk
- **Atomic file writes** (temp + rename) ensure state files are never half-written
- **Recovery** reads the WAL and state file to reconstruct the node on startup
- **Snapshots** compact old log entries to keep the log manageable
- **Rust's ownership** naturally maps to file lifecycle management -- no forgotten closes, no leaked handles

In the next chapter, we connect every piece together. SQL parsing, query planning, execution, MVCC storage, Raft consensus, and networking -- all wired into a single working database server.
