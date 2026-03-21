# Chapter 15: Raft -- Log Replication

Your cluster can elect a leader. But a leader that does not replicate data is just a single point of failure with extra steps. The entire purpose of consensus is to keep multiple copies of the data in sync. When a client sends a write to the leader, the leader must store it locally and replicate it to a majority of followers before confirming the write. If the leader crashes after a majority has the data, the next elected leader is guaranteed to have it too. No data is lost.

This chapter implements Raft's log replication protocol. The leader maintains a replicated log — an ordered sequence of commands that every node applies to its state machine in the same order. You will build the `AppendEntries` RPC, implement the consistency check that detects and repairs divergent logs, manage commit indices, and apply committed entries to the database. The spotlight concept is **concurrency with Arc and Mutex** — safely sharing mutable state across async tasks in a distributed system.

By the end of this chapter, you will have:

- A `RaftLog` with term-indexed entries, append, truncation, and consistency checking
- The `AppendEntries` RPC for both heartbeats and log replication
- Leader bookkeeping with `next_index` and `match_index` per follower
- Commitment rules: an entry is committed when replicated to a majority
- State machine application: applying committed entries to the database
- `Arc<Mutex<RaftState>>` for sharing Raft state across async network tasks
- A clear understanding of how Raft guarantees no committed entry is ever lost

---

## Spotlight: Concurrency — Arc & Mutex

Every chapter has one spotlight concept. This chapter's spotlight is **Arc and Mutex** — Rust's primitives for sharing mutable state across threads and async tasks.

### The problem: shared mutable state

A Raft node runs multiple concurrent activities:
1. Receiving client requests (writes to the log)
2. Sending AppendEntries RPCs to followers
3. Receiving AppendEntries responses and updating match indices
4. Applying committed entries to the state machine
5. Handling election timeouts and heartbeat timers

All of these need access to the same Raft state — the log, the commit index, the current term. In a single-threaded program, this is trivial. In a concurrent program, it is a data race waiting to happen.

Most languages solve this at runtime: Go uses goroutines with channels (or `sync.Mutex`), Java uses `synchronized` blocks, Python uses the GIL (which prevents true parallelism). Rust solves it at compile time — if your code compiles, it has no data races. The tools: `Arc` for shared ownership, `Mutex` for mutual exclusion.

### Arc: shared ownership across threads

`Rc<T>` (Reference Counted) lets multiple owners share the same data, but it is not thread-safe — its reference count uses non-atomic operations. `Arc<T>` (Atomically Reference Counted) is the thread-safe version:

```rust,ignore
use std::sync::Arc;

let data = Arc::new(vec![1, 2, 3]);

let data_clone = Arc::clone(&data);  // increment reference count (atomic)
std::thread::spawn(move || {
    println!("Thread sees: {:?}", data_clone);  // shared read access
});

println!("Main sees: {:?}", data);  // same data, different Arc handle
```

`Arc::clone` does not clone the data — it increments an atomic reference counter and returns a new `Arc` pointing to the same allocation. When the last `Arc` is dropped, the data is deallocated. This is similar to `shared_ptr` in C++, but Rust's type system prevents the use-after-free bugs that plague C++ shared pointers.

### Mutex: mutual exclusion

`Arc<T>` gives shared read access, but it does not allow mutation. To mutate shared data, you need `Mutex<T>`:

```rust,ignore
use std::sync::{Arc, Mutex};

let counter = Arc::new(Mutex::new(0));

let handles: Vec<_> = (0..10).map(|_| {
    let counter = Arc::clone(&counter);
    std::thread::spawn(move || {
        let mut value = counter.lock().unwrap();
        *value += 1;
        // MutexGuard dropped here — lock released
    })
}).collect();

for handle in handles {
    handle.join().unwrap();
}

println!("Final: {}", *counter.lock().unwrap());  // always 10
```

`mutex.lock()` returns a `MutexGuard<T>` — a smart pointer that dereferences to the inner data and releases the lock when dropped. The lock is released automatically when the guard goes out of scope. No manual `unlock()` call needed — RAII handles it.

### The critical insight: lock scope

The most common mistake with `Mutex` in async code: holding the lock across `.await` points.

```rust,ignore
// WRONG: lock held across .await
async fn bad(state: Arc<Mutex<RaftState>>) {
    let mut state = state.lock().unwrap();
    state.log.push(entry);
    network_send(&state).await;  // BLOCKS other tasks from accessing state
    state.commit_index += 1;
}

// RIGHT: lock acquired and released in tight scopes
async fn good(state: Arc<Mutex<RaftState>>) {
    // Scope 1: modify the log
    {
        let mut state = state.lock().unwrap();
        state.log.push(entry);
    }
    // Lock released

    network_send_something().await;  // other tasks can access state

    // Scope 2: update commit index
    {
        let mut state = state.lock().unwrap();
        state.commit_index += 1;
    }
    // Lock released
}
```

The pattern: lock, do fast work, unlock, await, lock again. Keep critical sections as short as possible. If the lock is held during a network call, no other task can read or write the Raft state until the network call completes — this destroys concurrency.

### std::sync::Mutex vs tokio::sync::Mutex

Rust has two `Mutex` implementations:

| | `std::sync::Mutex` | `tokio::sync::Mutex` |
|---|---|---|
| Blocking | Blocks the OS thread | Yields to the Tokio runtime |
| Use when | Lock is held briefly (no .await inside) | Lock is held across .await points |
| Performance | Faster (no runtime overhead) | Slower (runtime coordination) |
| Guard type | `MutexGuard<T>` (not Send) | `MutexGuard<T>` (Send) |

For Raft state: use `std::sync::Mutex`. Our critical sections are pure computation — update a vector, increment a counter, compare terms. No I/O, no awaiting. The lock is held for microseconds.

### Memory ordering (brief)

`Arc` uses atomic operations for its reference count. Atomic operations have **memory orderings** that determine how operations on different variables relate to each other:

```rust,ignore
use std::sync::atomic::{AtomicU64, Ordering};

let counter = AtomicU64::new(0);
counter.fetch_add(1, Ordering::Relaxed);  // no ordering guarantees
counter.fetch_add(1, Ordering::SeqCst);   // strongest guarantees
```

For our purposes:
- `Ordering::Relaxed` is sufficient for counters that are read independently
- `Ordering::SeqCst` (sequentially consistent) is the safe default — use it when in doubt
- `Ordering::Acquire`/`Ordering::Release` are for lock-free data structures (advanced)

We use `Mutex` for our Raft state, which handles memory ordering internally. You only need to think about orderings when using raw atomics without a lock.

> **Coming from JS/Python/Go?**
>
> | Concept | JavaScript | Python | Go | Rust |
> |---------|-----------|--------|-----|------|
> | Shared ownership | GC handles it | GC handles it | GC handles it | `Arc<T>` (explicit) |
> | Mutual exclusion | N/A (single-threaded) | `threading.Lock()` | `sync.Mutex` | `std::sync::Mutex` |
> | Lock + unlock | N/A | `with lock:` (context manager) | `mu.Lock()` + `defer mu.Unlock()` | `let guard = mutex.lock()` (auto-release) |
> | Data race prevention | N/A (single-threaded) | GIL (accidental) | Runtime race detector | Compile-time (type system) |
> | Async shared state | N/A (single-threaded) | `asyncio.Lock` | channels or `sync.Mutex` | `Arc<Mutex<T>>` or `Arc<tokio::sync::Mutex<T>>` |
>
> The key Rust difference: data races are compile-time errors, not runtime bugs. If you forget to use `Arc`, the compiler rejects your code. If you forget to lock the `Mutex`, the compiler rejects your code. In Go, you can forget to lock a `sync.Mutex` and the program compiles fine — the data race shows up under load in production (or if you run the race detector). In Rust, the type system makes this category of bug impossible.

---

## Exercise 1: The Raft Log

**Goal:** Implement the replicated log data structure — the ordered sequence of entries that forms the heart of Raft's replication protocol.

### Step 1: Understand what the log does

