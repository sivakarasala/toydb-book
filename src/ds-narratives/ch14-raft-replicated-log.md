# Raft Replicated Log — "The append-only notebook everyone agrees on"

Your database is running on a single server. It is fast, consistent, and simple. Then the server's hard drive fails at 3 AM, and everything is gone. You set up a second server as a backup, replicating data from the primary. But now you have a new problem: the primary writes `SET x = 5`, sends it to the replica, then writes `SET x = 10`. The network hiccups, and the replica never gets the second write. The primary says `x = 10`, the replica says `x = 5`. Which is correct? What happens when the primary crashes and clients failover to the replica?

This is the **consensus problem**: how do multiple servers agree on a sequence of operations, even when servers crash and networks are unreliable? The answer, used by etcd, CockroachDB, and TiKV, is a **replicated log** -- and the most widely adopted algorithm for maintaining one is **Raft**.

---

## The Naive Way

The simplest replication strategy: the primary sends every write to the replica, fire-and-forget. No acknowledgment, no ordering guarantees:

```rust
fn main() {
    // Simulate primary and replica as separate logs
    let mut primary_log: Vec<String> = Vec::new();
    let mut replica_log: Vec<String> = Vec::new();

    // Operations arrive at the primary
    let operations = vec![
        "SET x = 1",
        "SET y = 2",
        "SET x = 3",
        "DELETE y",
        "SET z = 4",
    ];

    // Simulate unreliable network: some messages are lost
    let network_delivers = vec![true, true, false, true, false];

    for (i, op) in operations.iter().enumerate() {
        // Primary always records the operation
        primary_log.push(op.to_string());

        // Replica only gets it if the network delivers
        if network_delivers[i] {
            replica_log.push(op.to_string());
        }
    }

    println!("Primary log ({} entries):", primary_log.len());
    for (i, entry) in primary_log.iter().enumerate() {
        println!("  [{}] {}", i, entry);
    }

    println!("\nReplica log ({} entries):", replica_log.len());
    for (i, entry) in replica_log.iter().enumerate() {
        println!("  [{}] {}", i, entry);
    }

    println!("\nProblem: logs have diverged!");
    println!("Primary has {} entries, replica has {}", primary_log.len(), replica_log.len());
    println!("If primary crashes, replica is missing operations.");
    println!("If we promote the replica, data is lost silently.");
}
```

The fire-and-forget approach has three fatal flaws:

1. **Lost writes**: network failures cause the replica to miss operations. The replica thinks it is up to date, but it is not.
2. **No ordering guarantee**: even if messages arrive, they might arrive out of order (via different network paths). `SET x = 3` might arrive before `SET x = 1`.
3. **Split brain**: if the primary is unreachable (but not actually dead), both servers might accept writes independently, and their logs diverge permanently.

---

## The Insight

Picture a courtroom where three clerks keep an official record of proceedings. The judge speaks, and all three clerks write down what was said. But what if one clerk misheard, or their pen ran out of ink mid-sentence?

The solution: the judge does not move on until at least two out of three clerks confirm they have written the statement correctly. If the judge says "Motion denied" and clerks A and B confirm, but clerk C was refilling ink, the court record is still valid -- two out of three agree, which is a **majority**. When clerk C comes back, they can copy the record from A or B.

The key principle: **an operation is committed when a majority of servers have recorded it**. A majority of N servers is `(N/2) + 1`. For 3 servers, that is 2. For 5 servers, that is 3. The magic of majority quorums: any two majorities overlap by at least one server. So no matter which servers you ask, at least one of them has seen the latest committed operation.

This is the foundation of Raft's replicated log:

1. **One leader** accepts all client writes.
2. The leader **appends the operation to its log** and sends it to all followers.
3. When a **majority of servers** have the operation in their logs, it is **committed**.
4. Committed operations are applied to the state machine (the database) in log order.
5. If the leader crashes, a new leader is elected, and it has all committed operations (because it was part of every majority quorum).

The log is the source of truth. The database state is just the result of replaying the log from the beginning. Two servers with identical logs will have identical database states.

---

## The Build

### Log Entry Structure

Each entry in the Raft log contains three things: the **term** (the election period during which it was created), the **index** (its position in the log), and the **command** (the operation to apply):

```rust,ignore
#[derive(Debug, Clone, PartialEq)]
struct LogEntry {
    term: u64,    // the election term when this entry was created
    index: u64,   // position in the log (1-based)
    command: String, // the operation, e.g., "SET x = 5"
}
```

