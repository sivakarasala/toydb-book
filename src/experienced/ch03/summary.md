## What You Built

In this chapter, you:

1. **Built an append-only log** — a binary file format with CRC32 checksums, key/value encoding, and sequential writes
2. **Added an in-memory index** — a `HashMap<String, u64>` mapping keys to file offsets for O(1) reads
3. **Implemented the Storage trait** — making `LogStorage` interchangeable with any other storage backend
4. **Handled crash recovery** — truncated records, CRC mismatches, and fsync for durability
5. **Practiced file I/O and error handling** — `File`, `OpenOptions`, `BufWriter`, `BufReader`, `Result<T, E>`, the `?` operator, and custom error types

Your data now survives restarts. The `LogStorage` engine is a simplified version of BitCask — the same architecture that powers production databases handling millions of writes per second. In Chapter 4, we will tackle serialization — converting complex Rust types into bytes and back, so our database can store more than raw byte arrays.

---

### DS Deep Dive

Ready to go deeper? This chapter's data structure deep dive explores log-structured storage from first principles — why appending is the most natural way to write to magnetic platters and flash cells, and how LSM trees evolved from the same idea.

**-> [Log-Structured Storage — "Append only, ask questions later"](../../ds-narratives/ch03-log-structured-storage.md)**

---

### Reference

The files you built in this chapter:

| Your file | Purpose |
|-----------|---------|
| `src/storage.rs` | `LogStorage` struct — append-only log, in-memory index, crash recovery |
| `src/traits.rs` | `Storage` trait — common interface for all storage backends |
| `src/main.rs` | Main entry point with demo usage |

Key Rust standard library types used:

| Type | Module | Purpose |
|------|--------|---------|
| `File` | `std::fs` | File handle for reading and writing |
| `OpenOptions` | `std::fs` | Builder for opening files with specific permissions |
| `BufWriter` | `std::io` | Buffered writes — batches small writes into large ones |
| `BufReader` | `std::io` | Buffered reads — reads large chunks, serves small ones |
| `SeekFrom` | `std::io` | Enum for file seek positions (Start, End, Current) |
| `HashMap` | `std::collections` | Hash map for the in-memory key-to-offset index |
| `Result<T, E>` | `std::result` | Return type for operations that can fail |
