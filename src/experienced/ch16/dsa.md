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
