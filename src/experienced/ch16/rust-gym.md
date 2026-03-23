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
