## What You Built

In this chapter, you:

1. **Built a replicated log** — `RaftLog` stores an ordered list of entries that every node agrees on
2. **Implemented AppendEntries** — the leader sends new entries to followers, and followers check that their log matches before appending
3. **Added commitment rules** — an entry is "committed" when a majority of nodes have it. Only then is it safe to apply
4. **Applied entries to the state machine** — committed entries are fed to your database engine in order, so every node ends up with the same data
5. **Used Arc and Mutex** — Rust's tools for safely sharing data between async tasks. `Arc` shares ownership, `Mutex` ensures only one task accesses the data at a time

Your database now replicates writes across multiple nodes. If the leader crashes, a new leader is elected and all committed data is still there. This is the core promise of Raft: **no committed entry is ever lost**.

---

### Key Rust concepts practiced

- **`Arc<Mutex<T>>`** — the standard pattern for shared mutable state across async tasks. `Arc` = shared ownership, `Mutex` = exclusive access
- **Lock scoping** — keeping the lock held for as short as possible to avoid blocking other tasks
- **Vec operations** — `push`, `truncate`, `split_off` for managing the log as a growable array
