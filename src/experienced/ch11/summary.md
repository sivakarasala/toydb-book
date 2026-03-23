## What You Built

In this chapter, you:

1. **Built `NestedLoopJoinExecutor`** — O(n*m) brute-force join with state machine for Volcano-model iteration across a double loop
2. **Built `HashJoinExecutor`** — O(n+m) hash join with build/probe phases, `HashableValue` wrapper for HashMap keys, and Entry API for build table construction
3. **Built `AggregateExecutor`** — groups rows by key columns using HashMap, computes COUNT/SUM/AVG/MIN/MAX with `Accumulator` structs, handles NULL values per SQL standard
4. **Built `SortExecutor`** — decorate-sort-undecorate pattern, multi-key sorting with `sort_by`, ascending/descending direction, NULL-first ordering
5. **Built `compare_values`** — comprehensive value comparison with numeric coercion, string lexicographic ordering, and NULL handling
6. **Composed complex pipelines** — Sort(Aggregate(Filter(Join(Scan, Scan)))) producing correct results for multi-table analytical queries

Your database now handles the core SQL operations that make relational databases powerful. Chapter 12 wraps the engine in a TCP server so clients can connect over the network and send SQL queries.

---

### DS Deep Dive

Our hash join builds the hash table in memory. What happens when the hash table does not fit? This deep dive explores grace hash joins, external sorting, and the memory management strategies that let databases handle datasets larger than available RAM.

**-> [External Algorithms — "The Warehouse Overflow"](../ds-narratives/ch11-external-algorithms.md)**

---

### Reference implementation

The files you built in this chapter correspond to these files in the reference codebase:

| Your file | Reference |
|-----------|-----------|
| `src/executor.rs` — `NestedLoopJoinExecutor` | [`src/sql/execution/join.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/sql/execution/join.rs) — `NestedLoopJoin` |
| `src/executor.rs` — `HashJoinExecutor` | [`src/sql/execution/join.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/sql/execution/join.rs) — `HashJoin` |
| `src/executor.rs` — `AggregateExecutor` | [`src/sql/execution/aggregate.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/sql/execution/aggregate.rs) — `Aggregation` |
| `src/executor.rs` — `SortExecutor` | [`src/sql/execution/transform.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/sql/execution/transform.rs) — `Order` |
| `src/executor.rs` — `compare_values` | [`src/sql/types/expression.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/sql/types/expression.rs) — value ordering |
