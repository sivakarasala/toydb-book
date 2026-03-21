# Log-Structured Storage — "Append only, ask questions later"

Every database you have built so far updates records in place. You find the record on disk, overwrite it with the new value, and move on. Simple. Obvious. And on a traditional hard drive, catastrophically slow.

Here is the problem: a spinning disk can read or write sequentially at 100+ MB/s. But when you ask it to jump to a random location -- a **seek** -- the disk head has to physically swing across a platter. That takes about 10 milliseconds. Ten milliseconds does not sound like much, until you multiply it by 10,000 writes per second. That is 100 seconds of seek time crammed into every second of real time. Your database cannot keep up.

A log-structured storage engine flips the model. Instead of updating records in place, it **appends every write to the end of a file**. Sequential I/O only. No seeks. On an HDD, that is 100x faster. On an SSD, it is still 10x faster (SSDs are faster at random I/O than HDDs, but sequential writes are still cheaper because they reduce write amplification and wear leveling overhead).

Let's build one from scratch.

---

## The Naive Way: Update in Place

Here is what traditional in-place update looks like, conceptually:

```rust,ignore
// Pseudocode for in-place update storage
fn update(file: &mut File, key: &str, value: &[u8]) {
    let offset = find_record_on_disk(file, key); // random seek #1
    file.seek(offset);                            // random seek #2
    file.write(value);                            // random write
}

fn read(file: &File, key: &str) -> Vec<u8> {
    let offset = find_record_on_disk(file, key);  // random seek
    file.seek(offset);                             // random seek
    file.read_record()                             // random read
}
```

Every read and write involves at least one random seek. With 10,000 operations per second:

```rust
fn main() {
    let ops_per_second = 10_000u64;
    let seek_time_ms = 10u64; // typical HDD seek time
    let total_seek_time_ms = ops_per_second * seek_time_ms;

    println!("Operations per second: {}", ops_per_second);
    println!("Seek time per operation: {}ms", seek_time_ms);
    println!("Total seek time needed: {}ms per second", total_seek_time_ms);
    println!("That is {}x more time than we have!", total_seek_time_ms / 1000);

    // Even on SSD (0.1ms seek):
    let ssd_seek_ms = 0.1f64;
    let ssd_total = ops_per_second as f64 * ssd_seek_ms;
    println!("\nOn SSD: {}ms of seek time per second", ssd_total);
    println!("Manageable, but still {} of our time budget",
             if ssd_total > 500.0 { "over half" } else { "a significant chunk" });
}
```

One hundred seconds of seek time for every second of wall clock time. Your disk physically cannot move the read/write head fast enough. The writes queue up, latency spikes, clients time out.

---

## The Insight

Think about a ship captain's log. The captain does not erase old entries when something changes. When the wind shifts, they do not go back to page 12 and white-out "northeast" and write "southwest." They flip to the next blank page and write: "14:30 -- wind now southwest." The log is append-only. The latest entry for any topic is the truth.

A log-structured storage engine works the same way. Every write -- whether it is a new key or an update to an existing one -- gets appended to the end of the file. To read a key, you need to know where its most recent entry is. That is what the **index** is for: a hash map from key to file offset.

The beauty of this approach:
1. **Writes are always sequential** -- append to the end, done
2. **The index lives in memory** -- lookups are O(1) hash table lookups
3. **Reads are one seek** -- jump to the offset, read the record

This is the core idea behind **Bitcask**, the storage engine used by Riak. It is also the foundation of **LSM trees**, used by LevelDB, RocksDB, Cassandra, and HBase. Let's build the Bitcask variant -- it is simpler and perfectly illustrates the concept.

---

## The Build

### The Record Format

Every record on disk follows a fixed format. We need to know where one record ends and the next begins, so we prefix each record with the lengths of its key and value:

```
[key_len: 4 bytes][value_len: 4 bytes][key: key_len bytes][value: value_len bytes]
```

This is called a **TLV (Type-Length-Value)** encoding, though here we have two lengths instead of a type byte. The fixed-size header (8 bytes) means we can read the lengths first, then read exactly the right number of bytes for the key and value.

### The Implementation