The term number is crucial. It monotonically increases with each new election. If a server sees a log entry with a higher term than its own, it knows that entry was created by a more recent leader. Terms act as a logical clock that helps detect stale leaders.

### The Raft Log

The log itself tracks entries and the commit index -- the highest index known to be committed (replicated to a majority):

```rust,ignore
struct RaftLog {
    entries: Vec<LogEntry>,
    commit_index: u64, // highest index committed (replicated to majority)
}

impl RaftLog {
    fn new() -> Self {
        RaftLog {
            entries: Vec::new(),
            commit_index: 0,
        }
    }

    /// Append a new entry to the log. Returns the new entry's index.
    fn append(&mut self, term: u64, command: String) -> u64 {
        let index = self.entries.len() as u64 + 1; // 1-based indexing
        self.entries.push(LogEntry { term, index, command });
        index
    }

    /// Get the entry at a given index (1-based).
    fn get(&self, index: u64) -> Option<&LogEntry> {
        if index == 0 || index as usize > self.entries.len() {
            None
        } else {
            Some(&self.entries[(index - 1) as usize])
        }
    }

    /// Get the last log index and term.
    fn last_index_term(&self) -> (u64, u64) {
        match self.entries.last() {
            Some(entry) => (entry.index, entry.term),
            None => (0, 0),
        }
    }

    /// Get all entries from start_index onward (for replication).
    fn entries_from(&self, start_index: u64) -> &[LogEntry] {
        if start_index == 0 || start_index as usize > self.entries.len() {
            &[]
        } else {
            &self.entries[(start_index - 1) as usize..]
        }
    }

    /// Advance the commit index.
    fn commit_to(&mut self, index: u64) {
        if index > self.commit_index && index <= self.entries.len() as u64 {
            self.commit_index = index;
        }
    }

    /// Get all committed but not yet applied entries.
    fn committed_entries(&self, last_applied: u64) -> &[LogEntry] {
        if last_applied >= self.commit_index {
            &[]
        } else {
            let start = last_applied as usize;
            let end = self.commit_index as usize;
            &self.entries[start..end]
        }
    }
}
```

### The Consistency Check: Matching Previous Entry

When the leader sends new entries to a follower, it also sends the **index and term of the entry immediately preceding the new ones**. The follower checks: "do I have an entry at that index with that term?" If yes, the logs match up to that point, and the new entries can be safely appended. If no, the logs have diverged, and the follower rejects the request.

This is how Raft detects and repairs inconsistencies:

```rust,ignore
impl RaftLog {
    /// Check if our log matches the leader's at the given point.
    /// This is the consistency check in AppendEntries.
    fn matches_at(&self, prev_index: u64, prev_term: u64) -> bool {
        if prev_index == 0 {
            return true; // empty log always matches
        }

        match self.get(prev_index) {
            Some(entry) => entry.term == prev_term,
            None => false, // we don't have an entry at that index
        }
    }

    /// Append entries from the leader, truncating any conflicting entries.
    /// This handles the case where the follower has stale entries from
    /// a previous leader that never committed.
    fn append_entries(&mut self, prev_index: u64, entries: &[LogEntry]) -> bool {
        // Check each new entry against existing entries
        for entry in entries {
            let idx = entry.index as usize;
            if idx <= self.entries.len() {
                // We have an entry at this index
                let existing = &self.entries[idx - 1];
                if existing.term != entry.term {
                    // Conflict! Truncate from here onward
                    self.entries.truncate(idx - 1);
                    self.entries.push(entry.clone());
                }
                // If terms match, entry is already correct -- skip it
            } else {
                // We don't have an entry at this index -- append
                self.entries.push(entry.clone());
            }
        }
        true
    }
}
```

The truncation step is subtle but essential. Suppose server A was leader in term 3 and appended `[3: SET x=5]` to its log but crashed before committing. Server B becomes leader in term 4 and appends `[4: SET y=7]` at the same index. When A comes back as a follower, it has a stale entry. The new leader sends the correct entry with `prev_index` and `prev_term`, A sees the mismatch, truncates the stale entry, and accepts the new one. The uncommitted entry is lost -- and that is correct, because it was never committed.

### Simulating Replication

Let's simulate a 3-node cluster with a leader replicating to two followers:

```rust,ignore
struct ReplicaState {
    log: RaftLog,
    next_index: u64, // next index to send to this follower
    match_index: u64, // highest index known to be replicated
}

struct Leader {
    log: RaftLog,
    current_term: u64,
    replicas: Vec<ReplicaState>,
    cluster_size: usize,
}

impl Leader {
    fn new(cluster_size: usize) -> Self {
        let mut replicas = Vec::new();
        for _ in 0..(cluster_size - 1) {
            replicas.push(ReplicaState {
                log: RaftLog::new(),
                next_index: 1,
                match_index: 0,
            });
        }

        Leader {
            log: RaftLog::new(),
            current_term: 1,
            replicas,
            cluster_size,
        }
    }

    /// Client submits a command. Leader appends it and replicates.
    fn propose(&mut self, command: String) -> u64 {
        let index = self.log.append(self.current_term, command);
        index
    }

    /// Send new entries to a follower. Returns true if the follower accepted.
    fn replicate_to(&mut self, follower_id: usize) -> bool {
        let replica = &mut self.replicas[follower_id];
        let next = replica.next_index;

        // Get the previous entry's index and term for consistency check
        let (prev_index, prev_term) = if next > 1 {
            match self.log.get(next - 1) {
                Some(entry) => (entry.index, entry.term),
                None => (0, 0),
            }
        } else {
            (0, 0)
        };

        // Get entries to send
        let entries = self.log.entries_from(next).to_vec();
        if entries.is_empty() {
            return true; // nothing to replicate
        }

        // Simulate the follower's response
        if replica.log.matches_at(prev_index, prev_term) {
            replica.log.append_entries(prev_index, &entries);
            let last_new_index = entries.last().map(|e| e.index).unwrap_or(next);
            replica.next_index = last_new_index + 1;
            replica.match_index = last_new_index;
            true
        } else {
            // Follower rejected -- decrement next_index and retry
            if replica.next_index > 1 {
                replica.next_index -= 1;
            }
            false
        }
    }

    /// Check if any new entries can be committed (majority replication).
    fn advance_commit_index(&mut self) {
        let majority = self.cluster_size / 2 + 1;

        // For each index from commit_index+1 onward, check if a majority has it
        let (last_index, _) = self.log.last_index_term();
        for index in (self.log.commit_index + 1)..=last_index {
            // Count how many servers have this entry
            // The leader always has it (count starts at 1)
            let mut replication_count = 1;
            for replica in &self.replicas {
                if replica.match_index >= index {
                    replication_count += 1;
                }
            }

            // Only commit entries from the current term
            // (Raft's safety requirement -- see Section 5.4.2 of the paper)
            if let Some(entry) = self.log.get(index) {
                if replication_count >= majority && entry.term == self.current_term {
                    self.log.commit_to(index);
                }
            }
        }
    }
}
```

### Applying Committed Entries to the State Machine

Once an entry is committed, it is safe to apply -- it will never be rolled back. Applying means executing the command against the actual database state:

```rust,ignore
use std::collections::HashMap;

struct StateMachine {
    data: HashMap<String, String>,
    last_applied: u64,
}

impl StateMachine {
    fn new() -> Self {
        StateMachine {
            data: HashMap::new(),
            last_applied: 0,
        }
    }

    fn apply(&mut self, entry: &LogEntry) {
        // Parse simple SET/DELETE commands
        let parts: Vec<&str> = entry.command.splitn(3, ' ').collect();
        match parts.as_slice() {
            ["SET", key_val @ ..] => {
                let kv: Vec<&str> = key_val.join(" ")
                    .splitn(2, '=')
                    .map(|s| s.trim().to_string())
                    .collect::<Vec<String>>()
                    .into_iter()
                    .collect();
                if kv.len() == 2 {
                    self.data.insert(kv[0].clone(), kv[1].clone());
                }
            }
            ["DELETE", key] => {
                self.data.remove(*key);
            }
            _ => {} // unknown command
        }
        self.last_applied = entry.index;
    }
}
```

---

## The Payoff

Let's run the full simulation -- a leader proposing commands, replicating to followers, and committing when a majority acknowledges:

```rust
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
struct LogEntry {
    term: u64,
    index: u64,
    command: String,
}

struct RaftLog {
    entries: Vec<LogEntry>,
    commit_index: u64,
}

impl RaftLog {
    fn new() -> Self {
        RaftLog { entries: Vec::new(), commit_index: 0 }
    }

    fn append(&mut self, term: u64, command: String) -> u64 {
        let index = self.entries.len() as u64 + 1;
        self.entries.push(LogEntry { term, index, command });
        index
    }

    fn get(&self, index: u64) -> Option<&LogEntry> {
        if index == 0 || index as usize > self.entries.len() { None }
        else { Some(&self.entries[(index - 1) as usize]) }
    }

    fn last_index_term(&self) -> (u64, u64) {
        self.entries.last().map(|e| (e.index, e.term)).unwrap_or((0, 0))
    }

    fn entries_from(&self, start: u64) -> Vec<LogEntry> {
        if start == 0 || start as usize > self.entries.len() { vec![] }
        else { self.entries[(start - 1) as usize..].to_vec() }
    }

    fn commit_to(&mut self, index: u64) {
        if index > self.commit_index && index <= self.entries.len() as u64 {
            self.commit_index = index;
        }
    }

    fn matches_at(&self, prev_index: u64, prev_term: u64) -> bool {
        if prev_index == 0 { return true; }
        self.get(prev_index).map(|e| e.term == prev_term).unwrap_or(false)
    }

    fn append_entries(&mut self, _prev_index: u64, entries: &[LogEntry]) {
        for entry in entries {
            let idx = entry.index as usize;
            if idx <= self.entries.len() {
                if self.entries[idx - 1].term != entry.term {
                    self.entries.truncate(idx - 1);
                    self.entries.push(entry.clone());
                }
            } else {
                self.entries.push(entry.clone());
            }
        }
    }
}

struct Follower {
    id: usize,
    log: RaftLog,
    next_index: u64,
    match_index: u64,
    online: bool,
}

fn main() {
    let mut leader_log = RaftLog::new();
    let current_term = 1u64;
    let cluster_size = 3usize;
    let majority = cluster_size / 2 + 1;

    let mut followers = vec![
        Follower { id: 1, log: RaftLog::new(), next_index: 1, match_index: 0, online: true },
        Follower { id: 2, log: RaftLog::new(), next_index: 1, match_index: 0, online: true },
    ];

    let commands = vec![
        "SET x = 1",
        "SET y = 2",
        "SET x = 3",
        "DELETE y",
        "SET z = 42",
    ];

    println!("=== Raft Replicated Log Simulation ===");
    println!("Cluster: 1 leader + 2 followers (majority = {})\n", majority);

    for (round, cmd) in commands.iter().enumerate() {
        println!("--- Round {} ---", round + 1);
        println!("Client proposes: {}", cmd);

        // Leader appends to its own log
        let index = leader_log.append(current_term, cmd.to_string());
        println!("Leader appended at index {}", index);

        // Simulate follower 2 going offline for round 3
        if round == 2 {
            followers[1].online = false;
            println!("  ** Follower 2 goes OFFLINE **");
        }
        if round == 4 {
            followers[1].online = true;
            println!("  ** Follower 2 comes back ONLINE **");
        }

        // Replicate to each follower
        let mut replication_count = 1; // leader counts as 1
        for follower in &mut followers {
            if !follower.online {
                println!("  Follower {}: OFFLINE (skipped)", follower.id);
                continue;
            }

            let next = follower.next_index;
            let (prev_index, prev_term) = if next > 1 {
                leader_log.get(next - 1).map(|e| (e.index, e.term)).unwrap_or((0, 0))
            } else {
                (0, 0)
            };

            let entries = leader_log.entries_from(next);
            if entries.is_empty() {
                replication_count += 1;
                continue;
            }

            if follower.log.matches_at(prev_index, prev_term) {
                follower.log.append_entries(prev_index, &entries);
                let last_idx = entries.last().unwrap().index;
                follower.next_index = last_idx + 1;
                follower.match_index = last_idx;
                replication_count += 1;
                println!("  Follower {}: accepted (log now has {} entries)",
                         follower.id, follower.log.entries.len());
            } else {
                follower.next_index -= 1;
                println!("  Follower {}: REJECTED (log mismatch)", follower.id);
            }
        }

        // Check if we can commit
        if replication_count >= majority {
            leader_log.commit_to(index);
            println!("  COMMITTED at index {} ({}/{} servers)",
                     index, replication_count, cluster_size);
        } else {
            println!("  NOT committed ({}/{} servers, need {})",
                     replication_count, cluster_size, majority);
        }
        println!();
    }

    // Apply committed entries to state machine
    println!("=== State Machine After Replay ===");
    let mut state: HashMap<String, String> = HashMap::new();
    for i in 1..=leader_log.commit_index {
        if let Some(entry) = leader_log.get(i) {
            let parts: Vec<&str> = entry.command.split_whitespace().collect();
            match parts.as_slice() {
                ["SET", key, "=", value] => {
                    state.insert(key.to_string(), value.to_string());
                    println!("  Applied [{}]: {} -> {}", i, key, value);
                }
                ["DELETE", key] => {
                    state.remove(*key);
                    println!("  Applied [{}]: DELETE {}", i, key);
                }
                _ => {}
            }
        }
    }

    println!("\nFinal state:");
    let mut keys: Vec<&String> = state.keys().collect();
    keys.sort();
    for key in keys {
        println!("  {} = {}", key, state[key]);
    }

    println!("\nLog status:");
    println!("  Leader: {} entries, committed through {}", leader_log.entries.len(), leader_log.commit_index);
    for follower in &followers {
        println!("  Follower {}: {} entries, match_index {}{}",
                 follower.id, follower.log.entries.len(), follower.match_index,
                 if !follower.online { " (was offline)" } else { "" });
    }
}
```

