# Chapter 16: Raft — Durability & Recovery

Your Raft cluster elects leaders and replicates log entries. Then the power goes out. Every node restarts with a blank slate — no idea who the leader was, no idea what entries were committed, no idea what term it was in. All of your carefully replicated state is gone. This is not a theoretical concern. Servers crash. Disks fail. Kernels panic. Data centers lose power. A consensus protocol that cannot survive a restart is a toy. This chapter makes it real.

You will build a durable storage layer that persists Raft state to disk using a write-ahead log, implement snapshot creation and transfer so slow followers can catch up without replaying the entire history, and design recovery logic that reconstructs the node's in-memory state from what it wrote to disk before it crashed. The spotlight concept is **ownership and persistence** — how Rust's ownership model naturally maps to the question of "who is responsible for this file handle, and when does it get closed?"

By the end of this chapter, you will have:

- A write-ahead log (WAL) that persists Raft log entries with append-only writes
- Persistent storage for `current_term`, `voted_for`, and `commit_index`
- `fsync`-based durability guarantees that survive process crashes and power loss
- A recovery procedure that reads persisted state on startup and reconstructs the node
- A snapshot mechanism that compacts old log entries into a point-in-time image
- An `InstallSnapshot` RPC for transferring snapshots to lagging followers
- Crash recovery tests that verify correctness after simulated failures

---
