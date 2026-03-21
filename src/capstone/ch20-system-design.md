# Chapter 20: System Design Deep Dive

This chapter is the system design capstone. It takes four questions that commonly appear in system design interviews and answers each one using the distributed database domain — the domain you have been building in for eighteen chapters. Every section follows the same format you would use in a 35-45 minute interview: clarify requirements, estimate capacity, sketch a high-level design, deep-dive into critical components, discuss scaling, and call out trade-offs.

Read each section as a standalone interview walkthrough. Practice explaining them out loud to a whiteboard or a friend. The goal is not to memorize answers but to internalize the *process* — requirements first, then estimation, then design, then depth.

---

## 20.1 Design a Distributed SQL Database

### The Interview Question

> "Design a distributed SQL database like CockroachDB or TiDB. It should support standard SQL queries, horizontal scaling across many servers, and strong consistency guarantees."

### Requirements Gathering

Before drawing a single box, ask clarifying questions:

- **Query language.** Full SQL or a subset? Joins, subqueries, aggregations? *Full SQL including multi-table joins and transactions.*
- **Consistency model.** Eventual consistency or strong consistency (linearizability)? *Strong consistency — reads always see the latest committed writes.*
- **Transactions.** Single-key or multi-key? Cross-shard? *Full ACID transactions spanning multiple keys on multiple nodes.*
- **Scale target.** How much data? How many queries per second? *Terabytes of data, thousands of queries per second.*
- **Failure tolerance.** How many simultaneous node failures must the system survive? *Any single node failure, with no data loss.*
- **Geographic distribution.** Single data center or multi-region? *Start with single data center, design for multi-region expansion.*

### Capacity Estimation

| Metric | Value |
|--------|-------|
| Total data size | 10 TB |
| Number of nodes | 30 (each with 500 GB SSD, 64 GB RAM) |
| Data per node | ~333 GB (with 3x replication: ~1 TB raw per node) |
| Peak queries/sec | 10,000 reads, 2,000 writes |
| Average row size | 500 bytes |
| Total rows | ~20 billion |
| Raft groups | ~3,000 (each managing ~3 GB of data) |
| Latency target | p50 < 5ms reads, p50 < 20ms writes |

### High-Level Design

```
┌──────────────────────────────────────────────────┐
│                  SQL Clients                      │
│           (PostgreSQL wire protocol)              │
└──────────────────┬───────────────────────────────┘
                   │
┌──────────────────▼───────────────────────────────┐
│              Gateway Layer                        │
│    (Connection handling, authentication,          │
│     query routing, transaction coordination)      │
├──────────────────────────────────────────────────┤
│              SQL Layer                            │
│    (Parser, Planner, Optimizer, Executor)          │
│    (Distributed query planning, cost estimation)   │
├──────────────────────────────────────────────────┤
│           Transaction Layer                       │
│    (Distributed transactions, 2PC coordinator,    │
│     MVCC, timestamp oracle)                       │
├──────────────────────────────────────────────────┤
│           Distribution Layer                      │
│    (Range partitioning, routing table,            │
│     Raft group management, rebalancing)           │
├──────────────────────────────────────────────────┤
│           Storage Layer                           │
│    (LSM tree / B-tree per node,                   │
│     Raft log, snapshots)                          │
├──────────────────────────────────────────────────┤
│           Physical Storage                        │
│    (SSD / HDD, filesystem)                        │
└──────────────────────────────────────────────────┘
```

Key design decisions:

1. **Range partitioning.** Data is divided into contiguous key ranges, each managed by a Raft group. Unlike hash partitioning, this preserves key ordering — essential for range scans and `ORDER BY` queries. CockroachDB, TiKV, and Spanner all use range partitioning.

2. **Multi-Raft.** Instead of one Raft group for all data, we run thousands of independent Raft groups (one per range). This parallelizes consensus — different ranges can accept writes concurrently on different leaders.

3. **PostgreSQL wire protocol.** Clients connect using the standard PostgreSQL protocol. This means existing tools (psql, ORMs, JDBC drivers) work without modification.

4. **Timestamp oracle.** A centralized (or distributed) service that issues monotonically increasing timestamps. Transactions use these timestamps for MVCC snapshot reads and commit ordering.

### Deep Dives

**1. Range Partitioning and Routing**

The entire key space is divided into ranges:

```
Range 1: [a, f)     → Raft Group 1 (Nodes 1, 4, 7)
Range 2: [f, m)     → Raft Group 2 (Nodes 2, 5, 8)
Range 3: [m, s)     → Raft Group 3 (Nodes 3, 6, 9)
Range 4: [s, z)     → Raft Group 4 (Nodes 1, 5, 9)
```

A **routing table** maps key ranges to Raft groups and their members. Every node caches this table. When a SQL query accesses key `"orders/12345"`, the gateway looks up which range contains this key and routes the request to the leader of that Raft group.

Range splits happen automatically when a range exceeds a size threshold (e.g., 512 MB). The leader splits the range into two, creates a new Raft group for the second half, and updates the routing table. This is similar to B-tree node splits — the same principle applied at the distributed level.