```rust
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// A log-structured key-value store (Bitcask-style).
/// All writes append to a log file. An in-memory index maps
/// keys to their file offsets for O(1) reads.
struct LogStore {
    file: File,
    path: PathBuf,
    index: HashMap<String, u64>, // key -> byte offset of the record
    write_offset: u64,           // current end of file
}

impl LogStore {
    /// Open or create a log store at the given path.
    fn open(path: &Path) -> io::Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)?;

        let write_offset = file.metadata()?.len();

        let mut store = LogStore {
            file,
            path: path.to_path_buf(),
            index: HashMap::new(),
            write_offset,
        };

        // Rebuild the index from the existing log
        if store.write_offset > 0 {
            store.rebuild_index()?;
        }

        Ok(store)
    }

    /// Append a key-value pair to the log. Returns the byte offset
    /// where the record was written.
    fn set(&mut self, key: &str, value: &[u8]) -> io::Result<u64> {
        let offset = self.write_offset;

        // Write the record: [key_len][value_len][key][value]
        let key_bytes = key.as_bytes();
        let key_len = key_bytes.len() as u32;
        let value_len = value.len() as u32;

        // Seek to end (in case someone else wrote)
        self.file.seek(SeekFrom::Start(offset))?;

        self.file.write_all(&key_len.to_le_bytes())?;
        self.file.write_all(&value_len.to_le_bytes())?;
        self.file.write_all(key_bytes)?;
        self.file.write_all(value)?;
        self.file.flush()?;

        // Update the in-memory index
        self.index.insert(key.to_string(), offset);

        // Advance the write offset
        self.write_offset = offset + 4 + 4 + key_len as u64 + value_len as u64;

        Ok(offset)
    }

    /// Read the value for a key. Returns None if the key does not exist.
    fn get(&mut self, key: &str) -> io::Result<Option<Vec<u8>>> {
        let offset = match self.index.get(key) {
            Some(&off) => off,
            None => return Ok(None),
        };

        self.read_value_at(offset)
    }

    /// Read the value from a record at the given byte offset.
    fn read_value_at(&mut self, offset: u64) -> io::Result<Option<Vec<u8>>> {
        self.file.seek(SeekFrom::Start(offset))?;

        // Read the header
        let mut header = [0u8; 8];
        if self.file.read_exact(&mut header).is_err() {
            return Ok(None);
        }

        let key_len = u32::from_le_bytes([header[0], header[1], header[2], header[3]]) as usize;
        let value_len = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;

        // Skip the key
        self.file.seek(SeekFrom::Current(key_len as i64))?;

        // Read the value
        let mut value = vec![0u8; value_len];
        self.file.read_exact(&mut value)?;

        Ok(Some(value))
    }

    /// Delete a key by appending a tombstone (zero-length value).
    fn delete(&mut self, key: &str) -> io::Result<bool> {
        if !self.index.contains_key(key) {
            return Ok(false);
        }

        // Write a tombstone: the key with an empty value
        self.set(key, &[])?;

        // Remove from index so get() returns None
        self.index.remove(key);

        Ok(true)
    }

    /// Rebuild the in-memory index by scanning the entire log file.
    /// This is called on startup to recover the index.
    fn rebuild_index(&mut self) -> io::Result<()> {
        self.index.clear();
        self.file.seek(SeekFrom::Start(0))?;

        let mut offset: u64 = 0;
        let file_len = self.write_offset;

        while offset < file_len {
            self.file.seek(SeekFrom::Start(offset))?;

            // Read the header
            let mut header = [0u8; 8];
            if self.file.read_exact(&mut header).is_err() {
                break;
            }

            let key_len = u32::from_le_bytes([header[0], header[1], header[2], header[3]]) as usize;
            let value_len = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;

            // Read the key
            let mut key_bytes = vec![0u8; key_len];
            self.file.read_exact(&mut key_bytes)?;

            let key = String::from_utf8_lossy(&key_bytes).to_string();

            // If value_len is 0, this is a tombstone -- remove from index
            if value_len == 0 {
                self.index.remove(&key);
            } else {
                self.index.insert(key, offset);
            }

            // Advance to the next record
            offset += 4 + 4 + key_len as u64 + value_len as u64;
        }

        self.write_offset = offset;
        Ok(())
    }

    /// Compact the log: write only the latest value for each live key
    /// to a new file, then replace the old file.
    fn compact(&mut self) -> io::Result<()> {
        let compact_path = self.path.with_extension("compact");

        // Collect current live entries
        let live_entries: Vec<(String, u64)> = self.index.iter()
            .map(|(k, &v)| (k.clone(), v))
            .collect();

        // Write live entries to the new file
        {
            let mut compact_file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(true)
                .open(&compact_path)?;

            let mut new_index = HashMap::new();
            let mut new_offset: u64 = 0;

            for (key, old_offset) in &live_entries {
                // Read the value from the old file
                let value = self.read_value_at(*old_offset)?
                    .unwrap_or_default();

                // Write to the new file
                let key_bytes = key.as_bytes();
                let key_len = key_bytes.len() as u32;
                let value_len = value.len() as u32;

                compact_file.write_all(&key_len.to_le_bytes())?;
                compact_file.write_all(&value_len.to_le_bytes())?;
                compact_file.write_all(key_bytes)?;
                compact_file.write_all(&value)?;

                new_index.insert(key.clone(), new_offset);
                new_offset += 4 + 4 + key_len as u64 + value_len as u64;
            }

            compact_file.flush()?;
        }

        // Replace old file with compacted file
        fs::rename(&compact_path, &self.path)?;

        // Reopen the file and update state
        self.file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&self.path)?;

        self.write_offset = self.file.metadata()?.len();

        // Rebuild the index from the compacted file to ensure consistency
        self.rebuild_index()?;

        Ok(())
    }

    /// Return the number of live keys.
    fn len(&self) -> usize {
        self.index.len()
    }

    /// Return the total size of the log file in bytes.
    fn file_size(&self) -> u64 {
        self.write_offset
    }
}

fn main() -> io::Result<()> {
    let path = Path::new("/tmp/toydb_log_demo.db");

    // Clean up from any previous run
    let _ = fs::remove_file(path);

    // Create a new log store
    let mut store = LogStore::open(path)?;

    // Write some data
    println!("--- Writing data ---");
    for i in 0..1000u32 {
        let key = format!("user:{}", i);
        let value = format!("{{\"name\": \"User {}\", \"score\": {}}}", i, i * 10);
        store.set(&key, value.as_bytes())?;
    }
    println!("Wrote 1,000 records");
    println!("File size: {} bytes", store.file_size());
    println!("Live keys: {}", store.len());

    // Read some data
    println!("\n--- Reading data ---");
    if let Some(value) = store.get("user:42")? {
        println!("user:42 -> {}", String::from_utf8_lossy(&value));
    }

    // Update a key (appends a new record, does NOT overwrite)
    println!("\n--- Updating user:42 ---");
    store.set("user:42", b"{\"name\": \"User 42\", \"score\": 9999}")?;
    println!("File size after update: {} bytes", store.file_size());
    println!("(File grew -- old record still exists on disk)");

    if let Some(value) = store.get("user:42")? {
        println!("user:42 -> {}", String::from_utf8_lossy(&value));
    }

    // Update ALL 1000 keys to simulate heavy write load
    println!("\n--- Heavy updates (all 1000 keys updated 3x) ---");
    for round in 1..=3 {
        for i in 0..1000u32 {
            let key = format!("user:{}", i);
            let value = format!("{{\"round\": {}, \"id\": {}}}", round, i);
            store.set(&key, value.as_bytes())?;
        }
    }
    println!("File size after 3 update rounds: {} bytes", store.file_size());
    println!("Live keys: {} (still 1,000 -- same keys, newer values)", store.len());
    println!("The file has ~4,000 records but only 1,000 are live.");
    println!("That means ~75% of the file is dead data.");

    // Compact!
    println!("\n--- Compacting ---");
    let size_before = store.file_size();
    store.compact()?;
    let size_after = store.file_size();
    println!("File size before compaction: {} bytes", size_before);
    println!("File size after compaction:  {} bytes", size_after);
    println!("Saved {} bytes ({:.0}% reduction)",
             size_before - size_after,
             (1.0 - size_after as f64 / size_before as f64) * 100.0);

    // Verify data is still correct after compaction
    if let Some(value) = store.get("user:42")? {
        println!("\nuser:42 after compaction -> {}", String::from_utf8_lossy(&value));
    }

    // Test delete
    println!("\n--- Deleting user:42 ---");
    store.delete("user:42")?;
    println!("user:42 after delete: {:?}", store.get("user:42")?);

    // Test crash recovery (close and reopen)
    println!("\n--- Simulating crash recovery ---");
    drop(store);
    let mut recovered = LogStore::open(path)?;
    println!("Recovered {} live keys from log", recovered.len());
    println!("user:0 -> {:?}",
             recovered.get("user:0")?.map(|v| String::from_utf8_lossy(&v).to_string()));
    println!("user:42 -> {:?} (was deleted)", recovered.get("user:42")?);

    // Clean up
    let _ = fs::remove_file(path);

    Ok(())
}
```

