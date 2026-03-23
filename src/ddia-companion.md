# DDIA Companion — Reading Guide

*How to read [Designing Data-Intensive Applications](https://dataintensive.net/) by Martin Kleppmann alongside this book.*

---

You're building a database from scratch. Kleppmann wrote the book that explains **why** every design decision you're making matters at scale. This guide tells you exactly what to read and when — so that DDIA theory lands right after you've wrestled with the implementation.

**How to use this guide:** After completing each toydb chapter and its exercise, read the corresponding DDIA sections listed below. You'll find that concepts that felt abstract in DDIA suddenly click because you just built them.

---

## Chapter 1-2: What Is a Database? + In-Memory Storage

### You just built
A key-value store using `HashMap` and `BTreeMap`, a `Storage` trait, and a generic `Database<S: Storage>`.

### Read in DDIA

**Chapter 1: Reliable, Scalable, and Maintainable Applications** (pp. 3-22)
- *Reliability* (pp. 6-10): You built a single-node database. What happens when your process crashes? When the disk fills up? Kleppmann catalogs every way systems fail — hardware faults, software errors, human mistakes. Keep these in mind as you build each layer.
- *Scalability* (pp. 10-17): Your `HashMap` lookup is O(1). But what happens with 100 million keys? 1 billion? Kleppmann introduces "load parameters" and "performance numbers" — the vocabulary you'll need when your toy database meets real workloads.
- *Maintainability* (pp. 18-22): You split storage into a trait and implementations. That's the "evolvability" Kleppmann describes. Notice how this decision already pays off — you can swap `MemoryStorage` for `BitCask` without changing the SQL layer.

**Chapter 3: Storage and Retrieval — "Data Structures That Power Your Database"** (pp. 69-78)
- *Hash Indexes* (pp. 72-75): You just built one! Kleppmann uses Bitcask (which you'll build in Ch3) as his primary example. Notice how he identifies the exact same limitation you'll discover — hash indexes can't do range queries.
- *SSTables and LSM-Trees* (pp. 76-79): An alternative to your `BTreeMap`. Read this to understand the design space — after you've seen how B-Trees work in our DS Deep Dive, you'll appreciate why some databases (Cassandra, RocksDB, LevelDB) chose a completely different approach.
- *B-Trees* (pp. 79-83): You used Rust's `BTreeMap`. Now read how B-Trees actually work on disk — 4KB pages, write-ahead logs for crash safety, and why "the basic underlying write operation is to overwrite a page on disk." Our DS Deep Dive builds one from scratch; DDIA explains why it has dominated databases since 1970.

> **The aha moment:** You chose `BTreeMap` over `HashMap` for `MemoryStorage`. Kleppmann spends 15 pages explaining why that's the right default — range queries, sorted iteration, prefix scans. You already know this because you coded it.

---

## Chapter 3: Persistent Storage — BitCask

### You just built
A log-structured storage engine with an in-memory hash index, CRC checksums, and append-only writes.

### Read in DDIA

**Chapter 3: Storage and Retrieval** (pp. 72-76)
- *Hash Indexes* (pp. 72-75): Kleppmann literally describes Bitcask by name. Your implementation follows the same design — append-only log file, in-memory hash map pointing to byte offsets. Now you understand every word on this page because you wrote the code.
- *Compaction* (pp. 73-74): You may not have implemented compaction yet. Read how "segments" work — when the log gets too big, you merge old segments, discarding overwritten keys. This is the maintenance task that keeps Bitcask from eating your disk.

**Chapter 3: Comparing B-Trees and LSM-Trees** (pp. 83-85)
- Read this section to understand where your BitCask engine fits in the landscape. BitCask is a simplified LSM — append-only writes (fast), but requires the entire key set to fit in memory. Kleppmann explains the trade-offs: write amplification, read amplification, and space amplification.

> **Discussion question:** Your BitCask engine rebuilds the entire index by scanning the log file on startup. Kleppmann mentions "hint files" as an optimization. How would you implement a hint file in your engine?

---

## Chapter 4: Serialization

### You just built
Binary serialization using `serde` and `bincode` to convert Rust structs to bytes and back.

### Read in DDIA

**Chapter 4: Encoding and Evolution** (pp. 111-148)
- *Formats for Encoding Data* (pp. 112-119): You used bincode (a binary format). Kleppmann surveys the entire landscape — JSON, XML, Protocol Buffers, Thrift, Avro. Each has different trade-offs around schema evolution, human readability, and compactness.
- *Language-Specific Formats* (pp. 113-114): Kleppmann warns against Java's `Serializable`, Python's `pickle`, etc. Notice that `bincode` avoids these pitfalls — it's language-agnostic in format (any language could decode the bytes) even though you're using Rust-specific derive macros.
- *Schema Evolution and Compatibility* (pp. 120-132): This is the killer section. What happens when you add a column to your database schema? Old data was serialized without it. Kleppmann explains forward/backward compatibility — concepts that don't matter for a toy database but are critical in production.

> **The bridge:** In your toydb, serialization is an internal detail — you control both the writer and reader. But Kleppmann shows that in real systems (microservices, message queues, API evolution), serialization format choice is a 10-year architectural decision.

---

## Chapter 5: MVCC — Multi-Version Concurrency

### You just built
Snapshot isolation using version chains — each write creates a new version, readers see a consistent snapshot based on their start timestamp.

### Read in DDIA

**Chapter 7: Transactions** (pp. 221-266)
- *The Meaning of ACID* (pp. 223-227): You implemented the "I" (Isolation). Kleppmann argues that ACID is a marketing term — each database defines these guarantees differently. After building MVCC, you can critically evaluate what "isolation" actually means.
- *Weak Isolation Levels* (pp. 233-242): You built Snapshot Isolation. Read why it's called "weak" — it prevents dirty reads and non-repeatable reads, but allows write skew. Kleppmann's examples (doctor on-call scheduling, meeting room booking) show exactly how snapshot isolation can produce anomalies your implementation allows.
- *Snapshot Isolation and Repeatable Read* (pp. 237-242): This section describes exactly what you built. Multi-version concurrency control, each transaction sees a consistent snapshot, readers never block writers. Kleppmann even draws the version chain diagrams that match your data structure.
- *Implementing Snapshot Isolation* (pp. 239-241): "The database must potentially keep several different committed versions of an object." That's your `Vec<VersionedEntry>`. Kleppmann explains visibility rules (created before, not deleted before the transaction's start) — the same logic in your `is_visible()` function.

> **The aha moment:** Kleppmann says "snapshot isolation is a useful isolation level, especially for read-only transactions." You proved this — your implementation allows concurrent readers without any locking, which is exactly why PostgreSQL (which calls it "Repeatable Read") uses MVCC.

---

## Chapter 6-7: SQL Lexer + Parser

### You just built
A tokenizer that converts SQL strings into token streams, and a recursive-descent parser that produces an AST.

### Read in DDIA

**Chapter 2: Data Models and Query Languages** (pp. 27-68)
- *Relational Model Versus Document Model* (pp. 28-42): You're building a relational database with SQL. Kleppmann explains why the relational model won (and where it didn't) — document databases (MongoDB), graph databases (Neo4j), and the ongoing debate. Your parser handles `SELECT * FROM users WHERE age > 21` — Kleppmann explains why this declarative approach beat navigational/hierarchical models in the 1970s.
- *Query Languages for Data* (pp. 42-48): You built a parser for SQL. Kleppmann contrasts SQL (declarative) with imperative approaches. The key insight: "In a declarative query language, you just specify the pattern of the data you want — what conditions the results must meet... but not how to achieve that goal." That separation between parsing (your Ch7) and execution (your Ch10) is exactly this principle.
- *MapReduce Querying* (pp. 46-48): An alternative to SQL for distributed data. Read this after building your parser to appreciate that SQL is one of many query languages, and each database chooses its model based on the use case.

