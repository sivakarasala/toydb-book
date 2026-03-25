## What You Built

In this chapter, you:

1. **Understood the problem** — demonstrated that a naive store shows inconsistent intermediate states during multi-step operations
2. **Built versioned keys** — `(key, version)` entries in a `BTreeMap` that preserve the full history of every key
3. **Implemented transactions** — `begin()`, `get()`, `set()`, `delete()`, `commit()`, `abort()` with buffered writes and snapshot reads
4. **Proved snapshot isolation** — tests showing that a reader's view is frozen at begin time, unaffected by concurrent commits

Your database now supports concurrent readers with consistent snapshots. Writers buffer their changes and apply them atomically on commit. This is the same mechanism that PostgreSQL, MySQL, and CockroachDB use to serve thousands of concurrent connections.

But users do not want to call `set("name", Value::String("Alice"))`. They want to write `INSERT INTO users (name) VALUES ('Alice')`. Chapter 6 begins the SQL journey with a lexer that breaks SQL strings into tokens.

---

### DS Deep Dive

MVCC snapshot isolation prevents most anomalies but not all. This deep dive explores write skew (the anomaly that snapshot isolation misses), serializable snapshot isolation (SSI), and how PostgreSQL and CockroachDB achieve full serializability without locks.

**-> [MVCC Anomalies & Serializable Snapshot Isolation -- "The Time Travel Paradox"](../../ds-narratives/ch05-mvcc-version-chains.md)**

---

### Reference implementation

The files you built in this chapter correspond to these files in the reference codebase:

| Your file | Reference |
|-----------|-----------|
| `src/mvcc.rs` — `MvccStore` | [`src/storage/mvcc.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/storage/mvcc.rs) — `MVCC` struct |
| `src/mvcc.rs` — `Transaction` | [`src/storage/mvcc.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/storage/mvcc.rs) — `Transaction` struct |
| `VersionedKey` | [`src/storage/mvcc.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/storage/mvcc.rs) — `Key` enum with version encoding |
| Snapshot isolation tests | [`src/storage/mvcc.rs` tests](https://github.com/erikgrinaker/toydb/blob/master/src/storage/mvcc.rs) |