Every write to the database becomes a **log entry**. The leader appends it to its log, replicates it to followers, and once a majority has it, the entry is **committed** and applied to the state machine (the database).

```
Client: INSERT INTO users VALUES (1, 'Alice')

Leader's log:
  Index: 1  2  3  4  5
  Term:  1  1  1  2  2
  Cmd:   C  I  I  U  I    (C=CREATE, I=INSERT, U=UPDATE)
                    ▲
              commit_index = 4
              (entries 1-4 are committed and applied)
              (entry 5 is pending — waiting for majority ack)
```

The log has three critical properties:
1. **Ordered**: entries have sequential indices starting at 1
2. **Immutable once committed**: a committed entry is never overwritten
3. **Agreement**: if two nodes have an entry at the same index with the same term, the entries are identical (and all preceding entries are identical)

### Step 2: Define the log structure

Add to `src/raft.rs`:

```rust,ignore
/// The replicated log. Each entry contains a command to apply
/// to the state machine, tagged with the term it was received.
#[derive(Debug, Clone)]
pub struct RaftLog {
    /// Log entries (0-indexed internally, but 1-indexed in Raft protocol).
    /// entries[0] corresponds to log index 1.
    entries: Vec<LogEntry>,
}

impl RaftLog {
    /// Create an empty log.
    pub fn new() -> Self {
        RaftLog {
            entries: Vec::new(),
        }
    }

    /// The index of the last entry, or 0 if empty.
    pub fn last_index(&self) -> u64 {
        self.entries.len() as u64
    }

    /// The term of the last entry, or 0 if empty.
    pub fn last_term(&self) -> Term {
        self.entries.last().map(|e| e.term).unwrap_or(0)
    }

    /// Get the entry at the given index (1-based).
    /// Returns None if the index is out of range.
    pub fn get(&self, index: u64) -> Option<&LogEntry> {
        if index == 0 || index as usize > self.entries.len() {
            None
        } else {
            Some(&self.entries[(index - 1) as usize])
        }
    }

    /// Get the term of the entry at the given index.
    /// Returns 0 for index 0 (before the log starts).
    pub fn term_at(&self, index: u64) -> Term {
        if index == 0 {
            0
        } else {
            self.get(index).map(|e| e.term).unwrap_or(0)
        }
    }

    /// Append a new entry to the log.
    pub fn append(&mut self, term: Term, command: Vec<u8>) -> u64 {
        let index = self.last_index() + 1;
        self.entries.push(LogEntry {
            term,
            index,
            command,
        });
        index
    }

    /// Append multiple entries starting at a given index.
    /// If there are existing entries that conflict (same index, different term),
    /// truncate the log from the conflict point and append the new entries.
    /// This is the core of the consistency repair mechanism.
    pub fn append_entries(&mut self, prev_log_index: u64, entries: Vec<LogEntry>) {
        for entry in entries {
            let idx = entry.index;
            if let Some(existing) = self.get(idx) {
                if existing.term != entry.term {
                    // Conflict: truncate from here and append
                    self.entries.truncate((idx - 1) as usize);
                    self.entries.push(entry);
                }
                // Same term: already have this entry, skip
            } else {
                // New entry: append
                self.entries.push(entry);
            }
        }
    }

    /// Check if the log matches at the given index and term.
    /// This is the consistency check in AppendEntries.
    pub fn matches(&self, index: u64, term: Term) -> bool {
        if index == 0 {
            return true; // empty log always matches
        }
        match self.get(index) {
            Some(entry) => entry.term == term,
            None => false, // we do not have this entry
        }
    }

    /// Get entries from start_index to the end (inclusive).
    pub fn entries_from(&self, start_index: u64) -> Vec<LogEntry> {
        if start_index == 0 || start_index as usize > self.entries.len() {
            return Vec::new();
        }
        self.entries[(start_index - 1) as usize..].to_vec()
    }

    /// The number of entries in the log.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the log is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
```

### Step 3: The Log Matching Property

The log matching property is Raft's key safety invariant:

```
LOG MATCHING PROPERTY:
  If two logs contain an entry with the same index and term,
  then:
  1. The entries are identical (same command)
  2. All preceding entries are also identical
```

This property is maintained by the AppendEntries consistency check. Before appending entries, the leader sends `prev_log_index` and `prev_log_term` — the index and term of the entry immediately before the new entries. The follower checks: "Do I have an entry at `prev_log_index` with term `prev_log_term`?" If yes, the logs agree up to that point and the new entries can be safely appended. If no, the follower rejects the request and the leader backs up.

```
Leader's log:
  Index: 1  2  3  4  5
  Term:  1  1  2  2  2

AppendEntries to follower:
  prev_log_index = 3
  prev_log_term  = 2
  entries = [Entry{index:4, term:2}, Entry{index:5, term:2}]

Follower checks: "Do I have entry 3 with term 2?"
  If yes: append entries 4 and 5 ✓
  If no:  reject — leader will retry with prev_log_index = 2
```

### Step 4: Test the log

```rust,ignore
#[test]
fn test_raft_log() {
    let mut log = RaftLog::new();

    // Empty log
    assert_eq!(log.last_index(), 0);
    assert_eq!(log.last_term(), 0);
    assert!(log.matches(0, 0)); // empty matches empty

    // Append entries
    let idx1 = log.append(1, b"SET x 1".to_vec());
    let idx2 = log.append(1, b"SET y 2".to_vec());
    let idx3 = log.append(2, b"SET z 3".to_vec());
    assert_eq!(idx1, 1);
    assert_eq!(idx2, 2);
    assert_eq!(idx3, 3);

    // Check entries
    assert_eq!(log.last_index(), 3);
    assert_eq!(log.last_term(), 2);
    assert_eq!(log.term_at(1), 1);
    assert_eq!(log.term_at(2), 1);
    assert_eq!(log.term_at(3), 2);

    // Consistency check
    assert!(log.matches(1, 1));  // entry 1 has term 1 ✓
    assert!(log.matches(3, 2));  // entry 3 has term 2 ✓
    assert!(!log.matches(3, 1)); // entry 3 does NOT have term 1 ✗
    assert!(!log.matches(5, 1)); // entry 5 does not exist ✗
}

#[test]
fn test_log_conflict_resolution() {
    let mut log = RaftLog::new();
    log.append(1, b"cmd1".to_vec());
    log.append(1, b"cmd2".to_vec());
    log.append(2, b"cmd3".to_vec()); // this will conflict

    // Simulate receiving entries from the leader that conflict at index 3
    let leader_entries = vec![
        LogEntry { term: 1, index: 3, command: b"leader_cmd3".to_vec() },
        LogEntry { term: 1, index: 4, command: b"leader_cmd4".to_vec() },
    ];

    log.append_entries(2, leader_entries);

    // Entry 3 should be replaced (term 2 -> term 1)
    assert_eq!(log.term_at(3), 1);
    assert_eq!(log.last_index(), 4);
    assert_eq!(log.get(3).unwrap().command, b"leader_cmd3");
    assert_eq!(log.get(4).unwrap().command, b"leader_cmd4");
}
```

<details>
<summary>Hint: Why Vec&lt;LogEntry&gt; and not a B-tree or HashMap?</summary>

The log is append-only and accessed sequentially. A `Vec` provides:
- O(1) append (amortized)
- O(1) random access by index (index - 1 = array position)
- Cache-friendly sequential access for replication

A B-tree or HashMap would add overhead without benefit. The only limitation: truncation (removing entries from the end) is O(n) in the worst case because `Vec::truncate` must drop elements. In practice, truncation is rare (only during conflict repair) and the number of truncated entries is small.

For a production implementation, you would store the log on disk (not in memory), using a write-ahead log file with an in-memory index. That is Chapter 16.

</details>

---

## Exercise 2: AppendEntries RPC

**Goal:** Implement the full AppendEntries RPC — the mechanism the leader uses to replicate log entries to followers and maintain its authority through heartbeats.

### Step 1: Add the log to RaftNode