> **Connection to your code:** Your parser produces an AST — `Statement::Select { columns, from, where_clause }`. That AST is the "declarative specification" Kleppmann describes. The planner and executor (Ch8-11) are the "how" — the database's job, not the user's.

---

## Chapter 8-9: Query Planner + Optimizer

### You just built
An AST-to-plan converter and a plan optimizer with constant folding and filter pushdown.

### Read in DDIA

**Chapter 3: Storage and Retrieval** (pp. 85-91)
- *Multi-column indexes* (pp. 87-88): Your planner currently does full table scans. Kleppmann explains how indexes (B-Tree, hash, concatenated) change the planner's decisions. "The most common type of multi-column index is called a concatenated index" — this is why query planning is hard: you have to choose which index to use.
- *Keeping everything in memory* (pp. 88-89): Your entire database is in memory. Kleppmann discusses in-memory databases (VoltDB, MemSQL, Redis) and argues "the performance advantage of in-memory databases is not due to the fact that they don't need to read from disk" — it's because they avoid the overhead of encoding data for disk layout. Your `MemoryStorage` proves this.

**Chapter 2: Query Languages** (pp. 42-46)
- Re-read the declarative vs imperative section. Your optimizer rewrites `WHERE 2 + 3 > x` to `WHERE 5 > x` (constant folding) — this is the optimizer taking advantage of the declarative specification. The user said *what* they want; the optimizer figures out the fastest *how*.

