## Design Insight: Design It Twice

> *"Designing software is hard, so it's unlikely that your first idea will be the best one. You'll end up with a much better result if you consider multiple options."*
> — John Ousterhout, *A Philosophy of Software Design*

We could have designed the WAL differently. Here are two alternatives:

### Design A: Single-file WAL (our approach)

```
One file: raft.wal
Entries appended sequentially.
Compaction: rewrite the entire file.

Pros: Simple. One file to manage. Easy to reason about.
Cons: Compaction requires rewriting everything. Large WAL means
      slow compaction.
```

### Design B: Segmented WAL

```
Multiple files: wal-0001.log, wal-0002.log, wal-0003.log
Each segment holds up to N entries.
New segment created when current one is full.
Compaction: delete old segment files.

Pros: Compaction is O(1) — just delete files. No rewriting.
Cons: More files to manage. Reads span multiple files.
      Need to track which segment contains which indices.
```

### Design C: Embedded key-value store for WAL

```
Use sled or RocksDB as the WAL backend.
Each entry stored with its index as the key.
Compaction: delete keys by range.

Pros: Proven, crash-safe, handles all the file management.
Cons: Heavy dependency. Hides the learning. Hard to debug.
```

We chose Design A because it is the simplest and most instructive. A production system would likely use Design B (segmented WAL) — it is what etcd, CockroachDB, and TiKV use. Design C trades understanding for convenience, which is the wrong tradeoff for a learning project.

The point of "design it twice" is not to implement all three — it is to *think through* all three before committing to one. The act of comparing designs reveals tradeoffs that you would not see by diving straight into implementation.

---