```rust,ignore
pub struct RaftNode {
    // ... existing fields from Chapter 14 ...
    pub id: NodeId,
    pub peers: Vec<NodeId>,
    pub state: NodeState,
    pub current_term: Term,
    pub voted_for: Option<NodeId>,
    pub leader_id: Option<NodeId>,
    pub election_deadline: Instant,
    pub election_timeout: Duration,
    pub votes_received: HashSet<NodeId>,

    // NEW: Log replication fields
    /// The replicated log.
    pub log: RaftLog,
    /// Index of the highest log entry known to be committed.
    /// Entries up to this index have been replicated to a majority
    /// and are safe to apply to the state machine.
    pub commit_index: u64,
    /// Index of the highest log entry applied to the state machine.
    /// Always <= commit_index.
    pub last_applied: u64,

    // Leader-only state (only valid when state == Leader)
    /// For each peer: the next log entry to send.
    /// Initialized to leader's last log index + 1 on election.
    pub next_index: HashMap<NodeId, u64>,
    /// For each peer: the highest log entry known to be replicated.
    /// Initialized to 0 on election.
    pub match_index: HashMap<NodeId, u64>,
}
```

### Step 2: Initialize leader state on election

When a node becomes leader, it initializes `next_index` and `match_index` for each peer:

```rust,ignore
impl RaftNode {
    fn become_leader(&mut self) {
        println!(
            "[Node {}] Won election for term {} with {} votes",
            self.id, self.current_term, self.votes_received.len()
        );

        self.state = NodeState::Leader;
        self.leader_id = Some(self.id);

        // Initialize leader state
        let last_log_index = self.log.last_index();
        for &peer in &self.peers {
            // Optimistic: assume all peers are up-to-date
            // If they are not, the consistency check will detect it
            // and we will back up next_index
            self.next_index.insert(peer, last_log_index + 1);
            self.match_index.insert(peer, 0);
        }

        // Send initial heartbeat to all peers
        // (this also serves as "I am the new leader" announcement)
    }
}
```

The `next_index` starts at `last_log_index + 1` — the optimistic assumption that every peer has the same log as the leader. If a peer is behind, the first AppendEntries will fail the consistency check, and the leader will decrement `next_index` and retry. This back-off mechanism converges quickly: in the worst case, the leader sends `O(log_length)` probes before finding the divergence point.

### Step 3: Implement AppendEntries sending

The leader periodically sends AppendEntries to each follower:

```rust,ignore
impl RaftNode {
    /// Send AppendEntries RPCs to all followers.
    /// Called periodically (e.g., every 50ms for heartbeats)
    /// and immediately after appending a new entry.
    pub fn send_append_entries(&self) -> Vec<(NodeId, RaftMessage)> {
        if self.state != NodeState::Leader {
            return Vec::new();
        }

        let mut messages = Vec::new();

        for &peer in &self.peers {
            let next = self.next_index.get(&peer).copied().unwrap_or(1);
            let prev_log_index = next - 1;
            let prev_log_term = self.log.term_at(prev_log_index);
            let entries = self.log.entries_from(next);

            messages.push((
                peer,
                RaftMessage::AppendEntries {
                    term: self.current_term,
                    leader_id: self.id,
                    prev_log_index,
                    prev_log_term,
                    entries,
                    leader_commit: self.commit_index,
                },
            ));
        }

        messages
    }

    /// Append a new command to the log (called when the leader receives a client request).
    /// Returns the log index of the new entry.
    pub fn client_request(&mut self, command: Vec<u8>) -> Option<u64> {
        if self.state != NodeState::Leader {
            return None; // only the leader can accept client requests
        }

        let index = self.log.append(self.current_term, command);
        println!(
            "[Node {}] Appended entry {} (term {})",
            self.id, index, self.current_term
        );

        Some(index)
    }
}
```

### Step 4: Handle AppendEntries on the follower

This is the most complex part of Raft. The follower must:
1. Reject if the term is stale
2. Check the consistency condition (prev_log_index, prev_log_term)
3. Resolve any conflicts in the log
4. Append new entries
5. Update commit_index

```rust,ignore
impl RaftNode {
    /// Handle an AppendEntries RPC (full implementation).
    fn handle_append_entries_full(
        &mut self,
        from: NodeId,
        term: Term,
        leader_id: NodeId,
        prev_log_index: u64,
        prev_log_term: Term,
        entries: Vec<LogEntry>,
        leader_commit: u64,
    ) -> Vec<(NodeId, RaftMessage)> {
        // Rule 1: Reply false if term < currentTerm
        if term < self.current_term {
            return vec![(
                from,
                RaftMessage::AppendEntriesResponse {
                    term: self.current_term,
                    success: false,
                    match_index: 0,
                },
            )];
        }

        // Valid leader — reset election timer
        self.leader_id = Some(leader_id);
        self.reset_election_timer();

        // Step down if we were a candidate
        if self.state == NodeState::Candidate {
            self.state = NodeState::Follower;
            println!(
                "[Node {}] Stepping down — {} is leader for term {}",
                self.id, leader_id, term
            );
        }

        // Rule 2: Reply false if log does not contain an entry at
        // prevLogIndex whose term matches prevLogTerm
        if !self.log.matches(prev_log_index, prev_log_term) {
            println!(
                "[Node {}] Log mismatch at index {} (expected term {}, have term {})",
                self.id,
                prev_log_index,
                prev_log_term,
                self.log.term_at(prev_log_index)
            );
            return vec![(
                from,
                RaftMessage::AppendEntriesResponse {
                    term: self.current_term,
                    success: false,
                    match_index: self.log.last_index(),
                },
            )];
        }

        // Rules 3 & 4: If an existing entry conflicts with a new one,
        // delete it and all that follow. Append new entries not already in the log.
        if !entries.is_empty() {
            self.log.append_entries(prev_log_index, entries);
            println!(
                "[Node {}] Appended entries up to index {}",
                self.id, self.log.last_index()
            );
        }

        // Rule 5: If leaderCommit > commitIndex,
        // set commitIndex = min(leaderCommit, index of last new entry)
        if leader_commit > self.commit_index {
            self.commit_index = std::cmp::min(leader_commit, self.log.last_index());
            println!(
                "[Node {}] Updated commit_index to {}",
                self.id, self.commit_index
            );
        }

        vec![(
            from,
            RaftMessage::AppendEntriesResponse {
                term: self.current_term,
                success: true,
                match_index: self.log.last_index(),
            },
        )]
    }
}
```

### Step 5: Handle AppendEntries responses on the leader

When the leader receives a response from a follower, it updates its bookkeeping:

```rust,ignore
impl RaftNode {
    /// Handle an AppendEntriesResponse (on the leader).
    fn handle_append_entries_response(
        &mut self,
        from: NodeId,
        term: Term,
        success: bool,
        follower_match_index: u64,
    ) -> Vec<(NodeId, RaftMessage)> {
        if self.state != NodeState::Leader {
            return Vec::new();
        }

        if term > self.current_term {
            // Stale leader — step down (already handled in handle_message)
            return Vec::new();
        }

        if success {
            // Update match_index and next_index for this follower
            self.match_index.insert(from, follower_match_index);
            self.next_index.insert(from, follower_match_index + 1);

            println!(
                "[Node {}] Follower {} matched up to index {}",
                self.id, from, follower_match_index
            );

            // Check if any new entries can be committed
            self.advance_commit_index();
        } else {
            // Follower's log does not match — back up and retry
            let current_next = self.next_index.get(&from).copied().unwrap_or(1);
            if current_next > 1 {
                self.next_index.insert(from, current_next - 1);
                println!(
                    "[Node {}] Backing up next_index for {} to {}",
                    self.id, from, current_next - 1
                );
            }

            // Immediately retry with the backed-up next_index
            let next = self.next_index.get(&from).copied().unwrap_or(1);
            let prev_log_index = next - 1;
            let prev_log_term = self.log.term_at(prev_log_index);
            let entries = self.log.entries_from(next);

            return vec![(
                from,
                RaftMessage::AppendEntries {
                    term: self.current_term,
                    leader_id: self.id,
                    prev_log_index,
                    prev_log_term,
                    entries,
                    leader_commit: self.commit_index,
                },
            )];
        }

        Vec::new()
    }
}
```

