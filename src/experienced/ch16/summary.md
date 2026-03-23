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
