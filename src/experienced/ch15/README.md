# Chapter 15: Raft -- Log Replication

Your cluster can elect a leader. But a leader that does not replicate data is just a single point of failure with extra steps. The entire purpose of consensus is to keep multiple copies of the data in sync. When a client sends a write to the leader, the leader must store it locally and replicate it to a majority of followers before confirming the write. If the leader crashes after a majority has the data, the next elected leader is guaranteed to have it too. No data is lost.

This chapter implements Raft's log replication protocol. The leader maintains a replicated log — an ordered sequence of commands that every node applies to its state machine in the same order. You will build the `AppendEntries` RPC, implement the consistency check that detects and repairs divergent logs, manage commit indices, and apply committed entries to the database. The spotlight concept is **concurrency with Arc and Mutex** — safely sharing mutable state across async tasks in a distributed system.

By the end of this chapter, you will have:

- A `RaftLog` with term-indexed entries, append, truncation, and consistency checking
- The `AppendEntries` RPC for both heartbeats and log replication
- Leader bookkeeping with `next_index` and `match_index` per follower
- Commitment rules: an entry is committed when replicated to a majority
- State machine application: applying committed entries to the database
- `Arc<Mutex<RaftState>>` for sharing Raft state across async network tasks
- A clear understanding of how Raft guarantees no committed entry is ever lost

---
