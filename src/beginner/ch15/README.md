# Chapter 15: Raft -- Log Replication

Your cluster can elect a leader. Congratulations. But a leader that does not do anything useful is just a figurehead. The entire point of Raft is to keep data safe by storing copies on multiple servers. When a client sends a write to the leader, the leader needs to:

1. Store the write in its own log
2. Send the write to all followers
3. Wait until a majority of followers confirm they have it
4. Only then tell the client "your write is saved"

This is **log replication** -- the process of keeping every server's log in sync. If the leader crashes after a majority has the data, the next leader is guaranteed to have it too. No data is lost.

This chapter builds the replication machinery. You will learn about `Arc` and `Mutex` (Rust's tools for safely sharing data between tasks), the `AppendEntries` RPC, and how the leader tracks what each follower knows.

By the end of this chapter, you will have:

- A `RaftLog` data structure for storing ordered log entries
- The `AppendEntries` RPC for replicating entries and sending heartbeats
- Leader bookkeeping with `next_index` and `match_index` per follower
- Commitment rules: an entry is committed when a majority has it
- A working replication simulation
- A solid understanding of `Arc` and `Mutex`

---