### Step 6: The back-off mechanism visualized

When a follower is behind, the leader discovers this through failed consistency checks and backs up:

```
Leader's log:
  Index: 1  2  3  4  5  6
  Term:  1  1  1  2  2  2

Follower's log (behind):
  Index: 1  2  3
  Term:  1  1  1

Replication attempt 1:
  Leader sends: prev_log_index=5, prev_log_term=2, entries=[6]
  Follower: "I don't have index 5" → reject
  Leader backs up: next_index = 5

Replication attempt 2:
  Leader sends: prev_log_index=4, prev_log_term=2, entries=[5,6]
  Follower: "I don't have index 4" → reject
  Leader backs up: next_index = 4

Replication attempt 3:
  Leader sends: prev_log_index=3, prev_log_term=1, entries=[4,5,6]
  Follower: "I have index 3 with term 1" → match! → accept
  Follower appends entries 4, 5, 6
  Leader updates: match_index=6, next_index=7
```

The back-off is linear in the number of missing entries. The Raft paper describes an optimization: the follower can include its log length in the rejection, so the leader can jump directly to the right position instead of backing up one entry at a time.

> **Coming from JS/Python/Go?**
>
> Log replication is conceptually similar to database replication in other stacks:
>
> | Concept | MySQL | PostgreSQL | MongoDB | Raft |
> |---------|-------|-----------|---------|------|
> | Log type | Binlog | WAL | Oplog | Raft log |
> | Replication | Async | Sync or async | Async | Sync (majority) |
> | Consistency | Eventual | Configurable | Eventual | Linearizable |
> | Conflict resolution | Last-write-wins | N/A (single primary) | Last-write-wins | N/A (single leader) |
>
> The key difference: Raft provides **linearizable** consistency. Once a write is committed (acknowledged to the client), every subsequent read will see that write. MySQL and MongoDB offer eventual consistency by default — a read might return stale data from a replica that has not yet received the latest writes.

---

## Exercise 3: Commitment and State Machine Application

**Goal:** Implement the commitment rules (an entry is committed when replicated to a majority) and apply committed entries to the state machine.

### Step 1: Advance the commit index

The leader commits an entry when it knows a majority of nodes have it:

```rust,ignore
impl RaftNode {
    /// Check if any new entries can be committed.
    /// An entry at index N is committed if:
    /// 1. N > commit_index (not already committed)
    /// 2. A majority of nodes have match_index >= N
    /// 3. The entry at index N was created in the current term
    ///    (Raft's "commitment rule" — cannot commit entries from previous terms directly)
    fn advance_commit_index(&mut self) {
        if self.state != NodeState::Leader {
            return;
        }

        // Try each uncommitted entry, starting from the highest
        for n in (self.commit_index + 1..=self.log.last_index()).rev() {
            // Condition 3: only commit entries from the current term
            if self.log.term_at(n) != self.current_term {
                continue;
            }

            // Count how many nodes have this entry
            let mut count = 1; // count self
            for &peer in &self.peers {
                if self.match_index.get(&peer).copied().unwrap_or(0) >= n {
                    count += 1;
                }
            }

            if count >= self.quorum_size() {
                self.commit_index = n;
                println!(
                    "[Node {}] Committed entries up to index {} ({}/{} nodes)",
                    self.id, n, count, self.peers.len() + 1
                );
                break; // committing N implies committing all entries before N
            }
        }
    }
}
```

### Step 2: Understand the commitment restriction

Why condition 3 — "only commit entries from the current term"? This prevents a subtle bug:

```
Scenario (without the restriction):

Time 1: Node 1 is leader (term 1), appends entry A at index 1
         Replicates A to Node 2, but NOT Node 3
         Nodes 1,2 have [A(t1)]
         Node 3 has []

Time 2: Node 1 crashes. Node 3 wins election (term 2).
         Node 3 appends entry B at index 1.
         Replicates B to Node 2 (overwrites A).
         Nodes 2,3 have [B(t2)]

Time 3: Node 3 crashes. Node 1 restarts, wins election (term 3).
         Node 1 still has [A(t1)] — entry A was never committed.
         If Node 1 commits A (from term 1) based on its own copy,
         and A was already overwritten on nodes 2 and 3,
         committed data is lost!

With the restriction: Node 1 (term 3) cannot commit A (term 1) directly.
It must append a new entry in term 3 first. When that new entry
is committed (replicated to majority), all preceding entries
(including A) are also committed — but only because the new entry's
replication also verified the preceding entries' consistency.
```

This is one of the most subtle correctness issues in Raft. The paper's Figure 8 illustrates this scenario in detail.

### Step 3: Apply committed entries to the state machine

```rust,ignore
/// The result of applying entries to the state machine.
pub struct ApplyResult {
    pub index: u64,
    pub result: Vec<u8>,  // the result of the command execution
}

impl RaftNode {
    /// Apply all committed but unapplied entries to the state machine.
    /// Returns the results of applied entries (so the leader can respond
    /// to waiting clients).
    pub fn apply_committed(&mut self) -> Vec<ApplyResult> {
        let mut results = Vec::new();

        while self.last_applied < self.commit_index {
            self.last_applied += 1;

            if let Some(entry) = self.log.get(self.last_applied) {
                println!(
                    "[Node {}] Applying entry {} (term {}) to state machine",
                    self.id, self.last_applied, entry.term
                );

                // In a real implementation, this would execute the command
                // against the database. For now, we just record that it
                // was applied.
                results.push(ApplyResult {
                    index: self.last_applied,
                    result: entry.command.clone(), // echo the command as the result
                });
            }
        }

        results
    }
}
```

### Step 4: Integrate with the database

In a real system, `apply_committed` would execute commands against the database:

```rust,ignore
/// A state machine that applies Raft log entries as database commands.
pub struct DatabaseStateMachine {
    db: Database,  // from earlier chapters
}

impl DatabaseStateMachine {
    pub fn apply(&mut self, command: &[u8]) -> Vec<u8> {
        let sql = String::from_utf8_lossy(command);
        match self.db.execute_query(&sql) {
            Response::Ok { message } => message.into_bytes(),
            Response::Rows { columns, rows } => {
                // Serialize rows to bytes
                format!("{} rows", rows.len()).into_bytes()
            }
            Response::Error { message } => {
                format!("ERROR: {}", message).into_bytes()
            }
        }
    }
}
```

### Step 5: Test the complete replication flow

