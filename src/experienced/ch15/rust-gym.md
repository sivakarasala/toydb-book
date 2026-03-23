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
