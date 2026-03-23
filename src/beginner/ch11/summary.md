## What We Built

In this chapter, you extended the executor with four powerful operators:

1. **NestedLoopJoinExecutor** -- combines rows from two tables using a double loop (O(n * m))
2. **HashJoinExecutor** -- combines rows using a hash table for O(n + m) performance
3. **AggregateExecutor** -- groups rows and computes COUNT, SUM, AVG, MIN, MAX
4. **SortExecutor** -- sorts all rows by arbitrary expressions

The Rust concepts you learned:

- **HashMap** -- O(1) key-value lookup, essential for hash joins and grouping
- **Entry API** -- `entry().or_insert()` for ergonomic insert-or-update patterns
- **Vec::sort_by** -- sorting with custom comparators using closures
- **PartialOrd** -- defining how custom types are compared
- **Iterator::chain** -- combining two iterators end-to-end
- **State machines in iterators** -- maintaining loop state across `next()` calls (the join executor pattern)
- **Eager vs lazy execution** -- aggregation and sorting must see all data; scan, filter, and project can be lazy

Your database now handles a wide range of SQL: single-table queries, joins across tables, aggregations with grouping, and sorted output. In the next chapter, we make it accessible over the network.

---
