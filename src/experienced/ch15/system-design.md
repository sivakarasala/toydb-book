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