```
Before split:
  Range 1: [a, f)  → 1 GB  (too large)

After split:
  Range 1a: [a, c)  → 500 MB
  Range 1b: [c, f)  → 500 MB  (new Raft group)
```

**Connection to toydb:** In our database, we used a single Raft group for all data. A production system uses Multi-Raft — one Raft group per range. The Raft protocol you implemented in Chapters 14-16 is the same; it is just instantiated thousands of times.

**2. Distributed Transactions**

A transaction that touches multiple ranges cannot use a single Raft group. We need a distributed commit protocol. The standard approach is **two-phase commit (2PC)** with a **transaction coordinator**:

Phase 1 — Prepare:
```
Coordinator → Range 1 leader: "PREPARE txn-42, writes: {k1=v1}"
Coordinator → Range 3 leader: "PREPARE txn-42, writes: {k2=v2}"

Range 1 leader: Acquires locks, writes to Raft log, responds "PREPARED"
Range 3 leader: Acquires locks, writes to Raft log, responds "PREPARED"
```

Phase 2 — Commit:
```
Coordinator → Range 1 leader: "COMMIT txn-42"
Coordinator → Range 3 leader: "COMMIT txn-42"

Both leaders: Apply the writes, release locks, respond "COMMITTED"
```

If any participant responds "ABORT" during Phase 1, the coordinator sends "ABORT" to all participants. The coordinator's decision (commit or abort) is itself written to a Raft log — so the decision survives coordinator crashes.

**Connection to toydb:** Our MVCC implementation (Chapter 5) handles single-node transactions. Distributed transactions add a coordination layer on top — but the core MVCC logic (version chains, snapshot reads, conflict detection) is identical. The 2PC protocol ensures that all participants agree on the outcome, while Raft within each range ensures that the outcome is durable.

**3. Distributed Query Planning**

A query like `SELECT * FROM users JOIN orders ON users.id = orders.user_id WHERE users.country = 'US'` might span multiple ranges. The query planner must:

1. Determine which ranges contain `users` rows where `country = 'US'`
2. Determine which ranges contain matching `orders` rows
3. Decide where to execute the join — at the gateway, or push the join down to a co-located node

Strategies:

- **Co-located join.** If `users` and `orders` are partitioned by the same key (`user_id`), matching rows live on the same range. The join executes locally. This is the fastest option and the reason production systems carefully choose partition keys.

- **Broadcast join.** If one table is small, broadcast it to all nodes that hold the other table. Each node performs a local join. Good when one side fits in memory.

- **Shuffle join.** If neither table is co-located and both are large, repartition both tables by the join key and send matching partitions to the same node. This is the most expensive option but always correct.

```
Co-located:      ★★★★★  Best — no data movement
Broadcast:       ★★★☆☆  Good for small/large joins
Shuffle:         ★★☆☆☆  Expensive but universal
```

**Connection to toydb:** Our query optimizer (Chapter 9) chose between nested loop join and hash join. A distributed optimizer adds a third dimension: *where* to execute the join, not just *how*. The cost model expands from CPU + disk I/O to include network transfer costs.

### Scaling Discussion

| Scale | Architecture Change |
|-------|-------------------|
| **1 TB, 100 QPS** | 3 nodes, single Raft group. Essentially what we built in toydb. |
| **10 TB, 1K QPS** | 10 nodes, Multi-Raft with automatic range splitting. Dedicated timestamp oracle. |
| **100 TB, 10K QPS** | 30+ nodes. Learner replicas for read scaling. Range rebalancing for hotspot management. |
| **1 PB, 100K QPS** | 100+ nodes. Multi-region replication. Follower reads for geographic locality. Dedicated query coordinators. |

### Trade-offs

| Decision | Chosen | Alternative | Why |
|----------|--------|-------------|-----|
| Range partitioning | Ordered ranges | Hash partitioning | Preserves key order for range scans |
| Multi-Raft | One Raft group per range | Single Raft group | Parallelizes consensus across ranges |
| 2PC for distributed txns | Two-phase commit | Saga pattern | 2PC gives ACID guarantees; Sagas give eventual consistency |
| Timestamp oracle | Centralized service | Hybrid logical clocks | Simpler; HLC avoids single point of failure but adds complexity |
| PostgreSQL wire protocol | Compatible | Custom protocol | Ecosystem compatibility outweighs protocol overhead |

### What We Learned Building toydb That Applies Here

1. **The Storage trait pattern scales.** Our `Storage` trait with `set/get/delete/scan` maps directly to the per-range storage engine. The interface is the same; the implementation just runs on many machines.

2. **Raft works at range granularity.** The same election, replication, and durability protocol from Chapters 14-16 powers each range. The only addition is a management layer that creates, splits, and merges Raft groups.

3. **MVCC is the transaction foundation.** Whether single-node or distributed, snapshot isolation uses version chains and transaction timestamps. The distributed layer adds coordination (2PC) but not a fundamentally different concurrency model.

