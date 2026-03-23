## System Design Corner: Durability Guarantees and Recovery

Durability is one of the ACID properties (Atomicity, Consistency, Isolation, **Durability**). It means: once a transaction is committed, its effects will not be lost, even if the system crashes. Let us examine how real systems achieve this.

### Levels of durability

| Level | Guarantee | Mechanism | Example |
|-------|-----------|-----------|---------|
| None | Data lost on crash | In-memory only | Redis without persistence |
| Process crash | Survives process crash | OS page cache | Most file writes without fsync |
| OS crash | Survives kernel panic | fsync to disk | PostgreSQL with `synchronous_commit=on` |
| Power loss | Survives power failure | fsync + battery-backed write cache | Enterprise databases on enterprise hardware |
| Disk failure | Survives disk death | Replication to multiple disks | Raft/Paxos with 3+ nodes |
| Datacenter failure | Survives site loss | Cross-datacenter replication | CockroachDB multi-region |

Our Raft cluster with fsync provides "disk failure" level durability — if any minority of nodes lose their disks, the remaining majority still has the data. This is why Raft requires a majority to commit: with 3 nodes, any 1 can fail; with 5 nodes, any 2 can fail.

### Write-ahead logging in production databases

PostgreSQL's WAL is the gold standard for single-node durability:

1. **Transaction writes:** All changes go to the WAL first, not to the data files
2. **Commit:** `fsync` the WAL (the write is now durable)
3. **Checkpoint:** Periodically apply WAL changes to the actual data files
4. **Recovery:** On crash, replay the WAL from the last checkpoint

This is exactly our pattern: WAL for durability, snapshots (checkpoints) for compaction. PostgreSQL's `pg_xlog` directory is its WAL. `pg_basebackup` creates snapshots. `pg_dump` is a logical snapshot.

### The fsync controversy

In 2018, PostgreSQL developers discovered that Linux's `fsync()` behavior had a dangerous edge case: if the kernel fails to write a dirty page to disk (due to a disk error), it marks the page as clean anyway. A subsequent `fsync()` call returns success because the page is "clean" — even though the data never reached the disk. This means **a single fsync failure can silently lose data**.

PostgreSQL 12 added handling for this case (retry the write rather than trusting the cached page). The lesson: even `fsync` is not as simple as it appears. Durability is a system-wide property that depends on the application, the OS, the filesystem, the disk controller, and the physical disk all behaving correctly.

> **Interview talking point:** *"Our Raft implementation provides durability through a write-ahead log with CRC checksums and fsync. State metadata uses atomic writes via temp file + rename + directory fsync. Snapshots compact old log entries to bound recovery time and disk usage. For crash recovery, we read the metadata file for term/vote, replay the WAL for log entries, and start as a follower. The key invariant is that the disk state is always at least as recent as any acknowledgment we sent — we write to disk before responding to RPCs."*

---
