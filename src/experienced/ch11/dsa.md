## DSA in Context: Join Algorithms

Joins are the heart of relational databases and a favorite interview topic. Understanding the three main join algorithms — and when to use each — demonstrates that you understand the tradeoffs between time, memory, and data access patterns.

### Nested Loop Join — O(n * m)

```
For each row in left (n rows):
    For each row in right (m rows):
        If condition: emit combined row
```

**When to use:** When one table is very small (< 100 rows), or when you have an index on the join column of the inner table (turning it into an **index nested loop join** with O(n * log m) complexity).

**Memory:** O(1) if neither side is materialized. Our implementation materializes the right side for simplicity, using O(m) memory.

### Hash Join — O(n + m)

```
Build phase:
    For each row in right (smaller table, m rows):
        Insert into hash table keyed by join column

Probe phase:
    For each row in left (larger table, n rows):
        Look up join column in hash table
        For each match: emit combined row
```

**When to use:** When both tables are large and no useful index exists. The hash join is the workhorse of analytics queries.

**Memory:** O(m) for the hash table. The smaller table should be the build side. If the hash table does not fit in memory, the database uses a **grace hash join** that partitions both tables by hash value and joins each partition independently.

### Merge Join — O(n log n + m log m)

```
Sort left by join column
Sort right by join column
Merge:
    Advance pointers through both sorted lists
    When keys match: emit combined row
```

**When to use:** When both tables are already sorted on the join column (from an index or a previous ORDER BY). In that case, the sort step is free and the merge is O(n + m). Also useful when the result must be sorted by the join column.

**Memory:** O(1) for the merge step (assuming tables are already sorted). O(n + m) if sorting is needed.

### Comparison

| Algorithm | Time | Memory | Best when |
|-----------|------|--------|-----------|
| Nested Loop | O(n * m) | O(1) | One table is tiny, or indexed |
| Hash Join | O(n + m) | O(min(n,m)) | Large tables, no index, no order needed |
| Merge Join | O(n + m) | O(1) | Tables already sorted on join key |

### Join ordering

For multi-table joins, the order matters enormously. Joining three tables A (1000 rows), B (1,000,000 rows), C (10 rows):

- `(A JOIN B) JOIN C`: A JOIN B produces up to 1 billion intermediate rows
- `(A JOIN C) JOIN B`: A JOIN C produces up to 10,000 intermediate rows, then joined with B

The query optimizer's job is to find the best join order. This is an NP-hard problem in general, so databases use heuristics (small tables first, selective joins first) or dynamic programming (exhaustive search for up to ~10 tables).

---