> **Why this matters:** Kleppmann notes that "the fact that SQL is more limited in functionality gives the database much more room for automatic optimizations." Every optimization pass you add (filter pushdown, constant folding) is the database exploiting that declarative gap.

---

## Chapter 10-11: Query Executor + SQL Features

### You just built
A Volcano-model iterator executor, hash joins, GROUP BY aggregation, and ORDER BY sorting.

### Read in DDIA

**Chapter 3: Storage and Retrieval — OLTP vs OLAP** (pp. 90-95)
- *Transaction Processing or Analytics?* (pp. 90-92): Your executor handles both — point lookups (`WHERE id = 42`) and analytical queries (`SELECT COUNT(*) ... GROUP BY`). Kleppmann distinguishes OLTP (many small reads/writes by users) from OLAP (few large reads by analysts). Real databases split into separate engines for each. Your toydb does both in one — read why that's a deliberate simplification.
- *Column-Oriented Storage* (pp. 95-101): Your executor reads entire rows. Column stores read only the columns needed. Kleppmann explains why analytics queries (scanning millions of rows for a few columns) are dramatically faster with column-oriented layout. This is an extension you could add to toydb.

**Chapter 6: Partitioning — Partitioning and Secondary Indexes** (pp. 206-212)
- Your hash join builds a hash table from one side. Kleppmann describes how distributed databases partition data across nodes and how joins work when data is spread across machines — a natural extension of your single-node hash join.

> **Exercise:** After reading about column stores, think about how you'd modify `serialize_row()` and `deserialize_row()` to store data column-by-column instead of row-by-row. What queries would get faster? What would get slower?

---

## Chapter 12-13: Client-Server Protocol + Async Networking

### You just built
A TCP server with length-prefixed framing and an async Tokio server with shared state.

### Read in DDIA

