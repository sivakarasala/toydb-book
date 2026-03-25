## What You Built

In this chapter, you:

1. **Built the Raft log** — `RaftLog` with append, truncation, consistency checking, and the Log Matching Property invariant
2. **Implemented AppendEntries** — full RPC handling on both leader and follower sides, with the consistency check that detects and repairs divergent logs
3. **Added commitment rules** — majority-based commitment, the current-term restriction, and the `advance_commit_index` algorithm
4. **Applied committed entries** — `apply_committed()` feeds committed log entries to the database state machine in order
5. **Shared state with Arc<Mutex<>>** — wrapped `RaftState` for concurrent access from async tasks, with short critical sections and proper lock scoping
6. **Tested replication** — leader election, log replication, follower catch-up, and leader change scenarios with the deterministic test harness

Your database is now replicated. Writes go to the leader, are replicated to followers, committed when a majority acknowledges them, and applied to the state machine. If the leader crashes, a new leader is elected with all committed data intact. This is the core guarantee of Raft: **no committed entry is ever lost**.

Chapter 16 adds durability — persisting the log and Raft state to disk so that nodes can recover after crashes without losing their state.

---

### DS Deep Dive

The Raft log is a specific instance of a broader concept: replicated state machines. This deep dive explores the theory of state machine replication (Schneider 1990), compares it with operation-based and state-based replication, and traces how the same idea appears in database WALs, event sourcing systems, and blockchain. We examine the CAP theorem through the lens of Raft's design choices and discuss why linearizability matters for database correctness.

**-> [Replicated State Machines — "The Copy Room"](../../ds-narratives/ch14-raft-replicated-log.md)**

---

### Reference implementation

The files you built in this chapter correspond to these files in the reference codebase:

| Your file | Reference |
|-----------|-----------|
| `RaftLog` — replicated log | [`src/raft/log.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/raft/log.rs) — `Log` struct |
| `AppendEntries` handling | [`src/raft/node.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/raft/node.rs) — `append`, `heartbeat` |
| `commit_index`, `advance_commit_index` | [`src/raft/node.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/raft/node.rs) — `commit` logic |
| `Arc<Mutex<RaftState>>` | [`src/raft/node.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/raft/node.rs) — state sharing |
| State machine application | [`src/raft/state.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/raft/state.rs) — `apply` |
| Replication tests | [`tests/`](https://github.com/erikgrinaker/toydb/tree/master/tests) — cluster tests |
