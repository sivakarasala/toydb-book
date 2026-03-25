## What You Built

In this chapter, you:

1. **Added serde and bincode** — the foundation of Rust's serialization ecosystem, with derive macros that generate encoding/decoding code at compile time
2. **Built round-trip tests** — proving that every `Value` variant survives the encode-decode cycle, including edge cases like empty strings, extreme numbers, and corrupted input
3. **Created a Row and Table abstraction** — moving from raw key-value pairs to structured data with named columns and typed values
4. **Implemented a custom binary format** — understanding what serde does under the hood, and when manual encoding is the right choice

Your database now understands its data. It knows the difference between the integer `42` and the string `"42"`. It can store structured rows with multiple columns. And it can serialize everything to compact binary for storage or transmission.

But the database still has a critical limitation: if two users read and write at the same time, they will see inconsistent data. One user's write might appear halfway through another user's read. Chapter 5 introduces MVCC — Multi-Version Concurrency Control — the mechanism that gives every reader a consistent snapshot, even while writers are modifying data.

---

### DS Deep Dive

Serde's derive macros look like magic, but they are built on Rust's procedural macro system — code that writes code. This deep dive explores how proc macros work, the Visitor pattern that serde uses internally, and how to build your own derive macro from scratch.

**-> [Serde Internals & Procedural Macros -- "The Code That Writes Code"](../../ds-narratives/ch04-binary-serialization.md)**

---

### Reference implementation

The files you built in this chapter correspond to these files in the reference codebase:

| Your file | Reference |
|-----------|-----------|
| `src/value.rs` | [`src/sql/types.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/sql/types.rs) — `Value` enum with serialization |
| `src/table.rs` | [`src/sql/engine/local.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/sql/engine/local.rs) — table operations |
| Manual encoding | [`src/encoding.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/encoding.rs) — custom key encoding for ordered storage |
| Round-trip tests | Tests within each module |
