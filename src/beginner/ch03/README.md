# Chapter 3: Persistent Storage — BitCask

Your database has a problem. Close the program and all your data vanishes. Every key, every value — gone. This is because `MemoryStorage` keeps everything in RAM (your computer's temporary memory), and RAM is wiped when the program stops.

Real databases promise something stronger: your data survives restarts, crashes, and power outages. This chapter builds that promise.

You will create a **BitCask-style storage engine** — a design used by real databases (Riak, Bitcask) that writes every operation to a file on disk. It is elegant in its simplicity: append every write to the end of a file, keep an in-memory index for fast lookups, and replay the file on startup to rebuild the index.

By the end of this chapter, you will have:

- A `LogStorage` struct that persists data to an append-only log file
- An in-memory `HashMap` index that maps keys to file offsets for O(1) reads
- CRC32 checksums to detect data corruption
- Startup recovery that rebuilds the index by scanning the log file
- A deep understanding of Rust's file I/O, the `?` operator, and custom error types

---