**Chapter 8: The Trouble with Distributed Systems** (pp. 275-318)
- *Unreliable Networks* (pp. 277-284): You built a TCP server that assumes the network works. Kleppmann catalogs everything that can go wrong: packets lost, delayed, reordered, duplicated; connections timeout; the remote node might be dead or just slow. Read this right after your "it works on localhost" success — it's a cold shower.
- *Unreliable Clocks* (pp. 287-299): Your MVCC uses timestamps. What if two nodes have different clocks? Kleppmann explains why "last write wins" based on timestamps is fundamentally broken, and why logical clocks (Lamport timestamps, vector clocks) exist.
- *Knowledge, Truth, and Lies* (pp. 300-318): The most philosophical section of the book. "A node in the network cannot know anything for sure — it can only make guesses based on the messages it receives." After building a single-node server, this section prepares you for the distributed nightmare of Ch14-16.

> **The gap:** Your server works perfectly on one machine. DDIA Chapter 8 explains why adding a second machine turns every simple operation into an unsolved computer science problem. This is the motivation for Raft.

---

## Chapter 14-15: Raft — Leader Election + Log Replication

### You just built
A Raft state machine with Follower/Candidate/Leader states, term-based voting, and AppendEntries log replication.

### Read in DDIA

**Chapter 5: Replication** (pp. 151-198)
- *Leaders and Followers* (pp. 152-159): You built exactly this — a leader that accepts writes, followers that replicate. Kleppmann describes the same architecture but at production scale: replication lag, failover, read-your-writes consistency.
- *Problems with Replication Lag* (pp. 161-167): Your single-node Raft auto-commits. But with real followers, there's lag. Kleppmann describes three guarantees clients expect: read-after-write consistency, monotonic reads, and consistent prefix reads. Each is a different bug waiting to happen.
- *Multi-Leader Replication* (pp. 168-175): An alternative to your single-leader Raft. Multiple nodes accept writes, and conflicts are resolved later. Read this to understand why Raft's "one leader" rule is a simplification, not a limitation.

**Chapter 9: Consistency and Consensus** (pp. 321-375)
- *Atomic Commit and Two-Phase Commit* (pp. 354-360): The predecessor to Raft/Paxos. 2PC is simpler but has a fatal flaw — a crashed coordinator blocks everyone. Read this to appreciate why Raft's leader election is necessary.
- *Fault-Tolerant Consensus* (pp. 364-370): Kleppmann describes Raft alongside Paxos, Zab, and Viewstamped Replication. He identifies the four properties every consensus algorithm must provide: uniform agreement, integrity, validity, and termination. Map these back to your implementation — which properties does your code enforce?
- *Membership and Coordination Services* (pp. 370-375): ZooKeeper and etcd use consensus internally but expose a key-value interface — exactly what your toydb does. "They are designed to hold small amounts of data that can fit entirely in memory... and is replicated across all the nodes using a fault-tolerant total order broadcast algorithm." That's your architecture.

> **The deep connection:** Your `RaftNode::propose()` appends a command to the log and waits for a majority. Kleppmann calls this "total order broadcast" and proves it's mathematically equivalent to consensus. You implemented a fundamental theorem of distributed systems.

---

## Chapter 16: Raft — Durability & Recovery

### You just built
A write-ahead log (WAL) with CRC32 checksums, fsync for durability, and crash recovery by replaying the log.

### Read in DDIA

**Chapter 3: Storage and Retrieval** (pp. 79-83)
- *Making B-Trees reliable* (pp. 82-83): "An additional complication of updating pages in place is that careful handling is needed if the database crashes in the middle of a write. To make the database resilient to crashes, it is common for B-tree implementations to include an additional data structure on disk: a write-ahead log (WAL, also known as a redo log)." You just built exactly this. Kleppmann explains why the WAL is essential — without it, a half-written page corrupts the B-tree.

**Chapter 7: Transactions — Serializability** (pp. 251-266)
- *Actual Serial Execution* (pp. 252-256): An alternative to MVCC — run transactions one at a time. VoltDB does this. With your WAL, you have a total ordering of all writes — serial execution is a natural extension.

**Chapter 9: Consistency and Consensus — Total Order Broadcast** (pp. 348-354)
- Your WAL is a total order broadcast log for a single node. Kleppmann generalizes this to distributed systems: "total order broadcast requires messages to be delivered to all nodes in the same order." Raft's log replication achieves this across multiple nodes. Your WAL achieves it for crash recovery on a single node.

