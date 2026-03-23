## System Design Corner: Designing a Durable Key-Value Store

In a system design interview, you might hear: *"Design a persistent key-value store that survives crashes."* Here is how to structure your answer using what you built in this chapter.

### The durability spectrum

Not all "persistent" means the same thing:

| Level | Guarantee | Mechanism | Latency |
|-------|-----------|-----------|---------|
| No durability | Data lost on crash | In-memory only | Nanoseconds |
| OS-buffered | Data survives process crash, not power loss | `write()` + `flush()` | Microseconds |
| Disk-durable | Data survives power loss | `write()` + `flush()` + `fsync()` | Milliseconds |
| Replicated | Data survives disk failure | fsync + network replication | 10s of ms |

Our `LogStorage` with `sync_data()` is at level 3 — disk-durable. Level 4 requires replication, which we tackle in Chapters 14-16 with Raft consensus.

### Write-Ahead Logging (WAL)

The pattern we implemented — "write to a durable log before updating any in-memory state" — is called a **write-ahead log (WAL)**. It is the foundation of crash recovery in almost every database:

- **PostgreSQL** writes WAL records before modifying data pages
- **SQLite** uses either WAL mode or rollback journal mode
- **Redis** uses an append-only file (AOF) — nearly identical to our approach
- **LevelDB/RocksDB** write to a WAL before inserting into their in-memory memtable

The principle is simple: if the log is durable, you can always reconstruct the state by replaying it. The in-memory index is an optimization — it avoids scanning the log for every read — but the log is the source of truth.

### BitCask in the real world

Our implementation closely follows **BitCask**, the storage engine created by Basho for the Riak database. The original BitCask paper (2010) describes exactly what we built:

1. Append-only log for writes
2. In-memory hash index for reads
3. Tombstones for deletes
4. Periodic compaction to reclaim space

BitCask's constraint is that the entire key set must fit in memory (the index is a `HashMap`). For a dataset with billions of small keys, this can use gigabytes of RAM. Databases like LevelDB and RocksDB solve this with **LSM trees** (Log-Structured Merge trees), which keep parts of the index on disk using sorted files. LSM trees trade read performance for write performance — we will explore them in later chapters.

### Recovery Time Objective (RTO)

RTO is how long it takes to recover after a failure. For our `LogStorage`:

- **Process crash:** Restart the process, call `rebuild_index()`, done. RTO = time to scan the log file.
- **Disk failure:** Data is gone. RTO = time to restore from backup (if one exists).

The O(n) startup cost directly affects RTO. A 1 GB log file with 10 million records might take 5-10 seconds to scan. This is why hint files and compaction matter in production — they keep the log small and the recovery fast.

> **Interview talking point:** *"We use an append-only log for writes because it provides crash safety — if the process dies mid-write, only the last record is damaged, and we detect it with a CRC32 checksum. We trade disk space for write throughput, and manage space with periodic compaction. The in-memory hash index gives us O(1) reads, with the caveat that all keys must fit in memory. For datasets larger than memory, we would move to an LSM tree approach like LevelDB."*

---