```rust,ignore
#[test]
fn test_log_replication() {
    let mut network = SimulatedNetwork::new(3);

    // Elect node 1 as leader
    network.expire_election_timer(1);
    network.tick_all();
    network.deliver_all(); // RequestVote
    network.deliver_all(); // VoteResponse
    assert_eq!(network.nodes[&1].state, NodeState::Leader);

    // Client sends a write to the leader
    let index = network.nodes.get_mut(&1).unwrap()
        .client_request(b"SET x 1".to_vec())
        .unwrap();
    assert_eq!(index, 1);

    // Leader sends AppendEntries to followers
    let messages = network.nodes[&1].send_append_entries();
    for (to, msg) in messages {
        let responses = network.nodes.get_mut(&to).unwrap()
            .handle_message(1, msg);
        for (resp_to, resp_msg) in responses {
            network.in_flight.push((to, resp_to, resp_msg));
        }
    }

    // Followers should have the entry
    assert_eq!(network.nodes[&2].log.last_index(), 1);
    assert_eq!(network.nodes[&3].log.last_index(), 1);

    // Deliver responses to leader
    network.deliver_all();

    // Leader should have committed the entry
    assert_eq!(network.nodes[&1].commit_index, 1);

    // Apply committed entries
    let results = network.nodes.get_mut(&1).unwrap().apply_committed();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].command, b"SET x 1");
}

#[test]
fn test_follower_catch_up() {
    let mut network = SimulatedNetwork::new(3);

    // Elect node 1 as leader
    network.expire_election_timer(1);
    network.tick_all();
    network.deliver_all();
    network.deliver_all();

    // Add 5 entries to the leader
    for i in 1..=5 {
        network.nodes.get_mut(&1).unwrap()
            .client_request(format!("SET x {}", i).into_bytes());
    }

    // Only replicate to node 2 (simulate node 3 being partitioned)
    let messages = network.nodes[&1].send_append_entries();
    for (to, msg) in messages {
        if to == 2 { // only deliver to node 2
            let responses = network.nodes.get_mut(&2).unwrap()
                .handle_message(1, msg);
            for (resp_to, resp_msg) in responses {
                let _ = network.nodes.get_mut(&resp_to).unwrap()
                    .handle_message(2, resp_msg);
            }
        }
    }

    // Node 2 has all 5 entries, node 3 has none
    assert_eq!(network.nodes[&2].log.last_index(), 5);
    assert_eq!(network.nodes[&3].log.last_index(), 0);

    // Entries should be committed (leader + node 2 = majority)
    assert_eq!(network.nodes[&1].commit_index, 5);

    // Now heal the partition — send AppendEntries to node 3
    let messages = network.nodes[&1].send_append_entries();
    for (to, msg) in messages {
        if to == 3 {
            let responses = network.nodes.get_mut(&3).unwrap()
                .handle_message(1, msg);
            for (resp_to, resp_msg) in responses {
                let _ = network.nodes.get_mut(&resp_to).unwrap()
                    .handle_message(3, resp_msg);
            }
        }
    }

    // Node 3 should have caught up
    assert_eq!(network.nodes[&3].log.last_index(), 5);
}
```

<details>
<summary>Hint: Why "committing N implies committing all entries before N"?</summary>

Because of the Log Matching Property. If the leader has entry N and a follower also has entry N (verified by the consistency check), then they also agree on all entries 1 through N-1. So when entry N is replicated to a majority, entries 1 through N-1 are also replicated to a majority. Commitment propagates backwards through the log.

This is why we scan from the highest uncommitted index downward in `advance_commit_index` — once we find an index that has been replicated to a majority, everything below it is also committed.

</details>

---

## Exercise 4: Shared State with `Arc<Mutex<RaftState>>`

**Goal:** Wrap the Raft node's mutable state in `Arc<Mutex<>>` so it can be shared across async tasks — the network listener, the heartbeat timer, the client handler, and the state machine applicator.

### Step 1: Define the shared state

```rust,ignore
use std::sync::{Arc, Mutex};

/// The complete state of a Raft node, wrapped for concurrent access.
pub struct RaftState {
    pub node: RaftNode,
    pub state_machine: DatabaseStateMachine,
}

/// A handle to the shared Raft state. Clone this to share across tasks.
pub type SharedRaftState = Arc<Mutex<RaftState>>;

impl RaftState {
    pub fn new(id: NodeId, peers: Vec<NodeId>, db: Database) -> SharedRaftState {
        Arc::new(Mutex::new(RaftState {
            node: RaftNode::new(id, peers),
            state_machine: DatabaseStateMachine { db },
        }))
    }
}
```

### Step 2: The async server with shared state

```rust,ignore
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tokio::time::{interval, Duration};

async fn run_raft_server(
    id: NodeId,
    peers: Vec<NodeId>,
    addr: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let state = RaftState::new(id, peers, Database::new());
    let (shutdown_tx, _) = broadcast::channel::<()>(1);

    // Task 1: Listen for client connections
    let client_state = Arc::clone(&state);
    let client_shutdown = shutdown_tx.subscribe();
    let client_handle = tokio::spawn(async move {
        run_client_listener(addr, client_state, client_shutdown).await;
    });

    // Task 2: Periodic tick (election timeouts, heartbeats)
    let tick_state = Arc::clone(&state);
    let tick_shutdown = shutdown_tx.subscribe();
    let tick_handle = tokio::spawn(async move {
        run_tick_loop(tick_state, tick_shutdown).await;
    });

    // Task 3: Apply committed entries to state machine
    let apply_state = Arc::clone(&state);
    let apply_shutdown = shutdown_tx.subscribe();
    let apply_handle = tokio::spawn(async move {
        run_apply_loop(apply_state, apply_shutdown).await;
    });

    // Wait for shutdown signal
    tokio::signal::ctrl_c().await?;
    let _ = shutdown_tx.send(());

    // Wait for all tasks to finish
    let _ = tokio::join!(client_handle, tick_handle, apply_handle);

    Ok(())
}
```

### Step 3: The tick loop

The tick loop checks for election timeouts and sends heartbeats:

```rust,ignore
async fn run_tick_loop(
    state: SharedRaftState,
    mut shutdown: broadcast::Receiver<()>,
) {
    let mut ticker = interval(Duration::from_millis(10));

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                let messages = {
                    let mut state = state.lock().unwrap();
                    let mut messages = state.node.tick();

                    // If leader, also send heartbeats periodically
                    if state.node.state == NodeState::Leader {
                        messages.extend(state.node.send_append_entries());
                    }

                    messages
                };
                // Lock released here

                // Send messages over the network
                for (to, msg) in messages {
                    send_to_peer(to, msg).await;
                }
            }
            _ = shutdown.recv() => {
                println!("Tick loop shutting down");
                return;
            }
        }
    }
}
```

Notice the pattern: lock the mutex, extract the data you need, drop the lock, then do async work (network sends). The lock is held only during the pure computation (calling `tick()` and `send_append_entries()`), never during I/O.

### Step 4: The apply loop

The apply loop periodically checks for committed entries and applies them:

```rust,ignore
async fn run_apply_loop(
    state: SharedRaftState,
    mut shutdown: broadcast::Receiver<()>,
) {
    let mut ticker = interval(Duration::from_millis(10));

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                let results = {
                    let mut state = state.lock().unwrap();
                    // Apply any committed but unapplied entries
                    state.node.apply_committed()
                };
                // Lock released

                // Process results (e.g., respond to waiting clients)
                for result in results {
                    println!(
                        "Applied entry {}: {}",
                        result.index,
                        String::from_utf8_lossy(&result.result)
                    );
                }
            }
            _ = shutdown.recv() => {
                // Apply any remaining committed entries before shutting down
                let results = {
                    let mut state = state.lock().unwrap();
                    state.node.apply_committed()
                };
                for result in results {
                    println!(
                        "Applied entry {} (during shutdown): {}",
                        result.index,
                        String::from_utf8_lossy(&result.result)
                    );
                }
                println!("Apply loop shutting down");
                return;
            }
        }
    }
}
```

### Step 5: Handle peer messages

When a message arrives from another Raft node, we lock the state, process the message, unlock, and send any response messages:

```rust,ignore
async fn handle_peer_message(
    state: SharedRaftState,
    from: NodeId,
    message: RaftMessage,
) {
    let response_messages = {
        let mut state = state.lock().unwrap();
        state.node.handle_message(from, message)
    };
    // Lock released

    // Send responses
    for (to, msg) in response_messages {
        send_to_peer(to, msg).await;
    }
}
```

### Step 6: The full architecture

```
                    ┌─────────────────────────────────────┐
                    │        Arc<Mutex<RaftState>>         │
                    │  ┌────────────────────────────────┐  │
                    │  │ RaftNode                       │  │
                    │  │  - log: RaftLog                │  │
                    │  │  - state: NodeState            │  │
  Tick Loop ───────►│  │  - commit_index: u64           │◄───── Peer Messages
  (10ms interval)   │  │  - next_index: HashMap         │  │     (from network)
                    │  │  - match_index: HashMap        │  │
                    │  ├────────────────────────────────┤  │
  Apply Loop ──────►│  │ DatabaseStateMachine           │  │
  (10ms interval)   │  │  - db: Database                │  │
                    │  └────────────────────────────────┘  │
  Client Handler ──►│                                     │◄───── Client Requests
                    └─────────────────────────────────────┘       (from network)

  Four async tasks share the same RaftState via Arc<Mutex<>>:
  1. Tick loop:     checks timeouts, sends heartbeats
  2. Apply loop:    applies committed entries to database
  3. Peer handler:  processes incoming Raft messages
  4. Client handler: accepts client queries/writes
```

