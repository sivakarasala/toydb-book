# Chapter 5: MVCC — Multi-Version Concurrency Control

Your database can store typed values, serialize them to bytes, and persist them to disk. But it has a dirty secret: it assumes one user at a time. If two transactions run concurrently — one reading a bank balance while another transfers money — the reader might see a half-finished transfer. Account A debited, account B not yet credited. The money vanished into thin air. This is the consistency problem, and every real database must solve it.

This chapter builds a Multi-Version Concurrency Control (MVCC) layer. Instead of locking data so only one transaction can access it at a time, MVCC keeps multiple versions of each value. Readers see a consistent snapshot frozen at the moment their transaction began. Writers create new versions without disturbing readers. No one waits. No one blocks. Everyone sees a consistent world.

By the end of this chapter, you will have:

- A versioned key-value store where each write creates a new `(key, version)` entry
- A `Transaction` struct with `begin()`, `get()`, `set()`, and `commit()`
- Snapshot isolation: each transaction reads only versions that were committed before it started
- Tests proving that concurrent readers see consistent snapshots even during writes
- A clear understanding of Rust lifetimes, references, and borrowing rules

---
