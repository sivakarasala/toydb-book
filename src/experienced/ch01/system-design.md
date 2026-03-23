## System Design Corner: "Design a Key-Value Store"

This is a classic system design interview question. Your toydb is the simplest valid answer — and a good starting point for discussing tradeoffs.

### The spectrum of key-value stores

| Level | Example | Durability | Concurrency | Distribution |
|-------|---------|------------|-------------|-------------- |
| **In-memory, single-thread** | Your toydb (this chapter) | None — data lost on restart | None — single user | None — single machine |
| **In-memory, persistent** | Redis | AOF/RDB snapshots | Single-threaded event loop | Redis Cluster |
| **Disk-based, single-node** | RocksDB, LevelDB | LSM-tree + WAL | Multi-threaded with locks | Embedded (no network) |
| **Distributed** | DynamoDB, etcd, CockroachDB | Replicated WAL + Raft/Paxos | Sharded + replicated | Multi-node consensus |

### Key questions an interviewer expects you to address

1. **In-memory vs on-disk?** In-memory is fast but limited by RAM and loses data on crash. Disk-based is durable but slower. Most production systems use both — hot data in memory, everything on disk.

2. **How do you handle concurrent access?** Our toydb has a single `&mut self` — only one operation at a time. Redis solves this with a single-threaded event loop. RocksDB uses fine-grained locks. Distributed databases use consensus protocols (Raft, Paxos) to coordinate across nodes.

3. **How do you scale?** *Vertical scaling* means a bigger machine. *Horizontal scaling* means more machines. For horizontal scaling, you need to *partition* (shard) the keyspace — decide which keys live on which nodes. Consistent hashing is a common technique.

4. **What consistency guarantees?** Our toydb is trivially consistent — one copy, one thread. Distributed stores must choose between strong consistency (every read sees the latest write) and eventual consistency (reads might see stale data, but will catch up). This is the CAP theorem in practice.

> **Interview talking point:** *"I would start with an in-memory hash map for the prototype, add a write-ahead log for durability, then introduce sharding with consistent hashing for horizontal scaling. For replication, I would use Raft consensus to ensure strong consistency across replicas."* — This is exactly the architecture we will build across the 18 chapters of this book.

---