Every task follows the same pattern:
1. Lock the mutex
2. Do fast, synchronous work (compute, update state)
3. Extract any data needed for I/O (messages to send, results to return)
4. Unlock the mutex (guard dropped)
5. Do async I/O (send network messages, respond to clients)

This keeps the critical section short and contention low.

### Step 7: Testing with simulated network partitions

```rust,ignore
#[test]
fn test_replication_survives_leader_change() {
    let mut network = SimulatedNetwork::new(3);

    // Elect node 1 as leader
    network.expire_election_timer(1);
    network.tick_all();
    network.deliver_all();
    network.deliver_all();

    // Write 3 entries, replicate to all
    for i in 1..=3 {
        network.nodes.get_mut(&1).unwrap()
            .client_request(format!("SET x {}", i).into_bytes());
    }
    let messages = network.nodes[&1].send_append_entries();
    for (to, msg) in messages {
        let responses = network.nodes.get_mut(&to).unwrap()
            .handle_message(1, msg);
        for (resp_to, resp_msg) in responses {
            let _ = network.nodes.get_mut(&resp_to).unwrap()
                .handle_message(to, resp_msg);
        }
    }

    // All nodes have entries 1-3, leader has committed them
    assert_eq!(network.nodes[&1].commit_index, 3);
    assert_eq!(network.nodes[&2].log.last_index(), 3);
    assert_eq!(network.nodes[&3].log.last_index(), 3);

    // Node 1 crashes — simulate by not ticking or delivering to it

    // Node 2 wins election for term 2
    network.expire_election_timer(2);
    let messages = network.nodes.get_mut(&2).unwrap().tick();
    // Only deliver between nodes 2 and 3
    for (to, msg) in messages {
        if to == 3 {
            let responses = network.nodes.get_mut(&3).unwrap()
                .handle_message(2, msg);
            for (resp_to, resp_msg) in responses {
                if resp_to == 2 {
                    let _ = network.nodes.get_mut(&2).unwrap()
                        .handle_message(3, resp_msg);
                }
            }
        }
    }

    assert_eq!(network.nodes[&2].state, NodeState::Leader);
    assert_eq!(network.nodes[&2].current_term, 2);

    // Write a new entry via new leader
    let index = network.nodes.get_mut(&2).unwrap()
        .client_request(b"SET y 100".to_vec())
        .unwrap();
    assert_eq!(index, 4);

    // The new leader has all committed entries from the old leader,
    // plus its new entry
    assert_eq!(network.nodes[&2].log.last_index(), 4);
    assert_eq!(network.nodes[&2].log.get(1).unwrap().command, b"SET x 1");
    assert_eq!(network.nodes[&2].log.get(4).unwrap().command, b"SET y 100");
}
```

This test demonstrates the key guarantee: when a new leader is elected, it has all previously committed entries. No data is lost during leader changes.

---

## Rust Gym

### Drill 1: Arc<Mutex<>> Basics

Implement a shared counter that multiple threads increment:

```rust,ignore
use std::sync::{Arc, Mutex};
use std::thread;

fn main() {
    let counter = Arc::new(Mutex::new(0u64));
    let mut handles = Vec::new();

    for i in 0..5 {
        let counter = Arc::clone(&counter);
        handles.push(thread::spawn(move || {
            // Increment the counter 1000 times
            todo!()
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let final_count = counter.lock().unwrap();
    println!("Final count: {}", *final_count);
    assert_eq!(*final_count, 5000);
}
```

<details>
<summary>Solution</summary>

```rust
use std::sync::{Arc, Mutex};
use std::thread;

fn main() {
    let counter = Arc::new(Mutex::new(0u64));
    let mut handles = Vec::new();

    for _i in 0..5 {
        let counter = Arc::clone(&counter);
        handles.push(thread::spawn(move || {
            for _ in 0..1000 {
                let mut value = counter.lock().unwrap();
                *value += 1;
                // MutexGuard dropped here — lock released every iteration
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let final_count = counter.lock().unwrap();
    println!("Final count: {}", *final_count);
    assert_eq!(*final_count, 5000);
}
```

Key insight: the lock is acquired and released on every iteration. Each `lock()` call returns a `MutexGuard`, which releases the lock when dropped at the end of the `for` body. This gives other threads a chance to make progress between iterations.

If you lock once and then loop 1000 times, you hold the lock for the entire duration — other threads are blocked. Short critical sections are essential for concurrency.

</details>

### Drill 2: Scoped Locks in Async

Fix this async function that holds a `std::sync::Mutex` guard across an `.await` point:

```rust,ignore
use std::sync::{Arc, Mutex};
use tokio::time::{sleep, Duration};

struct State {
    value: u64,
    history: Vec<u64>,
}

// This function has a bug: it holds the MutexGuard across .await
async fn update_state_buggy(state: Arc<Mutex<State>>, new_value: u64) {
    let mut s = state.lock().unwrap();
    s.history.push(s.value);
    s.value = new_value;
    // Simulate some async work (e.g., notifying peers)
    sleep(Duration::from_millis(10)).await; // BUG: lock held across .await
    println!("Updated to {}", s.value);
}

// Fix the function so the lock is not held across .await
async fn update_state_fixed(state: Arc<Mutex<State>>, new_value: u64) {
    todo!()
}
```

<details>
<summary>Solution</summary>

```rust,ignore
use std::sync::{Arc, Mutex};
use tokio::time::{sleep, Duration};

struct State {
    value: u64,
    history: Vec<u64>,
}

async fn update_state_fixed(state: Arc<Mutex<State>>, new_value: u64) {
    // Scope 1: lock, update, unlock
    let current_value = {
        let mut s = state.lock().unwrap();
        s.history.push(s.value);
        s.value = new_value;
        s.value  // extract what we need before releasing the lock
    };
    // Lock released here

    // Async work happens without holding the lock
    sleep(Duration::from_millis(10)).await;

    println!("Updated to {}", current_value);
}
```

The fix: use a scoping block `{ ... }` to limit the lock's lifetime. Extract any values you need (here, `current_value`) before the block ends. After the block, the `MutexGuard` is dropped and the lock is released. Now `sleep().await` does not hold the lock.

This pattern is universal in async Rust code that uses `std::sync::Mutex`. The compiler actually helps here: `MutexGuard` from `std::sync` is not `Send`, so if you hold it across `.await`, the future is not `Send`, and `tokio::spawn` will reject it with a compile error. The compiler catches the bug before it reaches production.

</details>

### Drill 3: Majority Calculator

Implement a function that determines if an entry has been replicated to a majority:

```rust,ignore
use std::collections::HashMap;

struct ReplicationState {
    cluster_size: usize,
    match_index: HashMap<u64, u64>,  // peer_id -> their match index
}

impl ReplicationState {
    fn new(cluster_size: usize) -> Self {
        todo!()
    }

    fn update_match(&mut self, peer_id: u64, index: u64) {
        todo!()
    }

    /// What is the highest index that has been replicated to a majority?
    /// The leader itself always has the entry, so we start counting from 1.
    fn majority_match_index(&self) -> u64 {
        todo!()
    }
}

fn main() {
    // 5-node cluster: need 3 nodes (including leader) for majority
    let mut state = ReplicationState::new(5);
    assert_eq!(state.majority_match_index(), 0);

    // Follower 2 has replicated up to index 5
    state.update_match(2, 5);
    assert_eq!(state.majority_match_index(), 0); // only 2 nodes (leader + peer 2)

    // Follower 3 has replicated up to index 3
    state.update_match(3, 3);
    assert_eq!(state.majority_match_index(), 3); // 3 nodes have index 3+

    // Follower 4 has replicated up to index 7
    state.update_match(4, 7);
    assert_eq!(state.majority_match_index(), 5); // 3 nodes have index 5+

    println!("All majority tests passed!");
}
```

