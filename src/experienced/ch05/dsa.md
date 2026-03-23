## DSA in Context: Version Chains

MVCC is a data structures problem. Each key has a chain of versions — a linked list of `(version, value)` pairs. The fundamental operations are:

### Version chain as a sorted list

```
Key "alice":
  v1 -> 1000
  v3 -> 800     (after transfer)
  v5 -> 900     (after deposit)

Key "bob":
  v1 -> 500
  v3 -> 700     (after transfer)
```

To read "alice" at version 4, we find the latest version <= 4, which is v3 (value 800). This is a binary search on the version chain — O(log V) where V is the number of versions for that key.

### Visibility rules

A version `(key, v)` is visible to transaction T if:

1. `v <= T.snapshot_version` — the version was created before T's snapshot
2. The transaction that created version `v` has committed (not still active, not aborted)
3. There is no newer version `v'` where `v < v' <= T.snapshot_version` (we want the latest visible version)

Our simplified implementation only checks rule 1 and 3 (we do not track transaction commit status). The real toydb maintains an "active transactions" set to implement rule 2.

### Garbage collection

Without cleanup, version chains grow forever. A version is safe to garbage-collect when no active transaction can ever read it — specifically, when its version is older than the oldest active transaction's snapshot.

```
Active transactions: T5 (snapshot=4), T7 (snapshot=6)
Oldest snapshot: 4

Key "alice" versions: v1, v3, v5, v8
  - v1: can be removed (v3 supersedes it, and oldest snapshot is 4, so no one reads v1)
  - v3: KEEP (T5 reads this — it is the latest version <= 4)
  - v5: KEEP (T7 reads this)
  - v8: KEEP (future transactions read this)
```

This is the "MVCC vacuum" — PostgreSQL's `VACUUM` command does exactly this. It scans for dead versions and reclaims their space.

### Time complexity summary

| Operation | Naive (our impl) | Optimized (real DB) |
|-----------|-----------------|---------------------|
| Read | O(N) scan all versions | O(log V) binary search on version chain |
| Write | O(1) insert | O(1) insert + O(1) conflict check |
| Commit | O(W) apply W writes | O(W) apply + O(W) conflict detection |
| Garbage collection | O(N) scan all entries | Background incremental |

Our `BTreeMap` implementation is O(N) for reads because we scan the entire map. A production MVCC engine would use ordered iteration starting from `(key, snapshot_version)` and scanning backward to find the first match — O(log N + V) where N is total entries and V is versions of that key.

---
