# Chapter 15: Raft -- Log Replication

Your cluster can elect a leader. Congratulations. But a leader that does not do anything useful is just a figurehead. The entire point of Raft is to keep data safe by storing copies on multiple servers. When a client sends a write to the leader, the leader needs to:

1. Store the write in its own log
2. Send the write to all followers
3. Wait until a majority of followers confirm they have it
4. Only then tell the client "your write is saved"

This is **log replication** -- the process of keeping every server's log in sync. If the leader crashes after a majority has the data, the next leader is guaranteed to have it too. No data is lost.

This chapter builds the replication machinery. You will learn about `Arc` and `Mutex` (Rust's tools for safely sharing data between tasks), the `AppendEntries` RPC, and how the leader tracks what each follower knows.

By the end of this chapter, you will have:

- A `RaftLog` data structure for storing ordered log entries
- The `AppendEntries` RPC for replicating entries and sending heartbeats
- Leader bookkeeping with `next_index` and `match_index` per follower
- Commitment rules: an entry is committed when a majority has it
- A working replication simulation
- A solid understanding of `Arc` and `Mutex`

---

## Spotlight: Concurrency -- Arc & Mutex

Every chapter has one **spotlight concept**. This chapter's spotlight is **Arc and Mutex** -- Rust's tools for safely sharing data between concurrent tasks.

### The problem: sharing data

In Chapter 13, we used `tokio::spawn` to handle each client in its own task. But a Raft node needs to do many things at once:

1. Receive client requests
2. Send entries to followers
3. Receive confirmations from followers
4. Apply committed entries to the database
5. Send heartbeats on a timer

All of these need access to the same data -- the Raft log, the current term, the commit index. How do you share data between tasks safely?

Most languages let you share data freely and hope for the best. If two threads modify the same variable at the same time, you get a **data race** -- a bug where the result depends on timing. Data races cause corrupted data, crashes, and security vulnerabilities. They are notoriously hard to find because they only happen under specific timing conditions that are hard to reproduce.

Rust takes a different approach: **the compiler prevents data races entirely.** If your code compiles, it has no data races. The tools that make this possible are `Arc` and `Mutex`.

### Arc: sharing ownership (the library book analogy)

Imagine a library book. Normally, one person checks it out, reads it, and returns it. The library tracks who has the book.

Now imagine a magical library where the book can be in multiple people's hands at the same time -- but the library keeps a counter. Person A checks it out (counter: 1). Person B checks it out (counter: 2). Person A returns it (counter: 1). Person B returns it (counter: 0). When the counter reaches zero, the library puts the book back on the shelf.

That is `Arc` -- **Atomically Reference Counted**. It lets multiple owners share the same data. When the last owner drops their `Arc`, the data is freed.

```rust,ignore
use std::sync::Arc;

// Create shared data
let data = Arc::new(vec![1, 2, 3]);

// Clone the Arc -- this does NOT copy the vector!
// It just increments the reference counter.
let data_for_task = Arc::clone(&data);

// Both `data` and `data_for_task` point to the SAME vector.
println!("Main sees: {:?}", data);           // [1, 2, 3]
println!("Task sees: {:?}", data_for_task);  // [1, 2, 3]
```

> **Programming Concept: Arc vs Rc**
>
> Rust has two reference-counted smart pointers:
> - **`Rc`** (Reference Counted) -- for single-threaded code. The counter uses normal (non-atomic) operations, which are faster but not thread-safe.
> - **`Arc`** (Atomically Reference Counted) -- for multi-threaded code. The counter uses atomic CPU operations, which are safe across threads.
>
> If you try to use `Rc` with `tokio::spawn`, the compiler will refuse: "`Rc` cannot be sent between threads safely." This is Rust catching a potential data race at compile time.

### Mutex: exclusive access (the bathroom lock analogy)

`Arc` lets you share data, but only for reading. What if you need to change the data? You need a **Mutex** (mutual exclusion).

Think of a bathroom with a lock on the door. Only one person can be inside at a time. When you want to use it:

1. You check the lock. If it is unlocked, you go in and lock the door.
2. You do your business.
3. You unlock the door and leave.

If someone else tries to enter while it is locked, they wait until you come out.

```rust,ignore
use std::sync::Mutex;

// Create a mutex-protected counter
let counter = Mutex::new(0);

// Lock the mutex to access the data
{
    let mut value = counter.lock().unwrap();
    // `value` is a MutexGuard -- it acts like a mutable reference
    *value += 1;
    println!("Counter is now: {}", *value);
}
// The lock is automatically released here when `value` goes out of scope
```

The `lock()` method returns a **guard** (`MutexGuard`). The guard gives you access to the data inside. When the guard is dropped (goes out of scope), the lock is automatically released. You never need to manually "unlock" -- Rust's RAII (Resource Acquisition Is Initialization) pattern handles it.

> **What Just Happened?**
>
> `Mutex::lock()` does two things:
> 1. Waits until no one else is holding the lock
> 2. Returns a guard that gives you exclusive access to the data
>
> The `.unwrap()` handles the case where a thread panicked while holding the lock (the mutex is "poisoned"). For our purposes, unwrap is fine.

### Combining them: `Arc<Mutex<T>>`

To share mutable data across tasks:

```rust,ignore
use std::sync::{Arc, Mutex};

// The Raft state, shared across tasks
let state = Arc::new(Mutex::new(RaftState {
    log: Vec::new(),
    commit_index: 0,
    current_term: 0,
}));

// Give each task its own Arc (reference to the same data)
let state_for_task = Arc::clone(&state);

tokio::spawn(async move {
    // Lock, modify, unlock
    let mut s = state_for_task.lock().unwrap();
    s.log.push(new_entry);
    s.commit_index += 1;
    // Lock released here
});
```

The pattern is always the same:
1. Wrap your data in `Arc::new(Mutex::new(data))`
2. `Arc::clone` before each `tokio::spawn`
3. `lock().unwrap()` to access the data
4. Let the guard go out of scope to release the lock

### The critical rule: short lock scopes

The most important rule with `Mutex` in async code: **hold the lock for as short a time as possible.**

```rust,ignore
// BAD: lock held across .await
async fn bad_example(state: Arc<Mutex<RaftState>>) {
    let mut s = state.lock().unwrap();
    s.log.push(entry);
    network_send(&s).await;  // other tasks CANNOT access state while we wait!
    s.commit_index += 1;
}

// GOOD: lock acquired and released in tight scopes
async fn good_example(state: Arc<Mutex<RaftState>>) {
    // Scope 1: modify the log
    {
        let mut s = state.lock().unwrap();
        s.log.push(entry);
    }  // lock released

    network_send_something().await;  // other tasks CAN access state

    // Scope 2: update commit index
    {
        let mut s = state.lock().unwrap();
        s.commit_index += 1;
    }  // lock released
}
```

If you hold the lock during a network call, no other task can read or write the Raft state until the network call completes. That destroys concurrency. The pattern: **lock, do fast work, unlock, then await.**

> **Common Mistake: Holding MutexGuard Across `.await`**
>
> The compiler might even warn you about this in some cases. Even when it does not, holding a lock across an await point means other tasks are blocked from accessing the shared state for the entire duration of the await. Always use `{ ... }` blocks to ensure the guard is dropped before any `.await`.

---

## Exercise 1: The Raft Log

**Goal:** Build the data structure that stores ordered log entries -- the heart of Raft's replication.

### Step 1: What is the log?

Think of the Raft log like a teacher writing on a whiteboard. The teacher (leader) writes steps in order:

```
Step 1: Create the users table
Step 2: Insert Alice
Step 3: Insert Bob
Step 4: Update Alice's email
Step 5: Insert Charlie
```

Students (followers) copy these steps into their notebooks. Every student's notebook should be identical. If a student misses a class, the teacher helps them catch up by resending the missing steps.

The log has three important properties:

1. **Ordered**: entries have sequential indices starting at 1
2. **Immutable once committed**: once a majority has copied an entry, it is permanent
3. **Consistent**: if two nodes have an entry at the same index with the same term, the entries are identical

### Step 2: Define the log entry

Add to `src/raft.rs`:

```rust,ignore
/// A single entry in the Raft log.
/// Each entry records a command (like a SQL statement)
/// and the term it was received in.
#[derive(Debug, Clone, PartialEq)]
pub struct LogEntry {
    /// The term when this entry was created.
    pub term: Term,
    /// The position in the log (1-based).
    pub index: u64,
    /// The command to apply to the database.
    /// This is the actual data -- like a SQL statement
    /// serialized as bytes.
    pub command: Vec<u8>,
}
```

### Step 3: Build the RaftLog struct

```rust,ignore
/// The replicated log -- an ordered list of entries.
#[derive(Debug, Clone)]
pub struct RaftLog {
    /// The entries, stored in a Vec.
    /// entries[0] is log index 1, entries[1] is log index 2, etc.
    entries: Vec<LogEntry>,
}

impl RaftLog {
    /// Create an empty log.
    pub fn new() -> Self {
        RaftLog {
            entries: Vec::new(),
        }
    }

    /// The index of the last entry (0 if empty).
    pub fn last_index(&self) -> u64 {
        self.entries.len() as u64
    }

    /// The term of the last entry (0 if empty).
    pub fn last_term(&self) -> Term {
        self.entries.last().map(|e| e.term).unwrap_or(0)
    }

    /// Get the entry at the given index (1-based).
    pub fn get(&self, index: u64) -> Option<&LogEntry> {
        if index == 0 || index as usize > self.entries.len() {
            None
        } else {
            Some(&self.entries[(index - 1) as usize])
        }
    }

    /// Get the term of the entry at a given index.
    /// Returns 0 for index 0 (before the log starts).
    pub fn term_at(&self, index: u64) -> Term {
        if index == 0 {
            0
        } else {
            self.get(index).map(|e| e.term).unwrap_or(0)
        }
    }

    /// Append a new entry and return its index.
    pub fn append(&mut self, term: Term, command: Vec<u8>) -> u64 {
        let index = self.last_index() + 1;
        self.entries.push(LogEntry {
            term,
            index,
            command,
        });
        index
    }

    /// Get entries from start_index to the end.
    pub fn entries_from(&self, start_index: u64) -> Vec<LogEntry> {
        if start_index == 0 || start_index as usize > self.entries.len() {
            return Vec::new();
        }
        self.entries[(start_index - 1) as usize..].to_vec()
    }

    /// The number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the log is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
```

> **Programming Concept: Why 1-Based Indexing?**
>
> Raft uses 1-based log indexing (the first entry is index 1, not index 0). This means index 0 is special -- it means "before the log starts." This simplifies boundary conditions: an empty log has `last_index = 0`, and the consistency check with `prev_log_index = 0` always succeeds (there is nothing before the start).
>
> Internally, we store entries in a `Vec` (0-indexed), so we convert: `entries[(index - 1) as usize]`.

### Step 4: The consistency check

The most important method on the log is the consistency check: "Do I have an entry at this index with this term?"

```rust,ignore
impl RaftLog {
    /// Check if the log matches at the given index and term.
    /// This is used by the AppendEntries consistency check.
    pub fn matches(&self, index: u64, term: Term) -> bool {
        if index == 0 {
            return true;  // empty log always matches
        }
        match self.get(index) {
            Some(entry) => entry.term == term,
            None => false,  // we do not have this entry
        }
    }
}
```

This is the heart of Raft's safety. Before the leader sends new entries to a follower, it includes the index and term of the entry just before them. The follower checks: "Do I have that entry?" If yes, the logs agree up to that point and the new entries can be safely appended. If no, there is a disagreement and the leader needs to back up.

### Step 5: Conflict resolution

What happens when a follower's log disagrees with the leader's? The leader wins. The follower truncates its log at the point of disagreement and replaces it with the leader's entries:

```rust,ignore
impl RaftLog {
    /// Append entries from the leader, resolving conflicts.
    /// If there is a conflict (same index, different term),
    /// truncate our log and use the leader's entries.
    pub fn append_entries(&mut self, entries: Vec<LogEntry>) {
        for entry in entries {
            let idx = entry.index;
            if let Some(existing) = self.get(idx) {
                if existing.term != entry.term {
                    // Conflict! Truncate from here and append.
                    self.entries.truncate((idx - 1) as usize);
                    self.entries.push(entry);
                }
                // Same term: we already have this entry, skip.
            } else {
                // New entry: append.
                self.entries.push(entry);
            }
        }
    }
}
```

> **What Just Happened?**
>
> The conflict resolution logic handles the case where a follower was temporarily disconnected and received entries from a different leader. When the current leader's entries disagree, the follower trusts the leader and replaces its conflicting entries. This is safe because the leader's entries are backed by a majority -- the follower's conflicting entries were not committed.

### Step 6: Test the log

```rust,ignore
#[test]
fn test_raft_log_basics() {
    let mut log = RaftLog::new();

    // Empty log
    assert_eq!(log.last_index(), 0);
    assert_eq!(log.last_term(), 0);
    assert!(log.matches(0, 0));  // empty always matches

    // Append some entries
    let idx1 = log.append(1, b"INSERT INTO users VALUES (1, 'Alice')".to_vec());
    let idx2 = log.append(1, b"INSERT INTO users VALUES (2, 'Bob')".to_vec());
    let idx3 = log.append(2, b"UPDATE users SET name = 'Charlie' WHERE id = 1".to_vec());

    assert_eq!(idx1, 1);
    assert_eq!(idx2, 2);
    assert_eq!(idx3, 3);

    // Check properties
    assert_eq!(log.last_index(), 3);
    assert_eq!(log.last_term(), 2);
    assert_eq!(log.term_at(1), 1);
    assert_eq!(log.term_at(3), 2);

    // Consistency checks
    assert!(log.matches(1, 1));   // entry 1 has term 1
    assert!(log.matches(3, 2));   // entry 3 has term 2
    assert!(!log.matches(3, 1));  // entry 3 does NOT have term 1
    assert!(!log.matches(5, 1));  // entry 5 does not exist
}
```

---

## Exercise 2: AppendEntries RPC

**Goal:** Implement the message the leader uses to send entries to followers and maintain its authority through heartbeats.

### Step 1: Add the log to RaftNode

Update the `RaftNode` struct with log-related fields:

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

    // NEW fields for log replication
    /// The replicated log.
    pub log: RaftLog,

    /// Index of the highest committed entry.
    /// "Committed" means a majority of nodes have it.
    pub commit_index: u64,

    /// Index of the highest entry applied to the database.
    /// Always <= commit_index.
    pub last_applied: u64,

    // Leader-only fields (only used when state == Leader)
    /// For each follower: the next log entry to send them.
    pub next_index: HashMap<NodeId, u64>,

    /// For each follower: the highest entry they have confirmed.
    pub match_index: HashMap<NodeId, u64>,
}
```

Let's understand the new fields:

- **`log`** -- the entries themselves
- **`commit_index`** -- how far along we are in committing entries. Entries up to this index have been replicated to a majority and are safe.
- **`last_applied`** -- how far along we are in applying committed entries to the database. This might lag behind `commit_index` slightly.
- **`next_index`** -- leader's bookkeeping. For each follower, "what entry should I send them next?" Starts optimistically at the end of the log.
- **`match_index`** -- leader's bookkeeping. For each follower, "what is the latest entry they have confirmed?" Starts at 0 (unknown).

> **Programming Concept: HashMap<NodeId, u64>**
>
> A `HashMap` is like a dictionary or lookup table. Given a key (a `NodeId`), it gives you a value (a `u64`). The leader uses it to track each follower individually:
>
> ```rust,ignore
> use std::collections::HashMap;
>
> let mut next_index = HashMap::new();
> next_index.insert(2, 5);  // Node 2 needs entry 5 next
> next_index.insert(3, 3);  // Node 3 needs entry 3 next (behind!)
> next_index.insert(4, 5);  // Node 4 needs entry 5 next
>
> // Look up what Node 3 needs
> let next = next_index.get(&3);  // Some(&3)
> ```

### Step 2: Initialize leader state on election

When a node wins an election and becomes leader, it initializes the per-follower tracking:

```rust,ignore
impl RaftNode {
    fn become_leader(&mut self) {
        println!(
            "[Node {}] === WON ELECTION for term {} ===",
            self.id, self.current_term
        );

        self.state = NodeState::Leader;
        self.leader_id = Some(self.id);

        // Initialize leader bookkeeping
        let last_log_index = self.log.last_index();
        for &peer in &self.peers {
            // Optimistic: assume followers are up-to-date
            // If they are not, the consistency check will catch it
            // and we will back up
            self.next_index.insert(peer, last_log_index + 1);
            self.match_index.insert(peer, 0);
        }
    }
}
```

The leader starts by assuming all followers have the same log it does (`next_index = last_log_index + 1`). This is optimistic -- if a follower is behind, the first `AppendEntries` will fail the consistency check, and the leader will decrement `next_index` and try again. This back-tracking converges quickly.

### Step 3: The leader sends AppendEntries

```rust,ignore
impl RaftNode {
    /// Send AppendEntries RPCs to all followers.
    /// Called periodically for heartbeats and after new entries are appended.
    pub fn send_append_entries(&self) -> Vec<(NodeId, RaftMessage)> {
        if self.state != NodeState::Leader {
            return Vec::new();
        }

        let mut messages = Vec::new();

        for &peer in &self.peers {
            let next = self.next_index.get(&peer).copied().unwrap_or(1);
            let prev_log_index = next - 1;
            let prev_log_term = self.log.term_at(prev_log_index);

            // Get the entries to send (everything from next_index onward)
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
}
```

For each follower, the leader:
1. Looks up `next_index` -- the next entry the follower needs
2. Includes `prev_log_index` and `prev_log_term` -- for the consistency check
3. Sends all entries from `next_index` onward
4. Includes `leader_commit` so the follower knows what is committed

If there are no new entries, this becomes a **heartbeat** -- an empty `AppendEntries` that just says "I am still the leader."

### Step 4: Update the message types

We need to expand the `AppendEntries` message to carry log entries:

```rust,ignore
#[derive(Debug, Clone)]
pub enum RaftMessage {
    RequestVote {
        term: Term,
        candidate_id: NodeId,
        last_log_index: u64,
        last_log_term: Term,
    },
    RequestVoteResponse {
        term: Term,
        vote_granted: bool,
    },
    AppendEntries {
        term: Term,
        leader_id: NodeId,
        /// Index of the entry just before the new ones.
        prev_log_index: u64,
        /// Term of the prev_log_index entry.
        prev_log_term: Term,
        /// The entries to append (empty for heartbeat).
        entries: Vec<LogEntry>,
        /// The leader's commit index.
        leader_commit: u64,
    },
    AppendEntriesResponse {
        term: Term,
        success: bool,
        /// The follower's last log index (helps the leader update match_index).
        match_index: u64,
    },
}
```

### Step 5: Follower handles AppendEntries

```rust,ignore
impl RaftNode {
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
        // Reject if the leader's term is old
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

        // Valid leader -- reset election timer
        self.state = NodeState::Follower;
        self.leader_id = Some(leader_id);
        self.reset_election_timer();

        // Consistency check: do we have the entry at prev_log_index
        // with the right term?
        if !self.log.matches(prev_log_index, prev_log_term) {
            // Our log disagrees -- reject
            println!(
                "[Node {}] Log mismatch at index {} (expected term {}, have term {})",
                self.id,
                prev_log_index,
                prev_log_term,
                self.log.term_at(prev_log_index),
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

        // Append the new entries (resolving any conflicts)
        if !entries.is_empty() {
            self.log.append_entries(entries);
            println!(
                "[Node {}] Appended entries, log now has {} entries",
                self.id,
                self.log.len()
            );
        }

        // Update commit index
        if leader_commit > self.commit_index {
            self.commit_index = std::cmp::min(
                leader_commit,
                self.log.last_index(),
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

This is the most complex handler so far. Let's walk through it:

1. **Term check:** reject if the leader is from an old term
2. **Reset timer:** the leader is alive, so push back the election deadline
3. **Consistency check:** verify that our log matches the leader's at `prev_log_index`. If not, reject -- the leader will back up and try again.
4. **Append entries:** add the new entries, resolving any conflicts
5. **Update commit index:** if the leader has committed more entries than we knew about, update our commit index

> **What Just Happened?**
>
> The follower does two things: verify and store.
>
> **Verify:** "Do I agree with the leader about what came before these new entries?" This is the consistency check. If the follower's log diverges from the leader's, it says "no" and the leader tries an earlier starting point.
>
> **Store:** If the check passes, the follower appends the entries and acknowledges. The leader can now count this follower toward the majority needed for commitment.

### Step 6: Leader handles the response

```rust,ignore
impl RaftNode {
    fn handle_append_entries_response(
        &mut self,
        from: NodeId,
        success: bool,
        match_index: u64,
    ) -> Vec<(NodeId, RaftMessage)> {
        if self.state != NodeState::Leader {
            return Vec::new();
        }

        if success {
            // Update bookkeeping for this follower
            self.match_index.insert(from, match_index);
            self.next_index.insert(from, match_index + 1);

            // Check if any new entries can be committed
            self.try_advance_commit_index();
        } else {
            // The follower's log disagreed -- back up and retry
            let next = self.next_index.get(&from).copied().unwrap_or(1);
            if next > 1 {
                self.next_index.insert(from, next - 1);
                println!(
                    "[Node {}] Backing up next_index for {} to {}",
                    self.id, from, next - 1
                );
            }
        }

        Vec::new()
    }
}
```

When a follower succeeds, the leader updates `match_index` and `next_index`, then checks if any entries can be committed.

When a follower fails (log mismatch), the leader decrements `next_index` and will retry with an earlier `prev_log_index` on the next heartbeat. This back-tracking finds the point where the logs agree.

### Step 7: Commitment -- when a majority agrees

An entry is committed when a majority of nodes have it:

```rust,ignore
impl RaftNode {
    /// Check if any entries can be newly committed.
    /// An entry at index N is committed if a majority of
    /// match_index values are >= N.
    fn try_advance_commit_index(&mut self) {
        // We only commit entries from our current term
        // (Raft safety requirement)
        let last_index = self.log.last_index();

        for index in (self.commit_index + 1)..=last_index {
            // Check the term -- we only commit entries from current term
            if self.log.term_at(index) != self.current_term {
                continue;
            }

            // Count how many nodes have this entry
            let mut count = 1;  // we (the leader) always have it
            for &peer in &self.peers {
                if self.match_index.get(&peer).copied().unwrap_or(0) >= index {
                    count += 1;
                }
            }

            // If a majority has it, commit!
            if count >= self.quorum_size() {
                self.commit_index = index;
                println!(
                    "[Node {}] Entry {} committed (replicated to {} nodes)",
                    self.id, index, count
                );
            }
        }
    }
}
```

The commitment check is straightforward: for each uncommitted entry, count how many nodes have confirmed it. If the count reaches a majority, the entry is committed.

One subtle detail: we only commit entries from the **current term**. This is a Raft safety requirement. An entry from a previous term might be on a majority of nodes but still not safe to commit (there is a corner case described in the Raft paper). Committing an entry from the current term implicitly commits all earlier entries.

> **What Just Happened?**
>
> We built the complete replication pipeline:
>
> 1. **Leader appends** a new entry to its log
> 2. **Leader sends** `AppendEntries` to all followers
> 3. **Followers verify** the consistency check and append the entries
> 4. **Followers respond** with success and their latest log index
> 5. **Leader counts** confirmations and commits when a majority responds
>
> Once committed, an entry is permanent. Even if the leader crashes, the next leader will have all committed entries (because any new leader must have votes from a majority, and that majority overlaps with the majority that confirmed the committed entries).

---

## Exercise 3: Applying Committed Entries

**Goal:** Apply committed entries to the database state machine.

### Step 1: The apply loop

Once entries are committed, they need to be applied to the database. This is a simple loop that processes entries between `last_applied` and `commit_index`:

```rust,ignore
impl RaftNode {
    /// Apply committed entries to the state machine.
    /// Returns the commands that were applied.
    pub fn apply_committed(&mut self) -> Vec<Vec<u8>> {
        let mut applied = Vec::new();

        while self.last_applied < self.commit_index {
            self.last_applied += 1;

            if let Some(entry) = self.log.get(self.last_applied) {
                println!(
                    "[Node {}] Applying entry {} (term {})",
                    self.id, self.last_applied, entry.term
                );
                applied.push(entry.command.clone());
            }
        }

        applied
    }
}
```

The `apply_committed` method returns the commands (as byte vectors) that were applied. The caller (the server) takes these commands and executes them against the database -- parsing SQL, running queries, updating tables.

> **Programming Concept: Separation of Concerns**
>
> The Raft module does not know or care about SQL. It deals in opaque byte vectors (`Vec<u8>`). The server layer converts SQL strings to bytes when proposing entries, and converts bytes back to SQL when applying committed entries. This separation means Raft can be used for any kind of replicated state machine, not just databases.

### Step 2: The replication flow diagram

Here is the complete picture:

```
Client: INSERT INTO users VALUES (1, 'Alice')
                    |
                    v
            +-------+--------+
            |     LEADER     |
            | 1. Append to   |
            |    own log     |
            +-------+--------+
                    |
          +---------+---------+
          |         |         |
          v         v         v
      Follower  Follower  Follower
      2. Check   2. Check   2. Check
      3. Append  3. Append  3. Append
      4. Respond 4. Respond 4. Respond
          |         |         |
          +---------+---------+
                    |
            +-------+--------+
            |     LEADER     |
            | 5. Count acks  |
            | 6. Commit if   |
            |    majority    |
            | 7. Apply to DB |
            | 8. Reply to    |
            |    client      |
            +----------------+
```

> **Common Mistake: Replying Before Commit**
>
> A tempting shortcut is to reply to the client immediately after the leader appends the entry to its own log, without waiting for followers. This is dangerous -- if the leader crashes before replication, the entry is lost, but the client thinks it was saved. Always wait for a majority to confirm before replying.

---

## Exercise 4: Replication Simulation

**Goal:** Extend the election simulation from Chapter 14 to include log replication.

### Step 1: Client proposes a write

Add a method for the leader to receive client requests:

```rust,ignore
impl RaftNode {
    /// Called when a client sends a write to the leader.
    /// Returns messages to send to followers.
    pub fn propose(&mut self, command: Vec<u8>) -> Result<Vec<(NodeId, RaftMessage)>, String> {
        if self.state != NodeState::Leader {
            return Err("not the leader".to_string());
        }

        // Append to our log
        let index = self.log.append(self.current_term, command);
        println!(
            "[Node {}] Proposed entry {} in term {}",
            self.id, index, self.current_term
        );

        // Send to all followers
        Ok(self.send_append_entries())
    }
}
```

### Step 2: Run the full simulation

```rust,ignore
fn main() {
    println!("=== Raft Replication Simulation ===\n");

    // ... (create cluster, run election as in Chapter 14) ...

    // Find the leader
    let leader_id = nodes.iter()
        .find(|(_, n)| n.state == NodeState::Leader)
        .map(|(&id, _)| id)
        .expect("No leader elected!");

    // Client sends a write to the leader
    println!("\n--- Client sends: INSERT INTO users VALUES (1, 'Alice') ---\n");

    let messages = {
        let leader = nodes.get_mut(&leader_id).unwrap();
        leader.propose(b"INSERT INTO users VALUES (1, 'Alice')".to_vec())
            .unwrap()
    };

    // Deliver messages to followers
    let mut responses = Vec::new();
    for (to, msg) in messages {
        if let Some(node) = nodes.get_mut(&to) {
            let replies = node.handle_message(leader_id, msg);
            responses.extend(replies);
        }
    }

    // Deliver responses to leader
    for (to, msg) in responses {
        if let Some(node) = nodes.get_mut(&to) {
            node.handle_message(to, msg); // simplified
        }
    }

    // Check the leader's commit status
    let leader = nodes.get(&leader_id).unwrap();
    println!("\nLeader's log has {} entries", leader.log.len());
    println!("Commit index: {}", leader.commit_index);

    // Apply committed entries
    let leader = nodes.get_mut(&leader_id).unwrap();
    let applied = leader.apply_committed();
    for cmd in &applied {
        println!("Applied: {}", String::from_utf8_lossy(cmd));
    }
}
```

---

## Exercises

### Exercise A: Log Behind Scenario

Create a scenario where one follower is behind (has fewer log entries than others). Show that the leader's back-tracking mechanism catches it up.

<details>
<summary>Hint</summary>

Create a follower with an empty log while the leader has entries. The first `AppendEntries` will fail the consistency check. The leader decrements `next_index` and retries. Eventually, `next_index` reaches 1 and the leader sends the entire log.

</details>

### Exercise B: Heartbeat Counter

Add a counter that tracks how many heartbeats each follower has received. Print a summary every 10 ticks.

<details>
<summary>Hint</summary>

Add a `HashMap<NodeId, u64>` for heartbeat counts. Increment it each time a follower handles an `AppendEntries` (even an empty one).

</details>

### Exercise C: Multiple Writes

Propose three writes in sequence and verify that all three are replicated and committed. Print the complete log of every node at the end to verify they all match.

<details>
<summary>Hint</summary>

Call `propose()` three times, delivering messages between each. After all rounds, iterate through every node and print their log entries. All nodes should have the same three entries.

</details>

---

## Summary

You built the replication engine for Raft:

- **The Raft log** stores ordered entries that every node must replicate
- **`Arc<Mutex<T>>`** safely shares mutable data across concurrent tasks
- **`Arc`** provides shared ownership (like a library book with a checkout counter)
- **`Mutex`** provides exclusive access (like a bathroom with a lock)
- **AppendEntries RPC** sends entries from leader to followers with a consistency check
- **`next_index` and `match_index`** track each follower's progress
- **Commitment** happens when a majority of nodes have an entry
- **Back-tracking** catches up followers whose logs have diverged
- **Apply** converts committed entries into database operations

In the next chapter, we tackle the elephant in the room: what happens when a server crashes and restarts? Right now, all of this state is in memory. Turn off the power and everything is gone. Chapter 16 makes our Raft state durable -- written to disk so it survives crashes.