<details>
<summary>Solution</summary>

```rust
use std::collections::HashMap;

struct ReplicationState {
    cluster_size: usize,
    match_index: HashMap<u64, u64>,
}

impl ReplicationState {
    fn new(cluster_size: usize) -> Self {
        ReplicationState {
            cluster_size,
            match_index: HashMap::new(),
        }
    }

    fn update_match(&mut self, peer_id: u64, index: u64) {
        self.match_index.insert(peer_id, index);
    }

    fn majority_match_index(&self) -> u64 {
        // Collect all match indices, including the leader's
        // (leader implicitly has everything — represented by u64::MAX)
        let mut indices: Vec<u64> = self.match_index.values().copied().collect();
        indices.push(u64::MAX); // leader has everything
        indices.sort_unstable_by(|a, b| b.cmp(a)); // sort descending

        // The majority_match is the value at position (quorum - 1) in the sorted list
        // because we need quorum nodes to have at least this index
        let quorum = self.cluster_size / 2 + 1;
        if indices.len() >= quorum {
            // The (quorum-1)th largest value — at least quorum nodes have this or higher
            indices[quorum - 1]
        } else {
            0
        }
    }
}

fn main() {
    let mut state = ReplicationState::new(5);
    assert_eq!(state.majority_match_index(), 0);

    state.update_match(2, 5);
    assert_eq!(state.majority_match_index(), 0);

    state.update_match(3, 3);
    assert_eq!(state.majority_match_index(), 3);

    state.update_match(4, 7);
    assert_eq!(state.majority_match_index(), 5);

    println!("All majority tests passed!");
}
```

