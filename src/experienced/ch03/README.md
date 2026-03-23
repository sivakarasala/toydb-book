# Chapter 3: Persistent Storage — BitCask

Every database you have used promises the same thing: your data will survive a power outage, a crash, a restart. The in-memory storage engine from Chapter 2 breaks that promise — kill the process and everything evaporates. This chapter fixes that. You will build a BitCask-style log-structured storage engine that appends every write to a file on disk and maintains an in-memory index for fast lookups.

By the end of this chapter, you will have:

- A `LogStorage` struct that persists data to an append-only log file
- An in-memory `HashMap` index that maps keys to file offsets for O(1) reads
- Tombstone-based deletes and a full startup recovery flow
- CRC32 checksums for data integrity and truncated-record handling for crash safety
- A clear understanding of Rust's file I/O, the `?` operator, and custom error types

---
