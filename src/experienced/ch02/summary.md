## What You Built

In this chapter, you:

1. **Defined a `Storage` trait** — four methods that every storage engine must implement, with a custom `Error` enum for structured error handling
2. **Implemented `MemoryStorage`** — a B-tree-backed in-memory engine with ordered keys and efficient range scans
3. **Built a generic `Database<S: Storage>`** — a database layer that works with any storage engine through trait bounds and static dispatch
4. **Wrote unit tests** — eight tests covering set, get, delete, overwrite, missing keys, range scans, full scans, and empty ranges

Your database can store and retrieve key-value pairs, scan ranges in sorted order, and swap storage engines at compile time. But when the process exits, everything is gone. In Chapter 3, we will add persistent storage — writing to disk so data survives restarts.

---

### DS Deep Dive

Ready to go deeper? This chapter's data structure deep dive explores the B-tree — the data structure that powers your `BTreeMap` and nearly every database index in production.

**-> [B-Tree -- "The filing cabinet that sorts itself"](../../ds-narratives/ch02-b-tree.md)**

You used `BTreeMap` as a black box. This deep dive opens the box: node structure, key splitting, tree rebalancing, and why B-trees are cache-friendly. You will understand why databases chose this structure fifty years ago and why they still choose it today.

---

### Reference

The files you built in this chapter:

| File | Purpose |
|------|---------|
| `src/error.rs` | Custom `Error` enum with `NotFound` and `Internal` variants |
| `src/storage.rs` | `Storage` trait — the contract for all storage engines |
| `src/memory.rs` | `MemoryStorage` — B-tree-backed in-memory implementation with tests |
| `src/database.rs` | `Database<S: Storage>` — generic database layer |
| `src/main.rs` | Module registration and demo usage |