> **Reflection:** You wrote 28 bytes per WAL entry (8 term + 8 index + 4 len + N data + 4 CRC). PostgreSQL's WAL entry format is more complex but follows the same principle — enough information to reconstruct the state after a crash, with checksums to detect corruption.

---

## Chapter 17-18: Integration + Testing

### You just built
A complete database that wires Storage → SQL → Raft → REPL, plus integration tests and property-based testing patterns.

### Read in DDIA

**Chapter 1: Reliability, Scalability, and Maintainability** (pp. 18-22) — Re-read
- Now re-read the maintainability section. You've built a system with clear module boundaries (storage trait, SQL pipeline, Raft consensus). Kleppmann's three design principles — operability, simplicity, and evolvability — are exactly what your architecture achieves. The `Storage` trait is evolvability. The SQL pipeline's layered design is simplicity. The REPL is operability.

**Chapter 12: The Future of Data Systems** (pp. 489-526)
- *Data Integration* (pp. 490-500): Your toydb is a single unified system. But in practice, organizations use dozens of data systems — and the hard problem is making them work together. Read this after finishing your database to understand where toydb fits in the larger picture.
- *Doing the Right Thing* (pp. 515-526): Kleppmann's closing meditation on ethics in data engineering. A fitting end to your journey — now that you understand how databases work, think about how they should work.

---

## The Complete Reading Schedule

For quick reference — read these DDIA sections after completing each toydb chapter:

| toydb Chapter | DDIA Reading | Pages | Key Concept |
|:---:|:---|:---:|:---|
| 1-2 | Ch 1 (all) + Ch 3 (pp. 69-83) | ~35 | Reliability + B-Trees vs Hash Indexes |
| 3 | Ch 3 (pp. 72-76, 83-85) | ~10 | Bitcask, LSM vs B-Tree trade-offs |
| 4 | Ch 4 (pp. 111-132) | ~20 | Encoding formats + schema evolution |
| 5 | Ch 7 (pp. 221-242) | ~20 | ACID, isolation levels, MVCC |
| 6-7 | Ch 2 (pp. 27-48) | ~20 | Data models + declarative queries |
| 8-9 | Ch 3 (pp. 85-91) | ~6 | Indexes + in-memory databases |
| 10-11 | Ch 3 (pp. 90-101) | ~10 | OLTP vs OLAP + column stores |
| 12-13 | Ch 8 (all, pp. 275-318) | ~43 | Unreliable networks, clocks, truth |
| 14-15 | Ch 5 (pp. 151-175) + Ch 9 (pp. 354-375) | ~45 | Replication + consensus |
| 16 | Ch 3 (pp. 82-83) + Ch 7 (pp. 252-256) | ~5 | WAL + serial execution |
| 17-18 | Ch 1 (pp. 18-22) + Ch 12 (pp. 489-526) | ~40 | Maintainability + the bigger picture |
| **Total** | | **~254** | **~50% of DDIA's 500+ pages** |

You'll cover roughly half of DDIA this way — the half that directly relates to what you're building. The remaining chapters (Partitioning, Batch Processing, Stream Processing) are worth reading on their own but don't map directly to toydb's single-node architecture.

---

## Books That Pair Well After DDIA + toydb

Once you've finished both books, you'll have strong theoretical + practical foundations. To go deeper:

- **Database Internals** by Alex Petrov — deeper on B-Trees, LSM-Trees, and distributed consensus internals
- **The Raft Paper** (Diego Ongaro, 2014) — the original paper, now readable because you built it
- **Architecture of a Database System** by Hellerstein, Stonebraker, Hamilton — the classic survey paper, free online
- **PostgreSQL Internals** (interdb.jp) — see how a production database implements everything you built
