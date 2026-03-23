## DSA in Context: Log-Structured Storage

You just built a log-structured storage engine. Let us analyze its complexity:

| Operation | Time | Why |
|-----------|------|-----|
| `set(key, value)` | O(1) | Append to end of file, update HashMap |
| `get(key)` | O(1) | HashMap lookup + one file seek + read |
| `delete(key)` | O(1) | Append tombstone, remove from HashMap |
| `rebuild_index()` | O(n) | Scan entire file on startup |
| Space usage | O(n * updates) | Every update adds a new record; old records are dead weight |

The key insight is the **trade-off between write performance and space efficiency.** By making writes O(1) append-only, we pay for it in disk space — every update to a key creates a new record while the old one sits unused in the log.

Compare this to an in-place update model (like a B-tree):

| | Log-structured (BitCask) | In-place (B-tree) |
|---|---|---|
| Write | O(1) — sequential append | O(log n) — find position, possibly split nodes |
| Read | O(1) — HashMap + seek | O(log n) — tree traversal |
| Startup | O(n) — scan the log | O(1) — tree is always up to date |
| Space | Grows with updates | Stable — updates overwrite |

Neither is universally better. Log-structured storage shines when writes dominate reads (event logs, metrics, time-series data). B-trees shine when reads dominate and disk space matters (OLTP databases, file systems).

The startup cost — O(n) index rebuild — is the Achilles' heel of pure BitCask. If the log file grows to 10 GB, startup takes minutes. Real implementations solve this with **hint files** (a snapshot of the index saved periodically) and **compaction** (rewriting the log to remove dead records). We will address compaction in a later chapter.

---
