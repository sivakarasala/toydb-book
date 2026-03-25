## What You Built

In this chapter, you:

1. **Created a key-value store** — a `Database` struct wrapping `HashMap<String, Value>` with `set`, `get`, `delete`, and `list` operations
2. **Built an interactive REPL** — reading from stdin, parsing commands, dispatching with `match`
3. **Implemented typed values** — using Rust's `enum` with `String`, `Integer`, `Float`, and `Boolean` variants
4. **Added operation tracking** — counting gets, sets, and deletes with a `STATS` command
5. **Learned Rust fundamentals** — `let` vs `let mut`, `String` vs `&str`, `HashMap`, `Option`, `match`, `enum`, `Display` trait, and the borrow checker

Your toydb is ephemeral — close the program and the data is gone. In Chapter 2, we will build a proper in-memory storage engine with better abstractions. In Chapter 3, we will add persistence so data survives restarts. But the shape of the API — `set`, `get`, `delete` — will remain recognizable throughout the entire book. That is the power of getting the interface right from the start.

---

## DS Deep Dive

Want to go deeper on hash tables? Read the narrative deep dive:

[Hash Table — "The key-value locker room"](../../ds-narratives/ch01-hash-table-storage.md)

It covers open addressing vs chaining, load factors, resize strategies, and why Rust chose Robin Hood hashing — all through the lens of building a storage engine.

---

## Reference

The code you built in this chapter corresponds to these concepts in the [toydb reference implementation](https://github.com/erikgrinaker/toydb):

| Your code | toydb reference | Concept |
|-----------|----------------|---------|
| `Database` struct | [`src/storage/kv.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/storage/kv.rs) | The `KV` trait defines the key-value interface |
| `HashMap<String, Value>` | `Memory` storage engine | In-memory storage using `BTreeMap` (sorted keys) |
| `Value` enum | [`src/sql/types.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/sql/types.rs) | The `Value` enum with `Null`, `Boolean`, `Integer`, `Float`, `String` |
| `match` command dispatch | [`src/client.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/client.rs) | Client REPL command parsing |
| `OperationStats` | Not in toydb | Production databases expose stats via `SHOW STATUS` or metrics endpoints |
