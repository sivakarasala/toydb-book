## DSA in Context: BTreeMap vs HashMap

You used a `BTreeMap` to back your storage engine. Here is why that choice matters, examined through the lens of algorithmic complexity and real-world database behavior.

### The B-tree

A B-tree is a self-balancing tree where each node can hold multiple keys. Rust's `BTreeMap` uses a B-tree with a branching factor tuned for cache performance (typically 6-11 keys per node). The key property: **all keys are stored in sorted order**.

```
                    [   dog   |   mango   ]
                   /          |            \
          [apple|banana]   [cherry|date]   [orange|zebra]
```

Each node holds multiple keys in sorted order. Child pointers sit between and around the keys. To find "cherry", you start at the root: "cherry" is between "dog" and "mango" is wrong — "cherry" < "dog", so go left? No — "cherry" > "banana", so it is in the middle child. One comparison per level, and the tree is shallow because each node holds many keys.

### Complexity comparison

| Operation | `HashMap` | `BTreeMap` |
|-----------|-----------|-----------|
| `get(key)` | O(1) average, O(n) worst | O(log n) |
| `insert(key, value)` | O(1) average, O(n) worst | O(log n) |
| `remove(key)` | O(1) average, O(n) worst | O(log n) |
| `range(a..b)` | Not supported | O(log n + k) |
| `iteration order` | Arbitrary | Sorted by key |
| `min / max key` | O(n) | O(log n) |

For a million keys, O(log n) means about 20 comparisons. O(1) means one hash computation. The `HashMap` wins on raw point-lookup speed, but it cannot answer "give me the next 100 keys after this one" without a full scan and sort.

### Why databases choose B-trees

Databases are not just key-value stores. They answer queries like:

- `SELECT * FROM users WHERE id BETWEEN 100 AND 200` (range scan)
- `SELECT * FROM users ORDER BY name LIMIT 10` (ordered iteration)
- `SELECT MIN(created_at) FROM events` (minimum key)

All of these require ordered data. A hash table makes these operations O(n) — you must scan every entry. A B-tree makes them O(log n + k) — find the start point, then walk forward.

This is why PostgreSQL, MySQL (InnoDB), SQLite, and most relational databases use B-tree variants for their indexes. The O(1) point lookup of a hash table does not compensate for the inability to scan ranges efficiently.

Our `MemoryStorage` inherits these properties from `BTreeMap`. When we test `scan("banana"..="date")`, the `BTreeMap` finds "banana" in O(log n) and iterates to "date" without visiting any other keys. The data structure does the heavy lifting.

---