4. **The layered architecture generalizes.** Our SQL → Plan → Execute → MVCC → Storage stack applies at scale. Each layer gains a distribution dimension but retains its core responsibility.

---

## 20.2 Design a Key-Value Store

### The Interview Question

> "Design a distributed key-value store like DynamoDB or Redis Cluster. It should support GET, PUT, and DELETE operations with high availability and low latency."

### Requirements Gathering

- **Operations.** GET(key) -> value, PUT(key, value), DELETE(key). Any secondary indexes? *No — primary key access only.*
- **Consistency.** Strong or eventual? *Configurable — some use cases need strong, others accept eventual.*
- **Availability target.** *99.99% uptime (four nines). Less than 52 minutes of downtime per year.*
- **Latency target.** *p99 < 10ms for single-key operations.*
- **Data model.** Value size limits? TTL support? *Values up to 400 KB. TTL support required.*
- **Throughput.** *1 million requests per second at peak.*
- **Data durability.** *No data loss for committed writes, even during node failures.*

### Capacity Estimation

| Metric | Value |
|--------|-------|
| Total items | 10 billion |
| Average key size | 64 bytes |
| Average value size | 1 KB |
| Total data size | ~10 TB |
| Peak requests/sec | 1,000,000 (70% reads, 30% writes) |
| Replication factor | 3 |
| Raw storage needed | 30 TB (3x replication) |
| Number of nodes | 100 (each with 300 GB SSD, 32 GB RAM) |
| Partitions | ~1,000 (each ~10 GB) |

### High-Level Design

```
┌──────────────────────────────────────────────┐
│              Client SDK                       │
│    (Connection pooling, retry logic,          │
│     consistent hashing ring cache)            │
└──────────────────┬───────────────────────────┘
                   │
┌──────────────────▼───────────────────────────┐
│            Request Router                     │
│    (Partition lookup, leader/replica routing, │
│     quorum coordination)                      │
├──────────────────────────────────────────────┤
│          Partition Layer                      │
│    (Consistent hashing ring,                 │
│     virtual nodes, rebalancing)              │
├──────────────────────────────────────────────┤
│          Replication Layer                    │
│    (Leader-follower or leaderless,           │
│     quorum reads/writes, anti-entropy)       │
├──────────────────────────────────────────────┤
│          Storage Engine                      │
│    (LSM tree per partition,                  │
│     compaction, bloom filters)               │
└──────────────────────────────────────────────┘
```

### Deep Dives

**1. Consistent Hashing**

Hash partitioning distributes keys uniformly across nodes. But naive hash partitioning (`hash(key) % N`) breaks catastrophically when N changes — adding a node remaps almost every key.

Consistent hashing fixes this. Imagine a ring of positions from 0 to 2^32. Each node claims multiple positions on the ring (virtual nodes). A key maps to the ring position `hash(key)` and is assigned to the next node clockwise:

```
Ring positions (simplified):

     Node A (pos 0)
    /               \
   /                 \
  Node D (pos 270)    Node B (pos 90)
   \                 /
    \               /
     Node C (pos 180)

Key "user:42" → hash = 110 → assigned to Node C (next clockwise after 110)
Key "user:99" → hash = 50  → assigned to Node B (next clockwise after 50)
```

When Node B fails, only keys between Node A's position and Node B's position are remapped — to Node C. The other 75% of keys are unaffected. This is the key insight: adding or removing a node affects only 1/N of the keys.

Virtual nodes (each physical node claims 100-200 positions on the ring) ensure even distribution. Without virtual nodes, a three-node ring would have uneven segments. With virtual nodes, each physical node handles approximately 1/N of the key space regardless of how positions fall on the ring.

**Connection to toydb:** Our database used range partitioning (ordered key ranges) because SQL queries need range scans. A key-value store uses hash partitioning because it does not need ordered access — every operation is a single-key lookup. The trade-off: hash partitioning gives better load distribution, range partitioning gives ordered iteration.

**2. Replication and Quorum**

Each partition is replicated to R nodes (typically R=3). For availability, we want reads and writes to succeed even when some replicas are down. Quorum-based replication provides tunable consistency:

- **Write quorum (W):** The number of replicas that must acknowledge a write before it is considered successful. With R=3, W=2 means two out of three replicas must confirm.
- **Read quorum (R_q):** The number of replicas that must respond to a read. With R=3, R_q=2 means two out of three replicas must respond.
- **Consistency guarantee:** If W + R_q > R, every read sees the latest write. With W=2 and R_q=2 on a 3-replica system: 2+2=4 > 3, so strong consistency is guaranteed.

```
Strong consistency:    W=2, R_q=2  (W + R_q = 4 > 3)
Eventual consistency:  W=1, R_q=1  (W + R_q = 2 ≤ 3)
Write-optimized:       W=1, R_q=3  (strong reads, fast writes)
Read-optimized:        W=3, R_q=1  (fast reads, slow writes)
```

