## DSA in Context: The Volcano Model

The Volcano model (also called the iterator model or pull-based execution) was formalized by Goetz Graefe in his 1994 paper "Volcano — An Extensible and Parallel Query Evaluation System." Nearly every relational database uses some variant of this model. Understanding it gives you vocabulary for database internals interviews and system design discussions.

### Pull-based vs push-based execution

In the **pull model** (Volcano), the top operator drives execution. It calls `next()` on its child, which calls `next()` on its child, and so on down to the scan:

```
          Project.next()
              |
              v
          Filter.next()
              |
              v
          Scan.next()
              |
              v
          [Storage]
```

Control flows top-down. Data flows bottom-up. This is the model we built.

In the **push model**, the scan drives execution. It reads rows and pushes them to the next operator, which pushes results to the next, and so on up to the root:

```
          [Storage]
              |
              v
          Scan.produce()
              |
              v
          Filter.consume() -> Filter.produce()
              |
              v
          Project.consume() -> Project.produce()
```

The push model can be more efficient for complex pipelines because it avoids the overhead of function calls up and down the tree on every row. Modern databases like HyPer and Peloton use push-based or hybrid models.

### Why Volcano won (for teaching)

The pull model is simpler to implement and reason about:

1. **Each operator is self-contained.** A `FilterExecutor` knows nothing about what is above or below it. It just pulls from its child and yields rows.

2. **Composition is natural.** Building `Project(Filter(Scan))` is just nesting constructors. Adding a new operator type does not affect existing operators.

3. **Lazy evaluation is automatic.** If the top operator stops pulling (LIMIT, early termination), the pipeline stops. No cancellation protocol needed.

4. **Error handling is straightforward.** Errors propagate up through `Result` values. No callbacks, no error channels, no exception handling across thread boundaries.

The tradeoff is performance: each `next()` call is a virtual function call (because our executors are behind `Box<dyn Executor>`), and virtual calls are harder for the CPU to predict than direct calls. For tables with millions of rows, this per-row overhead adds up. That is why production databases use techniques like vectorized execution (processing batches of rows instead of one at a time) to amortize the call overhead.

### Complexity analysis

For a query `SELECT columns FROM table WHERE predicate`:

| Operation | Time complexity |
|-----------|----------------|
| Full table scan | O(n) where n = rows in table |
| Filter (no index) | O(n) — must examine every row |
| Project | O(k) per row where k = number of output columns |
| Overall | O(n * k) — linear in table size |

The Volcano model does not change the asymptotic complexity — it changes the constant factor and the memory usage. An eager evaluation stores all intermediate results in memory. The Volcano model uses O(1) memory per operator (just the current row), regardless of table size. For a 10-million-row table, the eager approach might need gigabytes of intermediate memory; the Volcano approach needs only a few kilobytes.

---