Let's walk through the key ideas.

### Append-Only Writes

Every call to `set()` appends a new record at the end of the file. It never seeks backward. It never overwrites existing data. This means:

- **Writes are always sequential** -- the disk head never moves
- **Old values are not destroyed** -- they are just orphaned (the index no longer points to them)
- **Crash safety is simpler** -- if the process crashes mid-write, the incomplete record at the end can be detected and discarded on recovery

The write path is dead simple: serialize the record, append it, update the index. Three steps, no seeking.

### The In-Memory Index

The index is a hash map from key to byte offset. When a client asks for `user:42`, we look up the offset (O(1)), seek to that position (one seek), and read the record. One seek per read, guaranteed.

The trade-off: the index must fit in memory. Every key is stored in RAM, along with its 8-byte offset. For a million keys averaging 20 bytes each, that is roughly 28 MB of RAM for the index. For ten million keys, 280 MB. At some point, you need a different design (like an LSM tree with on-disk indexes). But for many workloads, the in-memory index is perfectly practical.

### Rebuild on Startup

When the database restarts, the index is gone -- it was only in memory. So we scan the entire log file from beginning to end, reading each record's key and offset. For a 1 GB log, this takes a few seconds. For a 100 GB log, it takes minutes. Real implementations speed this up with **hint files** (a separate file that stores just the key-offset pairs, much smaller and faster to read).

### Compaction: Taking Out the Garbage

The append-only design has an obvious problem: the file grows forever. If you update `user:42` a thousand times, the file has a thousand records for that key, but only the last one matters. The other 999 are dead weight.

Compaction solves this. We create a new file, write only the latest value for each live key, and replace the old file. The result is a smaller file with no dead records. All the old, superseded values are gone.

When to compact? Common strategies:
- When the file exceeds a size threshold
- When the ratio of dead records to live records exceeds a threshold (e.g., 50% dead)
- On a periodic schedule (e.g., every hour)

### Tombstones: How Deletes Work

You cannot "delete" from an append-only file. Instead, you append a **tombstone** -- a record with an empty value that marks the key as deleted. The index removes the key, so `get()` returns `None`. During compaction, tombstoned keys are omitted from the new file.

---

## The Payoff

Let's quantify the improvement. Here is a comparison of random I/O versus sequential I/O for 10,000 write operations:

