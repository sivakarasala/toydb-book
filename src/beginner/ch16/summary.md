## What You Built

In this chapter, you:

1. **Built a write-ahead log (WAL)** — an append-only file that records every change before it is applied, so nothing is lost on crash
2. **Persisted Raft metadata** — saved `current_term` and `voted_for` to disk using atomic file writes (write to temp file, then rename)
3. **Implemented crash recovery** — on startup, your node reads the WAL and metadata file to reconstruct its state, then rejoins the cluster as a follower
4. **Built snapshots** — capturing the full state machine at a point in time so old log entries can be discarded, keeping the WAL from growing forever
5. **Implemented InstallSnapshot** — when a follower is too far behind to catch up from the log, the leader sends a complete snapshot instead

Your Raft cluster now survives crashes. Stop a node, restart it, and it picks up where it left off. Kill every node, restart them all, and they re-elect a leader with all committed data intact.

---

### Key Rust concepts practiced

- **File handles as owned resources** — opening a file gives you ownership; dropping it closes it automatically
- **`&mut self` for exclusive access** — only one piece of code can write to the WAL at a time, enforced by the borrow checker
- **CRC32 checksums** — detecting corrupted or partially written records after a crash