**Connection to toydb:** Our Raft implementation (Chapters 14-16) uses a quorum of (N/2 + 1) for both reads and writes — the strongest consistency guarantee. A key-value store often offers weaker quorum options for higher availability and lower latency. Understanding both approaches — Raft's single-leader quorum and DynamoDB's leaderless quorum — gives you the vocabulary to discuss consistency trade-offs.

**3. Anti-Entropy and Conflict Resolution**

With eventual consistency (W=1, R_q=1), replicas can diverge. Two mechanisms bring them back into agreement:

**Read repair:** When a read quorum returns conflicting values from different replicas, the coordinator sends the latest value to the stale replicas. This piggybacks repair on normal read traffic.

**Merkle tree anti-entropy:** Each replica maintains a Merkle tree (hash tree) over its key space. Periodically, replicas exchange Merkle tree roots. If roots differ, they drill down to find the divergent ranges and synchronize only those keys. This is O(log N) communication for N keys.

```
Replica A Merkle tree:          Replica B Merkle tree:
       [H_root_A]                     [H_root_B]
       /        \                     /        \
   [H_left]  [H_right_A]        [H_left]  [H_right_B]
                  ↑                              ↑
              Different!                     Different!

Only the right subtree needs synchronization.
```

**Conflict resolution** when two replicas have different values for the same key:

- **Last-writer-wins (LWW):** Each write carries a timestamp. The write with the highest timestamp wins. Simple but can lose data.
- **Vector clocks:** Track causality across replicas. Concurrent writes are detected and returned to the client for application-level resolution. More complex but never silently loses data.
- **CRDTs:** Use conflict-free data types (like the G-Counter from Chapter 19, Problem 8) that merge automatically. Limited to specific data models but always converge.

**Connection to toydb:** Our database avoids conflicts entirely through Raft — there is one leader, and all writes go through it. A leaderless key-value store must handle conflicts explicitly. The CRDT approach from Problem 8 in Chapter 19 is one solution. Understanding why Raft avoids the conflict resolution problem (at the cost of availability during leader elections) is a valuable interview insight.

### Scaling Discussion

| Scale | Architecture Change |
|-------|-------------------|
| **100 GB, 10K QPS** | 3 nodes, simple hash partitioning, Raft-based replication. |
| **1 TB, 100K QPS** | 10 nodes, consistent hashing with virtual nodes, tunable consistency. |
| **10 TB, 1M QPS** | 100 nodes, automatic rebalancing, cross-datacenter replication. DAX-style caching layer. |
| **100 TB, 10M QPS** | 1,000+ nodes, multi-region with local reads. Hot partition detection and splitting. Request hedging for tail latency. |

### Trade-offs

| Decision | Chosen | Alternative | Why |
|----------|--------|-------------|-----|
| Consistent hashing | Ring-based | Range partitioning | Uniform distribution without ordering requirement |
| Leaderless replication | Quorum-based | Leader-follower (Raft) | Higher availability during partitions |
| LSM tree storage | Write-optimized | B-tree | Batches writes for SSD-friendly sequential I/O |
| LWW conflict resolution | Timestamp-based | Vector clocks | Simpler; acceptable for most use cases |
| Virtual nodes | 200 per physical node | Static assignment | Even distribution, easy rebalancing |

### What We Learned Building toydb That Applies Here

1. **The Storage trait abstracts the engine.** Whether LSM tree, B-tree, or in-memory hash map, the storage engine interface is the same: `get(key)`, `put(key, value)`, `delete(key)`. Our `Storage` trait designed in Chapter 2 is this exact interface.

2. **Quorums generalize Raft majorities.** Raft requires a majority (N/2 + 1) for both reads and writes. Leaderless quorum systems separate read and write quorums, offering more flexibility. The underlying math — overlapping quorums guarantee consistency — is the same.

3. **Serialization matters at scale.** Our binary serialization (Chapter 4) directly applies. At 1M QPS, every byte of overhead in the serialization format costs megabytes per second of bandwidth. Efficient encoding (Protocol Buffers, FlatBuffers) is not premature optimization — it is table stakes.

---

## 20.3 Design a Distributed Transaction System

### The Interview Question

> "Design a distributed transaction system like Google Spanner or Calvin. It should support serializable transactions spanning multiple data centers with minimal coordination overhead."

### Requirements Gathering

- **Isolation level.** Serializable or snapshot isolation? *Serializable — the strongest guarantee.*
- **Geographic scope.** How far apart are data centers? *Continental distances — 50-200ms round-trip latency.*
- **Transaction profile.** Read-heavy or write-heavy? Short or long transactions? *Mostly short transactions (< 100ms). 80% read-only, 20% read-write.*
- **Conflict rate.** Do transactions frequently touch the same data? *Low conflict rate — most transactions touch disjoint key sets.*
- **External consistency.** If transaction T1 commits before T2 starts (in real time), must T2 see T1's writes? *Yes — external consistency (linearizability).*

### Capacity Estimation

| Metric | Value |
|--------|-------|
| Data centers | 5 (US-East, US-West, EU, Asia, Australia) |
| Cross-DC latency | 50-200ms round-trip |
| Total transactions/sec | 100,000 |
| Read-only transactions | 80,000/sec |
| Read-write transactions | 20,000/sec |
| Average keys per transaction | 5 reads, 2 writes |
| Conflict rate | < 1% of write transactions conflict |