The simulation shows the critical property: even when follower 2 goes offline, operations continue to commit because the leader plus follower 1 still form a majority. When follower 2 comes back, it catches up by receiving all the entries it missed.

---

## Complexity Table

| Operation | Cost | Notes |
|-----------|------|-------|
| Propose (leader) | O(1) log append + O(N) network sends | N = cluster size |
| Commit decision | O(N) check match indices | Find majority |
| Apply | O(1) per entry | Sequential state machine application |
| Follower catch-up | O(k) entries | k = number of missed entries |
| Log conflict resolution | O(k) truncate + resend | k = conflicting entries (usually small) |
| Leader election | O(N) vote requests | See companion chapter on elections |
| Reads (linearizable) | O(N) heartbeat or read index | Must confirm leadership |
| Space per entry | O(1) | term + index + command |
| Recovery from crash | O(n) log replay | n = total log entries |

The key insight: **writes scale with cluster size (more nodes = more network round trips), but the algorithm remains correct even with minority failures.** A 5-node cluster tolerates 2 failures, a 7-node cluster tolerates 3. The trade-off is write latency (more nodes to wait for) versus fault tolerance (more nodes can fail).

---

## Where This Shows Up in Our Database

In Chapter 14, we add replication to our database using a simplified Raft protocol. The replicated log sits between the client API and the storage engine:

```rust,ignore
pub struct ReplicatedDb {
    raft_log: RaftLog,
    state_machine: Database,
    // ...
}

impl ReplicatedDb {
    pub fn execute(&mut self, command: String) -> Result<()> {
        // Step 1: Append to log
        let index = self.raft_log.append(self.current_term, command);

        // Step 2: Replicate to followers (wait for majority)
        self.replicate_and_wait(index)?;

        // Step 3: Apply to state machine
        self.apply_committed_entries();
        Ok(())
    }
}
```

Beyond our toydb, replicated logs power the most critical infrastructure in production:

- **etcd** (Kubernetes' brain) uses Raft to replicate its key-value store. Every cluster configuration change, pod scheduling decision, and service discovery entry goes through the Raft log. If etcd's Raft breaks, your Kubernetes cluster stops working.
- **CockroachDB** runs a Raft group per range (a contiguous chunk of the keyspace). A cluster might have thousands of independent Raft groups, each maintaining its own replicated log for its portion of the data.
- **TiKV** (the storage layer of TiDB) also uses per-range Raft groups. It is one of the most performant Raft implementations, written in Rust.
- **Kafka** adopted KRaft (Kafka Raft) to replace ZooKeeper for metadata management. The controller quorum uses a Raft-like replicated log to track topic configurations, partition assignments, and broker membership.

The replicated log is not just a replication mechanism. It is a **total ordering primitive**. Once all servers agree on the same sequence of operations, any deterministic computation on that sequence produces the same result on every server. This is why Raft and Paxos are at the foundation of every serious distributed database.

---

## Try It Yourself

### Exercise 1: Log Compaction with Snapshots

The Raft log grows forever. Implement a snapshot mechanism: periodically capture the state machine's state, save it, and truncate the log up to the snapshot index. When a follower is far behind, send it the snapshot instead of replaying thousands of log entries.

<details>
<summary>Solution</summary>

```rust
use std::collections::HashMap;

#[derive(Debug, Clone)]
struct LogEntry {
    term: u64,
    index: u64,
    command: String,
}

#[derive(Debug, Clone)]
struct Snapshot {
    last_index: u64,
    last_term: u64,
    state: HashMap<String, String>,
}

struct CompactableLog {
    entries: Vec<LogEntry>,
    offset: u64, // index of the first entry in `entries` (entries before this were compacted)
    commit_index: u64,
    snapshot: Option<Snapshot>,
}

impl CompactableLog {
    fn new() -> Self {
        CompactableLog {
            entries: Vec::new(),
            offset: 0,
            commit_index: 0,
            snapshot: None,
        }
    }

    fn append(&mut self, term: u64, command: String) -> u64 {
        let index = self.offset + self.entries.len() as u64 + 1;
        self.entries.push(LogEntry { term, index, command });
        index
    }

    fn get(&self, index: u64) -> Option<&LogEntry> {
        if index <= self.offset || index > self.offset + self.entries.len() as u64 {
            None
        } else {
            Some(&self.entries[(index - self.offset - 1) as usize])
        }
    }

    fn compact(&mut self, state: &HashMap<String, String>, through_index: u64) {
        if through_index <= self.offset || through_index > self.commit_index {
            return;
        }

        let entry = self.get(through_index).unwrap();
        let snapshot = Snapshot {
            last_index: through_index,
            last_term: entry.term,
            state: state.clone(),
        };

        // Remove entries up to and including through_index
        let entries_to_remove = (through_index - self.offset) as usize;
        self.entries.drain(..entries_to_remove);
        self.offset = through_index;
        self.snapshot = Some(snapshot);
    }

    fn total_entries(&self) -> usize {
        self.entries.len()
    }
}

fn apply_command(state: &mut HashMap<String, String>, command: &str) {
    let parts: Vec<&str> = command.split_whitespace().collect();
    match parts.as_slice() {
        ["SET", key, "=", value] => { state.insert(key.to_string(), value.to_string()); }
        ["DELETE", key] => { state.remove(*key); }
        _ => {}
    }
}

fn main() {
    let mut log = CompactableLog::new();
    let mut state: HashMap<String, String> = HashMap::new();

    // Append and apply 20 operations
    for i in 1..=20u64 {
        let cmd = format!("SET key{} = value{}", i % 5, i);
        let index = log.append(1, cmd.clone());
        log.commit_index = index;
        apply_command(&mut state, &cmd);
    }

    println!("Before compaction:");
    println!("  Log entries: {}", log.total_entries());
    println!("  State: {:?}", state);
    println!("  Offset: {}", log.offset);

    // Compact through index 15
    log.compact(&state, 15);

    println!("\nAfter compacting through index 15:");
    println!("  Log entries: {}", log.total_entries());
    println!("  Offset: {}", log.offset);
    println!("  Snapshot: index={}, term={}",
             log.snapshot.as_ref().unwrap().last_index,
             log.snapshot.as_ref().unwrap().last_term);
    println!("  Snapshot state: {:?}", log.snapshot.as_ref().unwrap().state);

    // Remaining entries should be 16-20
    println!("\n  Remaining log entries:");
    for i in 16..=20 {
        if let Some(entry) = log.get(i) {
            println!("    [{}: term={}] {}", entry.index, entry.term, entry.command);
        }
    }

    // Entry 10 is gone (compacted)
    println!("\n  Entry 10: {:?}", log.get(10));
    println!("  Entry 16: {:?}", log.get(16).map(|e| &e.command));
}
```

</details>

### Exercise 2: Idempotent Client Requests

If a client sends a request and the response is lost, the client retries. But the command might have already been committed and applied. Implement a client request deduplication mechanism using client IDs and sequence numbers so that retried requests are not applied twice.

<details>
<summary>Solution</summary>

```rust
use std::collections::HashMap;

#[derive(Debug, Clone)]
struct ClientRequest {
    client_id: u64,
    sequence_num: u64,
    command: String,
}

#[derive(Debug, Clone)]
struct ClientResponse {
    success: bool,
    result: String,
}

struct DedupStateMachine {
    data: HashMap<String, String>,
    // Track the last processed sequence number and response for each client
    client_sessions: HashMap<u64, (u64, ClientResponse)>,
}

impl DedupStateMachine {
    fn new() -> Self {
        DedupStateMachine {
            data: HashMap::new(),
            client_sessions: HashMap::new(),
        }
    }

    fn apply(&mut self, request: &ClientRequest) -> ClientResponse {
        // Check for duplicate request
        if let Some((last_seq, last_response)) = self.client_sessions.get(&request.client_id) {
            if request.sequence_num <= *last_seq {
                // Duplicate! Return the cached response
                println!("  [DEDUP] Client {} seq {} is duplicate (last processed: {})",
                         request.client_id, request.sequence_num, last_seq);
                return last_response.clone();
            }
        }

        // Not a duplicate -- apply the command
        let response = self.execute_command(&request.command);

        // Cache the response for deduplication
        self.client_sessions.insert(
            request.client_id,
            (request.sequence_num, response.clone()),
        );

        response
    }

    fn execute_command(&mut self, command: &str) -> ClientResponse {
        let parts: Vec<&str> = command.split_whitespace().collect();
        match parts.as_slice() {
            ["SET", key, "=", value] => {
                self.data.insert(key.to_string(), value.to_string());
                ClientResponse {
                    success: true,
                    result: format!("OK: {} = {}", key, value),
                }
            }
            ["GET", key] => {
                let value = self.data.get(*key).cloned().unwrap_or_else(|| "(nil)".to_string());
                ClientResponse {
                    success: true,
                    result: value,
                }
            }
            ["DELETE", key] => {
                let existed = self.data.remove(*key).is_some();
                ClientResponse {
                    success: true,
                    result: format!("Deleted: {}", existed),
                }
            }
            _ => ClientResponse {
                success: false,
                result: "Unknown command".to_string(),
            },
        }
    }
}

fn main() {
    let mut sm = DedupStateMachine::new();

    let requests = vec![
        ClientRequest { client_id: 1, sequence_num: 1, command: "SET x = 10".to_string() },
        ClientRequest { client_id: 1, sequence_num: 2, command: "SET y = 20".to_string() },
        // Client 1 retries sequence 2 (response was lost)
        ClientRequest { client_id: 1, sequence_num: 2, command: "SET y = 20".to_string() },
        // Client 2 sends independently
        ClientRequest { client_id: 2, sequence_num: 1, command: "SET z = 30".to_string() },
        // Client 1 sends next request
        ClientRequest { client_id: 1, sequence_num: 3, command: "GET x".to_string() },
        // Client 1 retries an OLD sequence (stale retry)
        ClientRequest { client_id: 1, sequence_num: 1, command: "SET x = 10".to_string() },
    ];

    println!("=== Processing requests with deduplication ===\n");
    for req in &requests {
        println!("Request: client={}, seq={}, cmd='{}'",
                 req.client_id, req.sequence_num, req.command);
        let response = sm.apply(req);
        println!("  Response: {}\n", response.result);
    }

    println!("Final state: {:?}", sm.data);
    println!("Client sessions tracked: {}", sm.client_sessions.len());
}
```

</details>

### Exercise 3: Read-Only Optimization

Linearizable reads in Raft normally require a log round trip (append a no-op, wait for commit). Implement a "read index" optimization: the leader records the current commit index, confirms it is still leader by sending heartbeats to a majority, then serves the read at that commit index. This gives linearizable reads without a log entry.

<details>
<summary>Solution</summary>

```rust
use std::collections::HashMap;

#[derive(Debug, Clone)]
struct LogEntry {
    term: u64,
    index: u64,
    command: String,
}

struct SimplifiedRaftNode {
    log: Vec<LogEntry>,
    commit_index: u64,
    current_term: u64,
    state: HashMap<String, String>,
    last_applied: u64,
    is_leader: bool,
    follower_acks: Vec<bool>, // simulate heartbeat acks
}

impl SimplifiedRaftNode {
    fn new(is_leader: bool) -> Self {
        SimplifiedRaftNode {
            log: Vec::new(),
            commit_index: 0,
            current_term: 1,
            state: HashMap::new(),
            last_applied: 0,
            is_leader,
            follower_acks: vec![false; 2], // 2 followers
        }
    }

    fn append_and_commit(&mut self, command: &str) {
        let index = self.log.len() as u64 + 1;
        self.log.push(LogEntry {
            term: self.current_term,
            index,
            command: command.to_string(),
        });
        self.commit_index = index;

        // Apply
        let parts: Vec<&str> = command.split_whitespace().collect();
        if let ["SET", key, "=", value] = parts.as_slice() {
            self.state.insert(key.to_string(), value.to_string());
        }
        self.last_applied = index;
    }

    /// Read-only optimization: ReadIndex
    /// 1. Record the current commit index
    /// 2. Confirm leadership (heartbeat majority)
    /// 3. Wait until state machine has applied through the read index
    /// 4. Serve the read
    fn read_with_read_index(&mut self, key: &str) -> Result<Option<String>, String> {
        if !self.is_leader {
            return Err("Not the leader".to_string());
        }

        // Step 1: Record the read index
        let read_index = self.commit_index;
        println!("  ReadIndex: read_index = {}", read_index);

        // Step 2: Confirm leadership by sending heartbeats
        let acks = self.send_heartbeats();
        let majority = 2; // 3-node cluster, need 2 (leader + 1 follower)
        let total_acks = 1 + acks; // leader counts itself

        if total_acks < majority {
            return Err(format!(
                "Lost leadership: only {} acks, need {}", total_acks, majority
            ));
        }
        println!("  Leadership confirmed ({}/{} acks)", total_acks, 3);

        // Step 3: Ensure state machine has applied through read_index
        while self.last_applied < read_index {
            // Apply pending entries
            let idx = self.last_applied as usize;
            let entry = &self.log[idx];
            let parts: Vec<&str> = entry.command.split_whitespace().collect();
            if let ["SET", key, "=", value] = parts.as_slice() {
                self.state.insert(key.to_string(), value.to_string());
            }
            self.last_applied += 1;
        }

        // Step 4: Serve the read
        Ok(self.state.get(key).cloned())
    }

    /// The slow way: read via log
    fn read_via_log(&mut self, key: &str) -> Result<Option<String>, String> {
        if !self.is_leader {
            return Err("Not the leader".to_string());
        }

        // Append a no-op to the log
        let index = self.log.len() as u64 + 1;
        self.log.push(LogEntry {
            term: self.current_term,
            index,
            command: "NOOP".to_string(),
        });
        println!("  Log read: appended NOOP at index {}", index);

        // Wait for commit (simulate)
        self.commit_index = index;
        self.last_applied = index;
        println!("  Log read: committed and applied through {}", index);

        Ok(self.state.get(key).cloned())
    }

    fn send_heartbeats(&mut self) -> usize {
        // Simulate: both followers respond
        self.follower_acks = vec![true, true];
        self.follower_acks.iter().filter(|&&ack| ack).count()
    }
}

fn main() {
    let mut node = SimplifiedRaftNode::new(true);

    // Write some data
    node.append_and_commit("SET user = Alice");
    node.append_and_commit("SET score = 100");
    node.append_and_commit("SET level = 5");

    println!("=== Read-Only Optimization: ReadIndex ===\n");
    println!("Log has {} entries, commit_index = {}\n",
             node.log.len(), node.commit_index);

    // Method 1: Read via log (slow -- adds a log entry)
    println!("Method 1: Read via log round trip");
    let log_size_before = node.log.len();
    match node.read_via_log("user") {
        Ok(val) => println!("  Result: {:?}", val),
        Err(e) => println!("  Error: {}", e),
    }
    println!("  Log grew from {} to {} entries (added NOOP)\n",
             log_size_before, node.log.len());

    // Method 2: ReadIndex (fast -- no log entry)
    println!("Method 2: ReadIndex optimization");
    let log_size_before = node.log.len();
    match node.read_with_read_index("score") {
        Ok(val) => println!("  Result: {:?}", val),
        Err(e) => println!("  Error: {}", e),
    }
    println!("  Log still has {} entries (no NOOP added)\n", node.log.len());
    assert_eq!(node.log.len(), log_size_before, "ReadIndex should not grow the log");

    println!("=== Comparison ===");
    println!("Read via log:   1 log append + 1 replication round trip + 1 apply");
    println!("ReadIndex:      1 heartbeat round trip (no log growth)");
    println!("Savings:        No log entry, no disk write, no replication");
    println!("Trade-off:      Same latency (1 round trip), but less I/O");
}
```

</details>

---

## Recap

A replicated log ensures multiple servers agree on the same sequence of operations. The leader appends entries to its log, replicates them to followers, and commits an entry when a majority of servers have it. Committed entries are never lost -- any future leader must have them because it was part of a majority quorum.

The log's consistency check (matching previous entry's index and term) detects divergence between leader and follower, and the follower truncates conflicting entries. This guarantees that two servers with the same log prefix will converge to identical states.

The beauty of the replicated log is separation of concerns: the consensus algorithm (Raft) handles agreement on the log's contents, and the state machine (your database) handles interpreting those contents. Any deterministic computation replayed against the same log sequence produces the same result. This is why the replicated log is the primitive that distributed databases are built on.
