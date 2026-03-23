## What You Built

In this chapter, you:

1. **Understood the problem** -- demonstrated that a naive store shows inconsistent intermediate states during multi-step operations
2. **Built versioned keys** -- `(key, version)` entries in a `BTreeMap` that preserve the full history of every key
3. **Implemented transactions** -- `begin()`, `get()`, `set()`, `delete()`, `commit()`, `abort()` with buffered writes and snapshot reads
4. **Proved snapshot isolation** -- tests showing that a reader's view is frozen at begin time, unaffected by concurrent commits
5. **Learned references and lifetimes** -- Rust's system for safe borrowing that prevents dangling pointers, data races, and use-after-free bugs at compile time

Your database now supports concurrent readers with consistent snapshots. Writers buffer their changes and apply them atomically on commit. This is the same mechanism that PostgreSQL, MySQL, and CockroachDB use to serve thousands of concurrent connections.

But users do not want to call `txn.set("name", Value::String("Alice"))`. They want to write `INSERT INTO users (name) VALUES ('Alice')`. Chapter 6 begins the SQL journey with a lexer that breaks SQL strings into tokens.

---

## Exercises

**Exercise 5.1: Add a `scan` method to Transaction**

Add a method that returns all key-value pairs visible to the transaction, sorted by key:

```rust,ignore
pub fn scan(&self, store: &MvccStore) -> Result<Vec<(String, Value)>, String>
```

<details>
<summary>Hint</summary>

You need to merge two sources: the store's versioned data (at the snapshot version) and the transaction's local writes. Local writes take priority over store values. Keys with `None` values (tombstones) should be excluded from the result.

</details>

**Exercise 5.2: Version history**

Add a method to `MvccStore` that returns the complete version history of a key:

```rust,ignore
pub fn history(&self, key: &str) -> Vec<(u64, Option<&Value>)>
```

This should return a list of `(version, value)` pairs for the key, sorted by version.

<details>
<summary>Hint</summary>

Filter `self.data.iter()` to entries where `vk.key == key`, then collect into a vector. Each entry is `(vk.version, val.as_ref())`.

</details>

**Exercise 5.3: Transaction read-your-writes for deletes**

Verify that if a transaction deletes a key and then reads it, the read returns `None`. Write a test for this:

```rust,ignore
let mut txn = store.begin();
txn.set("key", Value::Integer(42)).unwrap();
assert!(txn.get(&store, "key").unwrap().is_some()); // sees its own write

txn.delete("key").unwrap();
assert!(txn.get(&store, "key").unwrap().is_none()); // sees its own delete
```

<details>
<summary>Hint</summary>

This should already work with the current implementation. The `get` method checks local writes first, and a delete stores `None` in the write buffer. When `get` finds `None` in the local writes, it returns `Ok(None)`.

</details>

---

## Key Takeaways

- **MVCC** keeps multiple versions of data so readers and writers do not block each other.
- **Snapshot isolation** freezes each transaction's view at begin time. Changes by other transactions are invisible.
- **Versioned keys** `(key, version)` in a sorted map naturally group versions together.
- **Buffered writes** give atomicity -- either all writes succeed (commit) or none do (abort).
- **References** (`&T`, `&mut T`) let you borrow data without owning it. The borrow checker prevents dangling references and data races.
- **Lifetimes** (`'a`) tell the compiler how long a reference is valid. They are a description, not a command.
- **Ownership by value** in method signatures (`fn commit(self, ...)`) prevents use-after-commit bugs at compile time.
- **`Option`** is Rust's way of saying "this might not have a value" -- safer than null pointers.

---

### Reference implementation

The files you built in this chapter correspond to these files in the reference codebase:

| Your file | Reference |
|-----------|-----------|
| `src/mvcc.rs` -- `MvccStore` | [`src/storage/mvcc.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/storage/mvcc.rs) -- `MVCC` struct |
| `src/mvcc.rs` -- `Transaction` | [`src/storage/mvcc.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/storage/mvcc.rs) -- `Transaction` struct |
| `VersionedKey` | [`src/storage/mvcc.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/storage/mvcc.rs) -- `Key` enum with version encoding |
| Snapshot isolation tests | [`src/storage/mvcc.rs` tests](https://github.com/erikgrinaker/toydb/blob/master/src/storage/mvcc.rs) |