### High-Level Design

```
┌─────────────────────────────────────────────┐
│              Client Application              │
│                                              │
│  BEGIN TRANSACTION                            │
│  SELECT balance FROM accounts WHERE id = 1   │
│  UPDATE accounts SET balance = balance - 100 │
│    WHERE id = 1                              │
│  UPDATE accounts SET balance = balance + 100 │
│    WHERE id = 2                              │
│  COMMIT                                      │
└──────────────────┬──────────────────────────┘
                   │
┌──────────────────▼──────────────────────────┐
│         Transaction Coordinator              │
│    (Begins txn, assigns timestamp,          │
│     coordinates 2PC across participants)     │
├─────────────────────────────────────────────┤
│         Concurrency Control                  │
│    (Lock manager or MVCC + SSI,             │
│     deadlock detection)                      │
├─────────────────────────────────────────────┤
│         Timestamp Authority                  │
│    (TrueTime / Hybrid Logical Clocks /      │
│     centralized oracle)                      │
├─────────────────────────────────────────────┤
│         Data Partitions                      │
│    (Raft groups per partition,              │
│     co-located with storage)                 │
└─────────────────────────────────────────────┘
```

### Deep Dives

**1. The Timestamp Problem**

For external consistency across data centers, we need a global ordering of transactions. If T1 commits in US-East at time 100 and T2 starts in EU at time 101, T2 must see T1's writes. But clocks across data centers are not perfectly synchronized — US-East's clock might be 5ms ahead of EU's clock.

Three approaches to timestamp ordering:

**TrueTime (Google Spanner's approach):**

Google uses atomic clocks and GPS receivers in every data center to keep clocks synchronized within a known uncertainty bound (typically < 7ms). TrueTime returns not a single timestamp but an interval: `[earliest, latest]`. When a transaction commits at `latest`, it waits until `earliest > latest` to ensure no other transaction can claim an earlier timestamp. This "commit wait" adds latency equal to the clock uncertainty (< 7ms).

```
TrueTime API:
  TT.now() → Interval { earliest: 100, latest: 107 }

Commit protocol:
  1. Assign commit timestamp = latest (107)
  2. Wait until TT.now().earliest > 107
  3. Commit is now externally consistent
```

**Hybrid Logical Clocks (CockroachDB's approach):**

Combine physical clocks with logical counters. The physical component provides rough ordering; the logical component breaks ties. When a node receives a message with a higher timestamp, it advances its clock. This provides causal consistency without specialized hardware — but not external consistency. CockroachDB handles the gap with a "clockless read" protocol that detects and retries transactions affected by clock skew.

```rust,ignore
struct HybridClock {
    physical: u64,    // wall clock in milliseconds
    logical: u32,     // logical counter for ties
}

impl HybridClock {
    fn now(&mut self) -> HybridTimestamp {
        let wall = system_time_ms();
        if wall > self.physical {
            self.physical = wall;
            self.logical = 0;
        } else {
            self.logical += 1;
        }
        HybridTimestamp {
            physical: self.physical,
            logical: self.logical,
        }
    }

    fn update(&mut self, received: HybridTimestamp) {
        if received.physical > self.physical {
            self.physical = received.physical;
            self.logical = received.logical + 1;
        } else if received.physical == self.physical {
            self.logical = self.logical.max(received.logical) + 1;
        }
        // If received.physical < self.physical, no update needed
    }
}
```

**Centralized Timestamp Oracle (TiDB's approach):**

A single service issues monotonically increasing timestamps. Simple and correct, but the oracle is a throughput bottleneck and a single point of failure. TiDB mitigates this by batching timestamp requests — the oracle issues ranges of timestamps (e.g., "here are timestamps 1000 through 2000") in a single RPC.

**Connection to toydb:** Our MVCC implementation (Chapter 5) used a simple local counter for transaction versions. In a distributed system, this counter must be globally ordered. Each of the three approaches above is a way to make our `version` counter work across multiple machines. The concept is identical — the distribution is the hard part.

**2. Two-Phase Commit (2PC) with Raft**

Standard 2PC is vulnerable to coordinator failure — if the coordinator crashes between Phase 1 and Phase 2, participants are stuck holding locks indefinitely. Spanner and CockroachDB solve this by making the coordinator's state durable via Raft:

```
Phase 1 (Prepare):
  Coordinator:
    1. Write PREPARE record to coordinator's Raft log
    2. Send PREPARE to all participants
  Each participant:
    1. Acquire locks, validate constraints
    2. Write PREPARED record to participant's Raft log
    3. Respond PREPARED to coordinator

Phase 2 (Commit):
  Coordinator:
    1. Receive PREPARED from all participants (or timeout → ABORT)
    2. Write COMMIT record to coordinator's Raft log
    3. Send COMMIT to all participants
  Each participant:
    1. Apply writes
    2. Release locks
    3. Write COMMITTED to participant's Raft log
```

If the coordinator crashes after writing COMMIT to its Raft log but before sending COMMIT to participants, the new coordinator (elected via Raft) reads the log, finds the COMMIT decision, and resends it. The COMMIT decision is never lost.

**Connection to toydb:** Our Raft implementation (Chapters 14-16) ensures that committed log entries survive crashes. Wrapping 2PC decisions in Raft log entries applies this same durability guarantee to the transaction coordination layer. The Raft protocol is unchanged; it just logs different types of entries (PREPARE, COMMIT, ABORT) in addition to data mutations.

**3. Read-Only Transaction Optimization**

80% of our transactions are read-only. For these, we can avoid the full 2PC protocol:

1. Assign a read timestamp from the timestamp authority
2. Read from any replica that has applied all entries up to that timestamp
3. No locks needed — MVCC snapshot isolation ensures consistent reads
4. No 2PC needed — no writes to coordinate

This is why the 80/20 read/write split matters for design. Read-only transactions are cheap — they use the same MVCC snapshot reads we built in Chapter 5, just distributed across nodes. Write transactions are expensive — they require 2PC coordination plus Raft consensus.

Spanner goes further with **stale reads**: a client can request a read at a timestamp in the past (e.g., "read as of 10 seconds ago"). Any replica that has applied entries up to that timestamp can serve the read — no leader communication needed. This reduces latency for read-heavy workloads that can tolerate slightly stale data.

### Scaling Discussion

| Scale | Architecture Change |
|-------|-------------------|
| **Single DC** | Single timestamp oracle, Raft-based 2PC, read replicas. Essentially toydb with Multi-Raft. |
| **2 DCs** | Raft groups span both DCs. Cross-DC latency dominates commit time (~50ms). |
| **5 DCs** | TrueTime or HLC for timestamp ordering. Leader placement optimization (place leaders near the workload). Follower reads for geographic locality. |
| **Global (10+ DCs)** | Partitioned timestamp oracles. Read-only transactions use stale reads. Write transactions route to the closest leader. |

### Trade-offs

| Decision | Chosen | Alternative | Why |
|----------|--------|-------------|-----|
| TrueTime | Specialized hardware | HLC (software-only) | External consistency without retries |
| 2PC over Raft | Durable coordinator | Saga pattern | ACID guarantees for financial workloads |
| Serializable isolation | SSI (Serializable Snapshot Isolation) | S2PL (Strict Two-Phase Locking) | Better read performance, fewer lock contentions |
| Centralized oracle | Single service | Distributed oracles | Simpler correctness argument; batch to reduce bottleneck |

### What We Learned Building toydb That Applies Here

1. **MVCC is the concurrency foundation.** Our snapshot isolation from Chapter 5 provides the read-side concurrency. Distributed transactions add write-side coordination but do not replace MVCC — they build on top of it.

2. **Raft makes 2PC durable.** The classic criticism of 2PC — "what if the coordinator crashes?" — is solved by making the coordinator a Raft group. Every Raft group we built in Chapters 14-16 can serve as a durable 2PC coordinator.

3. **The commit path determines latency.** In our single-node database, commit latency was one disk flush. In a distributed system, commit latency is one Raft round-trip (for the prepare) plus one Raft round-trip (for the commit), multiplied by the cross-DC latency. Understanding the commit path — the critical path from client request to durable commit — is the key to designing low-latency distributed transactions.

---

## 20.4 Design a Query Engine

### The Interview Question

> "Design a distributed query engine like Presto, Trino, or Apache Spark SQL. It should execute SQL queries over data stored in multiple backend systems (data lakes, databases, APIs) without moving the data."

### Requirements Gathering

- **Data sources.** What backend systems? *S3 (Parquet/ORC files), PostgreSQL, MySQL, Elasticsearch, REST APIs.*
- **Query types.** Ad-hoc analytics or pre-defined queries? *Ad-hoc SQL queries — users write arbitrary SQL.*
- **Data volume per query.** *Scans of 1 GB to 1 TB per query.*
- **Concurrency.** *Up to 100 concurrent queries.*
- **Latency.** *Seconds to minutes for analytical queries, not sub-millisecond OLTP.*
- **Join support.** *Cross-source joins — join a PostgreSQL table with data in S3.*
- **Data movement.** *The engine does NOT store data persistently. It reads from sources, processes, and returns results.*

### Capacity Estimation

| Metric | Value |
|--------|-------|
| Worker nodes | 50 (each with 128 GB RAM, 32 cores) |
| Total cluster memory | 6.4 TB |
| Total cluster cores | 1,600 |
| Peak concurrent queries | 100 |
| Data scanned per query | 1 GB - 1 TB |
| Total data scanned/day | 50 TB |
| Network bandwidth per node | 10 Gbps |

### High-Level Design

```
┌──────────────────────────────────────────────┐
│              Client (SQL)                     │
│    "SELECT u.name, SUM(o.amount)              │
│     FROM s3.orders o                          │
│     JOIN postgres.users u ON o.user_id = u.id │
│     GROUP BY u.name"                          │
└──────────────────┬───────────────────────────┘
                   │
┌──────────────────▼───────────────────────────┐
│             Coordinator Node                  │
│    ┌────────────────────────────┐             │
│    │  Parser → Planner →        │             │
│    │  Optimizer → Scheduler     │             │
│    └────────────┬───────────────┘             │
│                 │  distributes work            │
├─────────────────┼────────────────────────────┤
│    ┌────────────▼───────────────────┐         │
│    │       Execution Engine          │         │
│    │  ┌───────┐ ┌───────┐ ┌───────┐ │         │
│    │  │Worker │ │Worker │ │Worker │ │         │
│    │  │  1    │ │  2    │ │  ...  │ │         │
│    │  └───┬───┘ └───┬───┘ └───┬───┘ │         │
│    └──────┼─────────┼─────────┼─────┘         │
│           │         │         │                │
├───────────┼─────────┼─────────┼───────────────┤
│    ┌──────▼─────────▼─────────▼──────┐        │
│    │         Connector Layer          │        │
│    │  ┌─────┐ ┌──────┐ ┌───────────┐ │        │
│    │  │ S3  │ │Postgr│ │ Elastic   │ │        │
│    │  │     │ │ eSQL  │ │ search    │ │        │
│    │  └─────┘ └──────┘ └───────────┘ │        │
│    └─────────────────────────────────┘        │
└──────────────────────────────────────────────┘
```

### Deep Dives

**1. Push-Down vs Pull-Up Execution**

The most impactful optimization in a federated query engine is **predicate push-down**: pushing filters, projections, and aggregations into the data source, so less data is transferred over the network.

```
Original plan:
  Filter(country = 'US') → Scan(s3.users)
  Reads ALL users from S3, then filters locally.
  Data transferred: 10 GB (all users)

Optimized plan (push-down):
  Scan(s3.users, filter: country = 'US')
  S3 reads only matching Parquet row groups.
  Data transferred: 500 MB (US users only)
```

Each connector defines what it can push down:

| Connector | Pushable Operations |
|-----------|-------------------|
| S3/Parquet | Column selection, row group pruning (min/max statistics), partition pruning |
| PostgreSQL | Full SQL — any filter, join, aggregation can run on PostgreSQL |
| Elasticsearch | Filters, aggregations, but not arbitrary SQL |
| REST API | Query parameters only |

The optimizer's job: push as much as possible to the source, pull up only what it cannot push down.

**Connection to toydb:** Our query optimizer (Chapter 9) pushed filters below joins to reduce row counts early. The federated optimizer does the same thing across system boundaries — pushing a filter from the query engine into PostgreSQL or S3. The principle is identical: apply selective operations as early as possible.

**2. Execution Models: Pull vs Push**

Our database (Chapter 10) uses the **Volcano (pull) model**: the root operator calls `next()` on its child, which calls `next()` on its child, recursively. This is simple but has overhead: one virtual function call per row per operator.

Distributed query engines use two alternative models:

**Push model:** Data flows bottom-up. Each operator pushes rows to its parent. This eliminates the per-row virtual call overhead and enables pipelining:

```
Scan operator:
  for each batch of rows:
    filter.push(batch)

Filter operator:
  fn push(&self, batch: RowBatch):
    let matching = batch.filter(predicate)
    aggregate.push(matching)

Aggregate operator:
  fn push(&self, batch: RowBatch):
    self.accumulator.update(batch)
```

**Vectorized model:** Process batches of rows (e.g., 1024 rows at a time) through each operator instead of single rows. This amortizes function call overhead and enables SIMD (Single Instruction, Multiple Data) CPU optimizations:

```rust,ignore
struct RowBatch {
    columns: Vec<ColumnVector>,  // columnar layout within the batch
    num_rows: usize,
}

struct ColumnVector {
    data: Vec<i64>,        // all values for one column
    nulls: BitVec,         // null bitmap
}

// Vectorized filter: process 1024 values at once
fn filter_batch(batch: &RowBatch, predicate: &Expression) -> RowBatch {
    let mask = evaluate_predicate_batch(predicate, batch); // returns BitVec
    batch.select(&mask) // keep rows where mask is true
}
```

**Connection to toydb:** Our Volcano model from Chapter 10 works well for OLTP (one row at a time, low latency). Analytical query engines use vectorized or push-based execution for throughput. Understanding both models — and when each is appropriate — demonstrates depth in query engine design.

**3. Distributed Shuffle and Exchange**

When a query requires data from multiple nodes to be combined (e.g., a hash join or GROUP BY), the engine must **shuffle** data across the network. The exchange operator handles this:

```
Query: SELECT country, COUNT(*) FROM users GROUP BY country

Step 1: Each worker scans its partition and pre-aggregates locally.
  Worker 1: {US: 500, UK: 200, CA: 100}
  Worker 2: {US: 300, UK: 150, JP: 50}
  Worker 3: {US: 200, CA: 80, JP: 30}

Step 2: Hash-partition by country and send to designated workers.
  Worker 1 receives all "US" partial aggregates: [500, 300, 200]
  Worker 2 receives all "UK" partial aggregates: [200, 150]
  Worker 3 receives all "CA" and "JP" partial aggregates: [100, 80], [50, 30]

Step 3: Each worker performs final aggregation.
  Worker 1: US → 1000
  Worker 2: UK → 350
  Worker 3: CA → 180, JP → 80
```

This is a **two-phase aggregation**: local pre-aggregation reduces data volume before the shuffle, and final aggregation produces the result. Without pre-aggregation, every raw row would be shuffled across the network.

**Connection to toydb:** Our executor (Chapter 10) performed aggregation in a single pass on a single node. Distributed aggregation adds the shuffle step — but the aggregation logic (accumulators for SUM, COUNT, AVG) is identical. The two-phase optimization is the distributed equivalent of our optimizer pushing filters below joins: reduce data volume before expensive operations.

**4. Fault Tolerance and Intermediate Results**

Long-running analytical queries (minutes to hours) must handle node failures gracefully. Two approaches:

**Checkpoint-based (Spark):** Materialize intermediate results to disk (HDFS/S3) at stage boundaries. If a worker fails, restart the stage from the checkpoint. This adds I/O overhead but provides reliable fault tolerance.

**Re-execution (Presto/Trino):** Do not checkpoint. If a worker fails, re-execute the entire query from scratch (or from the last exchange boundary). This is faster for short queries but wasteful for long ones.

The trade-off maps to the strategic vs tactical split from the design reflection chapter: checkpointing is a strategic investment (more upfront work for reliability), while re-execution is tactical (simpler implementation, faster for the common case).

### Scaling Discussion

| Scale | Architecture Change |
|-------|-------------------|
| **10 workers, 10 GB/query** | Single coordinator, all-in-memory processing. Hash exchanges. |
| **50 workers, 100 GB/query** | Disk-based shuffle for large joins. Resource management (fair scheduling across queries). |
| **200 workers, 1 TB/query** | Hierarchical execution (sub-coordinators). Approximate query processing for interactive speed. |
| **1000+ workers** | Disaggregated compute and storage. Elastic scaling (spin up workers per query). Cost-based scheduling. |

### Trade-offs

| Decision | Chosen | Alternative | Why |
|----------|--------|-------------|-----|
| Vectorized execution | Batch of 1024 rows | Volcano (row-at-a-time) | 10-100x throughput for analytical queries |
| Hash exchange | Hash-partition shuffle | Broadcast | Scales to large datasets; broadcast only for small tables |
| Pre-aggregation | Two-phase aggregation | Full shuffle then aggregate | Reduces network traffic by orders of magnitude |
| No checkpointing (Presto model) | Re-execute on failure | Spark-style checkpoints | Lower latency for the common case (no failure) |
| Connector-based | Plugin architecture | Monolithic data access | Extensible to new data sources without engine changes |

### What We Learned Building toydb That Applies Here

1. **The Volcano model is the foundation.** Every distributed query engine started with the Volcano model and optimized from there. Understanding pull-based execution (Chapter 10) is prerequisite to understanding push-based and vectorized alternatives.

2. **Predicate push-down is the biggest optimization.** In our optimizer (Chapter 9), pushing filters below joins was the single most impactful rewrite. In a distributed engine, pushing predicates into the data source is even more impactful — it avoids network transfer entirely.

3. **Expression evaluation is the inner loop.** The expression evaluator from Chapter 10 (and Problem 2 in Chapter 19) runs for every row. At analytical scale (billions of rows), this is the hottest code path. Vectorized evaluation — processing columns of values instead of row-by-row — is the key to making it fast.

4. **The parser and planner are reusable.** Our SQL parser (Chapters 6-7) and planner (Chapter 8) work unchanged in a distributed engine. The distribution logic lives in the optimizer (choosing exchange operators) and the executor (parallel execution), not in parsing or planning.

---

## Summary

| Design | Key Concept | toydb Connection |
|--------|------------|-----------------|
| Distributed SQL Database | Multi-Raft, range partitioning, 2PC | Chapters 14-17: Raft + MVCC + Integration |
| Key-Value Store | Consistent hashing, quorums, CRDTs | Chapters 2-3: Storage + Chapter 19 Problem 8 |
| Distributed Transactions | TrueTime/HLC, 2PC over Raft, SSI | Chapter 5: MVCC + Chapters 14-16: Raft |
| Query Engine | Push/vectorized execution, push-down, exchange | Chapters 8-11: Optimizer + Executor |

Each design extends the single-node database you built into a distributed system. The core algorithms — Raft consensus, MVCC concurrency, tree-based query execution, cost-based optimization — are the same. Distribution adds partitioning, coordination, and fault tolerance on top of these foundations.

The most important takeaway: **distributed systems are not magic.** They are single-node systems (like the one you built) replicated, partitioned, and coordinated. Every concept in this chapter — range partitioning, quorums, 2PC, vectorized execution, consistent hashing — builds directly on what you already know.