```rust
fn main() {
    println!("=== Write Performance Comparison ===\n");

    // In-place update (random I/O)
    let ops = 10_000u64;

    // HDD numbers
    let hdd_seek_ms = 10.0f64;
    let hdd_sequential_mb_s = 100.0;
    let record_size_bytes = 100u64; // average record size

    let inplace_hdd_time_ms = ops as f64 * hdd_seek_ms;
    let append_hdd_data_mb = (ops * record_size_bytes) as f64 / 1_000_000.0;
    let append_hdd_time_ms = (append_hdd_data_mb / hdd_sequential_mb_s) * 1000.0;

    println!("--- HDD (10ms seek, 100 MB/s sequential) ---");
    println!("In-place update: {:.0}ms ({:.1}s)", inplace_hdd_time_ms, inplace_hdd_time_ms / 1000.0);
    println!("Append-only:     {:.1}ms", append_hdd_time_ms);
    println!("Speedup:         {:.0}x\n", inplace_hdd_time_ms / append_hdd_time_ms);

    // SSD numbers
    let ssd_seek_ms = 0.1;
    let ssd_sequential_mb_s = 500.0;

    let inplace_ssd_time_ms = ops as f64 * ssd_seek_ms;
    let append_ssd_time_ms = (append_hdd_data_mb / ssd_sequential_mb_s) * 1000.0;

    println!("--- SSD (0.1ms seek, 500 MB/s sequential) ---");
    println!("In-place update: {:.0}ms ({:.1}s)", inplace_ssd_time_ms, inplace_ssd_time_ms / 1000.0);
    println!("Append-only:     {:.2}ms", append_ssd_time_ms);
    println!("Speedup:         {:.0}x\n", inplace_ssd_time_ms / append_ssd_time_ms);

    // Read performance (one seek either way)
    println!("--- Read Performance ---");
    println!("In-place: one seek to find + read");
    println!("Log-structured: index lookup (O(1) in RAM) + one seek + read");
    println!("Read performance is comparable; the win is on writes.");
}
```

On an HDD, the append-only engine is roughly **100x faster** for writes. On an SSD, it is still about **5-10x faster**. The speedup comes entirely from eliminating random seeks.

The read path is roughly equivalent -- both approaches need one disk seek to read a record. The log-structured engine has a slight advantage because the index lookup is in RAM (O(1) hash lookup) versus potentially needing a disk-based index scan.

---

## Complexity Table

| Operation | In-Place Update | Log-Structured |
|-----------|----------------|----------------|
| Write | O(1) but 1 random seek | O(1), sequential append |
| Read | O(1) + 1 random seek | O(1) hash lookup + 1 seek |
| Delete | O(1) + 1 random seek | O(1), append tombstone |
| Startup recovery | Immediate | O(n) log scan |
| Space efficiency | No waste | Dead records until compaction |
| Compaction | Not needed | O(n) periodic |
| Crash safety | Complex (partial overwrite) | Simple (incomplete append) |

The trade-offs are clear:
- **Writes**: log-structured wins decisively
- **Reads**: roughly tied (one seek either way)
- **Space**: in-place wins (no dead records)
- **Recovery**: in-place wins (no index rebuild)
- **Crash safety**: log-structured wins (append is atomic at the OS level)

---

## Where This Shows Up in Our Database

In Chapter 3, we build `LogStorage` -- a Bitcask-style log-structured engine:

```rust,ignore
pub struct LogStorage {
    log: File,
    index: HashMap<String, u64>,
    // ...
}

impl Storage for LogStorage {
    fn get(&mut self, key: &str) -> Result<Option<Vec<u8>>> {
        // O(1) index lookup, then one disk read
    }

    fn set(&mut self, key: &str, value: &[u8]) -> Result<()> {
        // Append to log, update index
    }
}
```

This is the same design we just built. The chapter adds proper error handling, CRC checksums for data integrity, and a hint file for faster startup. But the core algorithm -- append, index, compact -- is identical.

Beyond our toydb, log-structured storage is the foundation of some of the most important databases in production:

- **Bitcask** (Riak's default engine) -- exactly what we built. Keys in RAM, values on disk, append-only log with compaction.
- **LevelDB / RocksDB** (Google / Facebook) -- extend the idea with **LSM trees**: multiple sorted levels of immutable files, merged during compaction. Used by CockroachDB, TiKV, and many others.
- **Cassandra** -- uses LSM trees for its storage engine. Every write goes to a commit log (append-only) and a memtable (in-memory sorted buffer). Memtables flush to immutable SSTables on disk.
- **Kafka** -- the message broker itself is a log-structured storage system. Topics are append-only logs. Consumers track their position (offset) in the log.
- **Write-ahead logs (WAL)** -- nearly every database (PostgreSQL, MySQL, SQLite) uses an append-only log for crash recovery, even if the primary storage uses in-place updates. The WAL is the safety net.

The log is not just a storage engine. It is a fundamental abstraction in distributed systems. As Jay Kreps (creator of Kafka) wrote: "The log is perhaps the simplest possible storage abstraction. It is an append-only, totally-ordered sequence of records ordered by time."

---

## Try It Yourself

### Exercise 1: CRC Checksums

Add a CRC32 checksum to each record to detect data corruption. The new format: `[crc32: 4 bytes][key_len: 4 bytes][value_len: 4 bytes][key][value]`. Compute the CRC over the key_len + value_len + key + value bytes. On read, verify the checksum and return an error if it does not match.

<details>
<summary>Solution</summary>

```rust
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;

/// Simple CRC32 implementation (no external crate needed).
fn crc32(data: &[u8]) -> u32 {
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
    !crc
}

// Record format: [crc32: 4][key_len: 4][value_len: 4][key][value]
// CRC covers: key_len + value_len + key + value

struct LogStore {
    file: File,
    index: HashMap<String, u64>,
    write_offset: u64,
}

impl LogStore {
    fn open(path: &Path) -> io::Result<Self> {
        let file = OpenOptions::new()
            .read(true).write(true).create(true).open(path)?;
        let write_offset = file.metadata()?.len();
        let mut store = LogStore { file, index: HashMap::new(), write_offset };
        if store.write_offset > 0 { store.rebuild_index()?; }
        Ok(store)
    }

    fn set(&mut self, key: &str, value: &[u8]) -> io::Result<u64> {
        let offset = self.write_offset;
        let key_bytes = key.as_bytes();
        let key_len = key_bytes.len() as u32;
        let value_len = value.len() as u32;

        // Build the payload that the CRC covers
        let mut payload = Vec::new();
        payload.extend_from_slice(&key_len.to_le_bytes());
        payload.extend_from_slice(&value_len.to_le_bytes());
        payload.extend_from_slice(key_bytes);
        payload.extend_from_slice(value);

        let checksum = crc32(&payload);

        self.file.seek(SeekFrom::Start(offset))?;
        self.file.write_all(&checksum.to_le_bytes())?; // 4 bytes CRC
        self.file.write_all(&payload)?;                 // key_len + value_len + key + value
        self.file.flush()?;

        self.index.insert(key.to_string(), offset);
        self.write_offset = offset + 4 + payload.len() as u64;
        Ok(offset)
    }

    fn get(&mut self, key: &str) -> io::Result<Option<Vec<u8>>> {
        let offset = match self.index.get(key) {
            Some(&off) => off,
            None => return Ok(None),
        };
        self.read_at(offset)
    }

    fn read_at(&mut self, offset: u64) -> io::Result<Option<Vec<u8>>> {
        self.file.seek(SeekFrom::Start(offset))?;

        // Read CRC + header (4 + 4 + 4 = 12 bytes)
        let mut header = [0u8; 12];
        if self.file.read_exact(&mut header).is_err() {
            return Ok(None);
        }

        let stored_crc = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
        let key_len = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;
        let value_len = u32::from_le_bytes([header[8], header[9], header[10], header[11]]) as usize;

        let mut key_bytes = vec![0u8; key_len];
        self.file.read_exact(&mut key_bytes)?;

        let mut value = vec![0u8; value_len];
        self.file.read_exact(&mut value)?;

        // Verify CRC
        let mut payload = Vec::new();
        payload.extend_from_slice(&(key_len as u32).to_le_bytes());
        payload.extend_from_slice(&(value_len as u32).to_le_bytes());
        payload.extend_from_slice(&key_bytes);
        payload.extend_from_slice(&value);

        let computed_crc = crc32(&payload);
        if computed_crc != stored_crc {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("CRC mismatch at offset {}: stored={:#x}, computed={:#x}",
                        offset, stored_crc, computed_crc),
            ));
        }

        Ok(Some(value))
    }

    fn rebuild_index(&mut self) -> io::Result<()> {
        self.index.clear();
        self.file.seek(SeekFrom::Start(0))?;
        let mut offset: u64 = 0;
        let file_len = self.write_offset;

        while offset < file_len {
            self.file.seek(SeekFrom::Start(offset))?;
            let mut header = [0u8; 12];
            if self.file.read_exact(&mut header).is_err() { break; }

            let key_len = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;
            let value_len = u32::from_le_bytes([header[8], header[9], header[10], header[11]]) as usize;

            let mut key_bytes = vec![0u8; key_len];
            self.file.read_exact(&mut key_bytes)?;

            let key = String::from_utf8_lossy(&key_bytes).to_string();
            if value_len == 0 {
                self.index.remove(&key);
            } else {
                self.index.insert(key, offset);
            }

            // 4 (crc) + 4 (key_len) + 4 (value_len) + key + value
            offset += 12 + key_len as u64 + value_len as u64;
        }

        self.write_offset = offset;
        Ok(())
    }
}

fn main() -> io::Result<()> {
    let path = Path::new("/tmp/toydb_crc_demo.db");
    let _ = fs::remove_file(path);

    let mut store = LogStore::open(path)?;
    store.set("hello", b"world")?;
    store.set("foo", b"bar")?;

    println!("hello -> {:?}", store.get("hello")?.map(|v| String::from_utf8_lossy(&v).to_string()));
    println!("foo -> {:?}", store.get("foo")?.map(|v| String::from_utf8_lossy(&v).to_string()));

    // Corrupt the file manually and try to read
    drop(store);
    {
        let mut f = OpenOptions::new().write(true).open(path)?;
        f.seek(SeekFrom::Start(20))?; // seek into the middle of a record
        f.write_all(b"CORRUPT")?;     // overwrite some bytes
    }

    let mut store2 = LogStore::open(path)?;
    match store2.get("hello") {
        Ok(val) => println!("hello after corruption: {:?}", val),
        Err(e) => println!("Detected corruption: {}", e),
    }

    let _ = fs::remove_file(path);
    Ok(())
}
```

</details>

### Exercise 2: Hint File for Fast Recovery

The `rebuild_index()` method scans the entire log file, reading every key. For a 10 GB log, this could take minutes. Implement a hint file that stores just `(key, offset)` pairs in a compact format. Write the hint file during compaction, and read it during startup instead of scanning the full log.

<details>
<summary>Solution</summary>

```rust
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

struct LogStore {
    file: File,
    path: PathBuf,
    index: HashMap<String, u64>,
    write_offset: u64,
}

impl LogStore {
    fn hint_path(log_path: &Path) -> PathBuf {
        log_path.with_extension("hint")
    }

    fn open(path: &Path) -> io::Result<Self> {
        let file = OpenOptions::new()
            .read(true).write(true).create(true).open(path)?;
        let write_offset = file.metadata()?.len();
        let mut store = LogStore {
            file, path: path.to_path_buf(),
            index: HashMap::new(), write_offset,
        };

        if store.write_offset > 0 {
            let hint_path = Self::hint_path(path);
            if hint_path.exists() {
                println!("Loading index from hint file (fast path)...");
                store.load_hint_file(&hint_path)?;
            } else {
                println!("No hint file found, scanning full log (slow path)...");
                store.rebuild_index()?;
            }
        }

        Ok(store)
    }

    fn set(&mut self, key: &str, value: &[u8]) -> io::Result<u64> {
        let offset = self.write_offset;
        let key_bytes = key.as_bytes();
        let key_len = key_bytes.len() as u32;
        let value_len = value.len() as u32;

        self.file.seek(SeekFrom::Start(offset))?;
        self.file.write_all(&key_len.to_le_bytes())?;
        self.file.write_all(&value_len.to_le_bytes())?;
        self.file.write_all(key_bytes)?;
        self.file.write_all(value)?;
        self.file.flush()?;

        self.index.insert(key.to_string(), offset);
        self.write_offset = offset + 4 + 4 + key_len as u64 + value_len as u64;
        Ok(offset)
    }

    fn get(&mut self, key: &str) -> io::Result<Option<Vec<u8>>> {
        let offset = match self.index.get(key) { Some(&o) => o, None => return Ok(None) };
        self.file.seek(SeekFrom::Start(offset))?;
        let mut header = [0u8; 8];
        self.file.read_exact(&mut header)?;
        let key_len = u32::from_le_bytes([header[0], header[1], header[2], header[3]]) as usize;
        let value_len = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;
        self.file.seek(SeekFrom::Current(key_len as i64))?;
        let mut value = vec![0u8; value_len];
        self.file.read_exact(&mut value)?;
        Ok(Some(value))
    }

    fn rebuild_index(&mut self) -> io::Result<()> {
        self.index.clear();
        let mut offset: u64 = 0;
        while offset < self.write_offset {
            self.file.seek(SeekFrom::Start(offset))?;
            let mut header = [0u8; 8];
            if self.file.read_exact(&mut header).is_err() { break; }
            let key_len = u32::from_le_bytes([header[0], header[1], header[2], header[3]]) as usize;
            let value_len = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;
            let mut key_bytes = vec![0u8; key_len];
            self.file.read_exact(&mut key_bytes)?;
            let key = String::from_utf8_lossy(&key_bytes).to_string();
            if value_len == 0 { self.index.remove(&key); }
            else { self.index.insert(key, offset); }
            offset += 4 + 4 + key_len as u64 + value_len as u64;
        }
        Ok(())
    }

    /// Write a hint file: one line per live key, format "offset key\n"
    fn write_hint_file(&self) -> io::Result<()> {
        let hint_path = Self::hint_path(&self.path);
        let mut hint_file = OpenOptions::new()
            .write(true).create(true).truncate(true).open(&hint_path)?;

        for (key, offset) in &self.index {
            writeln!(hint_file, "{} {}", offset, key)?;
        }
        hint_file.flush()?;
        Ok(())
    }

    /// Load the index from a hint file instead of scanning the log.
    fn load_hint_file(&mut self, hint_path: &Path) -> io::Result<()> {
        self.index.clear();
        let file = File::open(hint_path)?;
        let reader = BufReader::new(file);

        for line in reader.lines() {
            let line = line?;
            if let Some(space_pos) = line.find(' ') {
                let offset: u64 = line[..space_pos].parse().map_err(|e| {
                    io::Error::new(io::ErrorKind::InvalidData, format!("bad offset: {}", e))
                })?;
                let key = line[space_pos + 1..].to_string();
                self.index.insert(key, offset);
            }
        }
        Ok(())
    }

    /// Compact the log and write a hint file.
    fn compact(&mut self) -> io::Result<()> {
        let compact_path = self.path.with_extension("compact");
        let entries: Vec<(String, u64)> = self.index.iter()
            .map(|(k, &v)| (k.clone(), v)).collect();

        {
            let mut cf = OpenOptions::new()
                .read(true).write(true).create(true).truncate(true)
                .open(&compact_path)?;
            let mut new_offset: u64 = 0;
            let mut new_index = HashMap::new();

            for (key, old_offset) in &entries {
                let value = {
                    self.file.seek(SeekFrom::Start(*old_offset))?;
                    let mut header = [0u8; 8];
                    self.file.read_exact(&mut header)?;
                    let kl = u32::from_le_bytes([header[0], header[1], header[2], header[3]]) as usize;
                    let vl = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;
                    self.file.seek(SeekFrom::Current(kl as i64))?;
                    let mut v = vec![0u8; vl];
                    self.file.read_exact(&mut v)?;
                    v
                };

                let kb = key.as_bytes();
                let kl = kb.len() as u32;
                let vl = value.len() as u32;
                cf.write_all(&kl.to_le_bytes())?;
                cf.write_all(&vl.to_le_bytes())?;
                cf.write_all(kb)?;
                cf.write_all(&value)?;
                new_index.insert(key.clone(), new_offset);
                new_offset += 4 + 4 + kl as u64 + vl as u64;
            }
            cf.flush()?;
            self.index = new_index;
        }

        fs::rename(&compact_path, &self.path)?;
        self.file = OpenOptions::new().read(true).write(true).open(&self.path)?;
        self.write_offset = self.file.metadata()?.len();

        // Write the hint file after compaction
        self.write_hint_file()?;
        println!("Hint file written with {} entries", self.index.len());

        Ok(())
    }
}

fn main() -> io::Result<()> {
    let path = Path::new("/tmp/toydb_hint_demo.db");
    let _ = fs::remove_file(path);
    let _ = fs::remove_file(path.with_extension("hint"));

    // First run: no hint file
    let mut store = LogStore::open(path)?;
    for i in 0..500u32 {
        store.set(&format!("key:{}", i), format!("value-{}", i).as_bytes())?;
    }
    store.compact()?;
    drop(store);

    // Second run: hint file exists, fast startup
    let mut store2 = LogStore::open(path)?;
    println!("Recovered {} keys", store2.index.len());
    println!("key:42 -> {:?}", store2.get("key:42")?.map(|v| String::from_utf8_lossy(&v).to_string()));

    let _ = fs::remove_file(path);
    let _ = fs::remove_file(path.with_extension("hint"));
    Ok(())
}
```

</details>

### Exercise 3: Size-Triggered Compaction

Add automatic compaction that triggers when the ratio of file size to live data size exceeds 2.0 (meaning more than half the file is dead data). Track the total size of live records and trigger compaction inside `set()` when the threshold is crossed.

<details>
<summary>Solution</summary>

```rust
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

struct LogStore {
    file: File,
    path: PathBuf,
    index: HashMap<String, u64>,
    write_offset: u64,
    live_data_size: u64, // total bytes of live records only
    compaction_threshold: f64, // trigger when file_size / live_data_size > this
}

impl LogStore {
    fn open(path: &Path) -> io::Result<Self> {
        let file = OpenOptions::new()
            .read(true).write(true).create(true).open(path)?;
        let write_offset = file.metadata()?.len();
        let mut store = LogStore {
            file, path: path.to_path_buf(),
            index: HashMap::new(), write_offset,
            live_data_size: 0,
            compaction_threshold: 2.0,
        };
        if store.write_offset > 0 { store.rebuild_index()?; }
        Ok(store)
    }

    fn record_size(key: &str, value: &[u8]) -> u64 {
        4 + 4 + key.len() as u64 + value.len() as u64
    }

    fn should_compact(&self) -> bool {
        if self.live_data_size == 0 { return false; }
        let ratio = self.write_offset as f64 / self.live_data_size as f64;
        ratio > self.compaction_threshold
    }

    fn set(&mut self, key: &str, value: &[u8]) -> io::Result<u64> {
        let offset = self.write_offset;
        let key_bytes = key.as_bytes();
        let key_len = key_bytes.len() as u32;
        let value_len = value.len() as u32;
        let rec_size = Self::record_size(key, value);

        self.file.seek(SeekFrom::Start(offset))?;
        self.file.write_all(&key_len.to_le_bytes())?;
        self.file.write_all(&value_len.to_le_bytes())?;
        self.file.write_all(key_bytes)?;
        self.file.write_all(value)?;
        self.file.flush()?;

        // If this key already existed, subtract the old record's contribution
        if let Some(&old_offset) = self.index.get(key) {
            // Read old record size
            if let Ok(old_size) = self.read_record_size(old_offset) {
                self.live_data_size = self.live_data_size.saturating_sub(old_size);
            }
        }

        self.index.insert(key.to_string(), offset);
        self.write_offset = offset + rec_size;
        self.live_data_size += rec_size;

        // Check if compaction is needed
        if self.should_compact() {
            let ratio = self.write_offset as f64 / self.live_data_size as f64;
            println!("[auto-compact] ratio={:.1}x, file={}B, live={}B",
                     ratio, self.write_offset, self.live_data_size);
            self.compact()?;
        }

        Ok(offset)
    }

    fn read_record_size(&mut self, offset: u64) -> io::Result<u64> {
        self.file.seek(SeekFrom::Start(offset))?;
        let mut header = [0u8; 8];
        self.file.read_exact(&mut header)?;
        let key_len = u32::from_le_bytes([header[0], header[1], header[2], header[3]]) as u64;
        let value_len = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as u64;
        Ok(4 + 4 + key_len + value_len)
    }

    fn get(&mut self, key: &str) -> io::Result<Option<Vec<u8>>> {
        let offset = match self.index.get(key) { Some(&o) => o, None => return Ok(None) };
        self.file.seek(SeekFrom::Start(offset))?;
        let mut header = [0u8; 8];
        self.file.read_exact(&mut header)?;
        let kl = u32::from_le_bytes([header[0], header[1], header[2], header[3]]) as usize;
        let vl = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;
        self.file.seek(SeekFrom::Current(kl as i64))?;
        let mut v = vec![0u8; vl];
        self.file.read_exact(&mut v)?;
        Ok(Some(v))
    }

    fn rebuild_index(&mut self) -> io::Result<()> {
        self.index.clear();
        self.live_data_size = 0;
        let mut offset: u64 = 0;
        while offset < self.write_offset {
            self.file.seek(SeekFrom::Start(offset))?;
            let mut header = [0u8; 8];
            if self.file.read_exact(&mut header).is_err() { break; }
            let kl = u32::from_le_bytes([header[0], header[1], header[2], header[3]]) as usize;
            let vl = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;
            let rec_size = 4 + 4 + kl as u64 + vl as u64;

            let mut kb = vec![0u8; kl];
            self.file.read_exact(&mut kb)?;
            let key = String::from_utf8_lossy(&kb).to_string();

            if let Some(&old_off) = self.index.get(&key) {
                if let Ok(old_size) = self.read_record_size(old_off) {
                    self.live_data_size = self.live_data_size.saturating_sub(old_size);
                }
            }

            if vl == 0 {
                self.index.remove(&key);
            } else {
                self.index.insert(key, offset);
                self.live_data_size += rec_size;
            }
            offset += rec_size;
        }
        Ok(())
    }

    fn compact(&mut self) -> io::Result<()> {
        let compact_path = self.path.with_extension("compact");
        let entries: Vec<(String, u64)> = self.index.iter()
            .map(|(k, &v)| (k.clone(), v)).collect();

        let mut new_live_size: u64 = 0;
        {
            let mut cf = OpenOptions::new()
                .read(true).write(true).create(true).truncate(true)
                .open(&compact_path)?;
            let mut new_index = HashMap::new();
            let mut new_offset: u64 = 0;

            for (key, old_offset) in &entries {
                self.file.seek(SeekFrom::Start(*old_offset))?;
                let mut header = [0u8; 8];
                self.file.read_exact(&mut header)?;
                let kl = u32::from_le_bytes([header[0], header[1], header[2], header[3]]) as usize;
                let vl = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;
                self.file.seek(SeekFrom::Current(kl as i64))?;
                let mut v = vec![0u8; vl];
                self.file.read_exact(&mut v)?;

                let kb = key.as_bytes();
                let rec_size = 4 + 4 + kb.len() as u64 + v.len() as u64;
                cf.write_all(&(kb.len() as u32).to_le_bytes())?;
                cf.write_all(&(v.len() as u32).to_le_bytes())?;
                cf.write_all(kb)?;
                cf.write_all(&v)?;
                new_index.insert(key.clone(), new_offset);
                new_offset += rec_size;
                new_live_size += rec_size;
            }
            cf.flush()?;
            self.index = new_index;
        }

        fs::rename(&compact_path, &self.path)?;
        self.file = OpenOptions::new().read(true).write(true).open(&self.path)?;
        self.write_offset = self.file.metadata()?.len();
        self.live_data_size = new_live_size;
        Ok(())
    }
}

fn main() -> io::Result<()> {
    let path = Path::new("/tmp/toydb_autocompact_demo.db");
    let _ = fs::remove_file(path);

    let mut store = LogStore::open(path)?;

    // Write 100 keys, then update them 5 times each
    // This should trigger auto-compaction
    println!("Writing 100 keys...");
    for i in 0..100u32 {
        store.set(&format!("k:{}", i), format!("v0-{}", i).as_bytes())?;
    }
    println!("File size: {}, live data: {}\n", store.write_offset, store.live_data_size);

    for round in 1..=5 {
        println!("Update round {}...", round);
        for i in 0..100u32 {
            store.set(&format!("k:{}", i), format!("v{}-{}", round, i).as_bytes())?;
        }
        println!("  File size: {}, live data: {}, ratio: {:.1}x",
                 store.write_offset, store.live_data_size,
                 store.write_offset as f64 / store.live_data_size as f64);
    }

    // Verify data
    println!("\nk:42 -> {:?}", store.get("k:42")?.map(|v| String::from_utf8_lossy(&v).to_string()));
    println!("Final: {} keys, file size: {}", store.index.len(), store.write_offset);

    let _ = fs::remove_file(path);
    Ok(())
}
```

</details>

---

## Recap

A log-structured storage engine converts random writes into sequential appends. Every write goes to the end of the file. An in-memory hash map tracks where each key's latest record lives on disk. Reads do one index lookup and one disk seek. Old records accumulate as dead weight until compaction rewrites the file with only live data.

The trade-offs are real: the index must fit in RAM, startup requires scanning the log (unless you have a hint file), and the file grows until compaction runs. But for write-heavy workloads -- which describes most database engines -- the performance gain from sequential I/O is transformative.

This is not a toy idea. Bitcask, LevelDB, RocksDB, Cassandra, and Kafka all build on this foundation. The append-only log is one of the most powerful abstractions in storage engineering. Once you understand it, you understand why these systems make the architectural choices they do.