The algorithm: sort all match indices (including the leader's, which is effectively infinite) in descending order. The value at position `quorum - 1` is the highest index that at least `quorum` nodes have. This is the commit point.

For the example with indices [MAX, 7, 5, 3] and quorum=3: position 2 (0-indexed) is 5. Three nodes (leader, peer 4, peer 2) have index >= 5.

</details>

### Drill 4: Log Consistency Check

Implement the log consistency check from AppendEntries:

```rust,ignore
struct SimpleLog {
    entries: Vec<(u64, String)>,  // (term, command)
}

impl SimpleLog {
    fn new() -> Self {
        SimpleLog { entries: Vec::new() }
    }

    fn append(&mut self, term: u64, command: &str) {
        self.entries.push((term, command.to_string()));
    }

    fn last_index(&self) -> u64 {
        self.entries.len() as u64
    }

    /// Check consistency and append entries if they pass.
    /// Returns true if the entries were accepted.
    fn try_append(
        &mut self,
        prev_index: u64,
        prev_term: u64,
        new_entries: Vec<(u64, String)>,
    ) -> bool {
        todo!()
    }
}

fn main() {
    let mut log = SimpleLog::new();
    log.append(1, "SET x 1");
    log.append(1, "SET y 2");
    log.append(2, "SET z 3");

    // Valid: prev matches
    assert!(log.try_append(3, 2, vec![(2, "SET w 4".to_string())]));
    assert_eq!(log.last_index(), 4);

    // Invalid: prev term mismatch
    let mut log2 = SimpleLog::new();
    log2.append(1, "SET x 1");
    assert!(!log2.try_append(1, 2, vec![(2, "SET y 2".to_string())]));
    assert_eq!(log2.last_index(), 1); // unchanged

    // Valid: prev_index=0 (empty log prefix)
    let mut log3 = SimpleLog::new();
    assert!(log3.try_append(0, 0, vec![(1, "SET x 1".to_string())]));
    assert_eq!(log3.last_index(), 1);

    println!("All consistency tests passed!");
}
```

<details>
<summary>Solution</summary>

```rust
struct SimpleLog {
    entries: Vec<(u64, String)>,
}

impl SimpleLog {
    fn new() -> Self {
        SimpleLog { entries: Vec::new() }
    }

    fn append(&mut self, term: u64, command: &str) {
        self.entries.push((term, command.to_string()));
    }

    fn last_index(&self) -> u64 {
        self.entries.len() as u64
    }

    fn try_append(
        &mut self,
        prev_index: u64,
        prev_term: u64,
        new_entries: Vec<(u64, String)>,
    ) -> bool {
        // Check 1: if prev_index > 0, we must have that entry
        if prev_index > 0 {
            if prev_index as usize > self.entries.len() {
                return false; // we don't have this entry
            }
            let (term, _) = &self.entries[(prev_index - 1) as usize];
            if *term != prev_term {
                return false; // term mismatch
            }
        }

        // Consistency check passed — append entries
        for (i, (term, cmd)) in new_entries.into_iter().enumerate() {
            let target_index = prev_index as usize + 1 + i;
            if target_index <= self.entries.len() {
                // Entry exists — check for conflict
                if self.entries[target_index - 1].0 != term {
                    // Conflict: truncate and append
                    self.entries.truncate(target_index - 1);
                    self.entries.push((term, cmd));
                }
                // Same term: skip (already have it)
            } else {
                // New entry: append
                self.entries.push((term, cmd));
            }
        }

        true
    }
}

fn main() {
    let mut log = SimpleLog::new();
    log.append(1, "SET x 1");
    log.append(1, "SET y 2");
    log.append(2, "SET z 3");

    assert!(log.try_append(3, 2, vec![(2, "SET w 4".to_string())]));
    assert_eq!(log.last_index(), 4);

    let mut log2 = SimpleLog::new();
    log2.append(1, "SET x 1");
    assert!(!log2.try_append(1, 2, vec![(2, "SET y 2".to_string())]));
    assert_eq!(log2.last_index(), 1);

    let mut log3 = SimpleLog::new();
    assert!(log3.try_append(0, 0, vec![(1, "SET x 1".to_string())]));
    assert_eq!(log3.last_index(), 1);

    println!("All consistency tests passed!");
}
```

The consistency check is the guard that maintains the Log Matching Property. Without it, followers could accept entries that conflict with the leader's log, leading to divergent state machines. With it, every accepted entry is guaranteed to be consistent with the leader's log from the beginning.

</details>

---

## DSA in Context: The Raft Replicated Log

The Raft log is a specialized data structure with unique properties that arise from its distributed context.

### Log as an ordered event stream

At its core, the Raft log is an append-only sequence of events. This is the same abstraction as:

- **Database write-ahead logs (WAL)**: PostgreSQL's WAL, MySQL's binlog
- **Event sourcing**: storing every state change as an immutable event
- **Kafka**: a distributed commit log
- **Git**: a chain of commits (each commit references its parent)

The insight: if every node applies the same events in the same order, they end up in the same state. This is the **state machine replication** principle, proven by Schneider in 1990.

### Comparing replication strategies

| Strategy | Consistency | Latency | Availability |
|----------|------------|---------|-------------|
| Single leader (Raft) | Linearizable | Higher (majority ack) | Majority needed |
| Multi-leader | Eventual | Lower (local ack) | Any node can write |
| Leaderless (Dynamo) | Eventual | Lowest (quorum write) | Configurable |
| Chain replication | Linearizable | Higher (all nodes) | All nodes needed |

Raft chooses linearizable consistency at the cost of latency (must wait for majority acknowledgment) and availability (cannot accept writes without a majority).

### Log compaction

The log grows unboundedly. In production, you need **log compaction** — periodically taking a snapshot of the state machine and discarding all log entries before the snapshot:

```
Before compaction:
  Log: [1] [2] [3] [4] [5] [6] [7] [8] [9] [10]
  Snapshot: (none)

After compaction at index 7:
  Log: [8] [9] [10]
  Snapshot: state machine state as of entry 7
```

When a follower is so far behind that the leader has already compacted the entries it needs, the leader sends the snapshot instead. This is the `InstallSnapshot` RPC in the Raft paper.

### Performance characteristics

```
Operation          | Time complexity | Amortized
-------------------|-----------------|-----------
Append (leader)    | O(1)            | O(1)
Replicate (leader) | O(entries * peers) | O(peers) per entry
Commit check       | O(peers)        | O(peers)
Apply              | O(1) per entry  | O(1)
Consistency check  | O(1)            | O(1)
Conflict repair    | O(divergent entries) | Rare
```

The critical path for a write: client -> leader append -> replicate to majority -> commit -> respond. This is typically 2-4 network round trips, plus the time to persist the entry to disk (covered in Chapter 16).

---

## System Design Corner: Replication Strategies

Log replication is one of several approaches to keeping data copies in sync. Understanding the tradeoffs demonstrates depth in system design discussions.

### Synchronous vs asynchronous replication

**Synchronous** (Raft's approach): the leader waits for a majority of followers to acknowledge before confirming the write. Guarantees durability — if the leader crashes, the data is on other nodes.

```
Client ──► Leader ──► Follower 1 (ack)
                  └──► Follower 2 (ack)
           ◄──── majority ack ────
Client ◄── OK
```

**Asynchronous** (MySQL default, MongoDB default): the leader confirms the write immediately and replicates in the background. Lower latency but data can be lost if the leader crashes before replicating.

```
Client ──► Leader ──► (background: replicate to followers)
Client ◄── OK         (followers get it eventually)
```

**Semi-synchronous** (MySQL semi-sync): wait for at least one follower to acknowledge. A middle ground.

### Quorum writes and reads

Raft uses a write quorum (majority acknowledgment). For reads, the leader can serve reads without contacting followers (it knows it has the latest data). But this only works if the leader is still the leader — a stale leader might serve stale data.

Solutions for linearizable reads:
1. **Read index**: leader confirms it is still leader by sending a heartbeat round before serving the read
2. **Leader lease**: leader serves reads during its lease period (depends on bounded clock skew)
3. **Read from quorum**: read from a majority of nodes (expensive but safe without leader confirmation)

### Chain replication

An alternative to quorum-based replication:

```
Client ──► Head ──► Middle ──► Tail ──► Client (read/confirm)
```

Writes go to the head and propagate through the chain. Reads go to the tail (which has the most up-to-date confirmed data). Advantages: reads are always linearizable without special protocols. Disadvantages: latency is proportional to chain length, and any node failure requires reconfiguration.

HDFS uses a variant of chain replication for writing data blocks.

### Conflict-free replicated data types (CRDTs)

For data structures where operations are commutative (order does not matter), you can replicate without consensus:

```rust,ignore
// A grow-only counter — each node maintains its own count
struct GCounter {
    counts: HashMap<NodeId, u64>,
}

impl GCounter {
    fn increment(&mut self, node_id: NodeId) {
        *self.counts.entry(node_id).or_insert(0) += 1;
    }

    fn value(&self) -> u64 {
        self.counts.values().sum()
    }

    fn merge(&mut self, other: &GCounter) {
        for (&node, &count) in &other.counts {
            let entry = self.counts.entry(node).or_insert(0);
            *entry = (*entry).max(count);
        }
    }
}
```

CRDTs are used in systems that prioritize availability over consistency (AP in CAP theorem): Redis CRDTs, Riak, Automerge (for collaborative editing).

> **Interview talking point:** *"Our database uses Raft log replication for strong consistency. Each write is appended to the leader's log, replicated to a majority of followers via AppendEntries RPCs with consistency checks, and committed only after majority acknowledgment. The Log Matching Property ensures that if two nodes agree on entry N, they agree on all entries 1 through N. We share Raft state across async tasks using Arc<Mutex<>> with short critical sections — the lock is held only during state computation, never across network I/O. For production, I would add log compaction with snapshots to bound memory usage, batch AppendEntries for throughput, and pipeline replication to hide network latency."*

---

## Design Insight: Modules Should Be Deep

In *A Philosophy of Software Design*, Ousterhout distinguishes between **deep modules** (simple interface, complex implementation) and **shallow modules** (complex interface, simple implementation). Deep modules are better — they hide complexity behind simple abstractions.

> *"The best modules are those whose interfaces are much simpler than their implementations."*

The Raft log is a deep module. Its interface is remarkably simple:

```rust,ignore
// The interface (what callers see):
log.append(term, command)           // add an entry
log.get(index)                      // read an entry
log.matches(index, term)            // consistency check
log.append_entries(prev, entries)   // replicate entries
```

Five methods. That is the entire interface. Behind this interface, the implementation handles:
- Term-based conflict detection
- Automatic truncation of divergent entries
- The Log Matching Property invariant
- Index translation (1-based external, 0-based internal)
- Efficient sequential access patterns

The `RaftNode` is also a deep module. Its interface is two methods:

```rust,ignore
node.tick()                         // check timeouts, produce messages
node.handle_message(from, msg)      // process incoming message, produce responses
```

Behind this interface: leader election with randomized timeouts, vote counting with quorum detection, term management with automatic step-down, log replication with consistency repair, commitment with majority verification, and state machine application.

A shallow alternative would expose all the internal state transitions as separate methods: `start_election()`, `grant_vote()`, `count_votes()`, `become_leader()`, `check_commit()`, `advance_commit()`, `apply_entry()`. The caller would need to understand the Raft protocol to call them in the right order. The deep interface — `tick()` and `handle_message()` — hides all of this. The caller just delivers messages and checks for outgoing messages.

This is why the Raft paper is so effective as a teaching tool: it presents a complex algorithm through a simple interface (two RPCs: RequestVote and AppendEntries). The implementation is non-trivial, but the interface is elegant. Deep modules make complex systems manageable.

> *"The best modules are those whose interfaces are much simpler than their implementations."*
> — John Ousterhout

---

## What You Built

In this chapter, you:

1. **Built the Raft log** — `RaftLog` with append, truncation, consistency checking, and the Log Matching Property invariant
2. **Implemented AppendEntries** — full RPC handling on both leader and follower sides, with the consistency check that detects and repairs divergent logs
3. **Added commitment rules** — majority-based commitment, the current-term restriction, and the `advance_commit_index` algorithm
4. **Applied committed entries** — `apply_committed()` feeds committed log entries to the database state machine in order
5. **Shared state with Arc<Mutex<>>** — wrapped `RaftState` for concurrent access from async tasks, with short critical sections and proper lock scoping
6. **Tested replication** — leader election, log replication, follower catch-up, and leader change scenarios with the deterministic test harness

Your database is now replicated. Writes go to the leader, are replicated to followers, committed when a majority acknowledges them, and applied to the state machine. If the leader crashes, a new leader is elected with all committed data intact. This is the core guarantee of Raft: **no committed entry is ever lost**.

Chapter 16 adds durability — persisting the log and Raft state to disk so that nodes can recover after crashes without losing their state.

---

### DS Deep Dive

The Raft log is a specific instance of a broader concept: replicated state machines. This deep dive explores the theory of state machine replication (Schneider 1990), compares it with operation-based and state-based replication, and traces how the same idea appears in database WALs, event sourcing systems, and blockchain. We examine the CAP theorem through the lens of Raft's design choices and discuss why linearizability matters for database correctness.

**-> [Replicated State Machines — "The Copy Room"](../ds-narratives/ch15-replicated-state-machines.md)**

---

### Reference implementation

The files you built in this chapter correspond to these files in the reference codebase:

| Your file | Reference |
|-----------|-----------|
| `RaftLog` — replicated log | [`src/raft/log.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/raft/log.rs) — `Log` struct |
| `AppendEntries` handling | [`src/raft/node.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/raft/node.rs) — `append`, `heartbeat` |
| `commit_index`, `advance_commit_index` | [`src/raft/node.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/raft/node.rs) — `commit` logic |
| `Arc<Mutex<RaftState>>` | [`src/raft/node.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/raft/node.rs) — state sharing |
| State machine application | [`src/raft/state.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/raft/state.rs) — `apply` |
| Replication tests | [`tests/`](https://github.com/erikgrinaker/toydb/tree/master/tests) — cluster tests |
