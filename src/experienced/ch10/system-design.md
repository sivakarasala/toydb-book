## System Design Corner: Vectorized vs Tuple-at-a-Time Execution

The Volcano model we built is **tuple-at-a-time** — each `next()` call produces exactly one row. Modern analytical databases use **vectorized execution**, where each `next()` call produces a *batch* of rows (typically 1,000-4,000 at a time).

### Why vectorize?

The overhead of the tuple-at-a-time model is not the algorithm — it is the function calls. For a query scanning 10 million rows through 5 operators, that is 50 million virtual function calls. Each call involves:

1. Indirect jump (vtable lookup for `dyn Executor`)
2. Function prologue/epilogue
3. Branch prediction miss (the CPU cannot predict which executor will be called)
4. Pipeline stall (modern CPUs execute instructions out of order, but indirect jumps break the pipeline)

Vectorized execution amortizes this overhead:

```rust
// Tuple-at-a-time: 10 million function calls
for _ in 0..10_000_000 {
    let row = executor.next()?;
}

// Vectorized: 10,000 function calls (1,000 rows per batch)
for _ in 0..10_000 {
    let batch: Vec<Row> = executor.next_batch(1000)?;
    // Process 1000 rows in a tight loop (no virtual dispatch)
}
```

Within each batch, the executor processes rows in a tight `for` loop — no virtual dispatch, no indirect jumps. The CPU can predict the loop, prefetch data, and use SIMD instructions (processing 4 or 8 values simultaneously).

### Column stores vs row stores

Our storage is a **row store** — each row contains all columns: `[1, "Alice", 30]`. Column stores flip this: each column is stored separately.

```
Row store:
  row 0: [1, "Alice", 30]
  row 1: [2, "Bob",   25]
  row 2: [3, "Carol", 35]

Column store:
  id:   [1, 2, 3]
  name: ["Alice", "Bob", "Carol"]
  age:  [30, 25, 35]
```

Column stores are better for analytical queries that touch few columns across many rows (`SELECT AVG(age) FROM users` reads only the `age` column). Row stores are better for transactional queries that touch all columns of few rows (`SELECT * FROM users WHERE id = 3`).

### Production database architectures

| Database | Execution model | Storage model | Notes |
|----------|----------------|---------------|-------|
| PostgreSQL | Tuple-at-a-time (Volcano) | Row store | Most traditional RDBMS |
| MySQL | Tuple-at-a-time | Row store (InnoDB) | |
| SQLite | Bytecode interpreter | Row store (B-tree pages) | Not Volcano — uses a VM |
| DuckDB | Vectorized | Column store | Designed for analytics |
| ClickHouse | Vectorized | Column store | High-performance analytics |
| CockroachDB | Vectorized | Row store (LSM) | Distributed SQL |
| TiDB | Chunk-based (vectorized) | Row store (TiKV) | Distributed SQL |

> **Interview talking point:** *"Our query executor uses the Volcano model — each operator implements a `next()` method that yields one row at a time. Operators compose into a pipeline: Project(Filter(Scan)). This gives us lazy evaluation (LIMIT stops the pipeline early), O(1) memory per operator, and clean error propagation through Result. For production workloads, I would consider vectorized execution to amortize the per-row virtual dispatch overhead — processing batches of 1,000 rows at a time reduces function call overhead by 1,000x and enables SIMD optimization."*

---
