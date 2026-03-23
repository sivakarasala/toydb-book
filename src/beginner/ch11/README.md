# Chapter 11: SQL Features -- Joins, Aggregations, GROUP BY

Your executor can scan tables, filter rows, and project columns. That covers a surprising amount of SQL. But it misses the operations that make relational databases *relational*: combining data from multiple tables, and summarizing groups of rows. Without joins, every query reads from a single table -- you cannot answer "which users placed which orders?" Without aggregations, you cannot answer "how many users are over 30?" Without ORDER BY, results come back in whatever order they were stored.

This chapter extends the executor with four new operators. Each one is another struct implementing the `Executor` trait, composing with the operators you already have. By the end, your database will handle queries that combine data from multiple tables, group rows, compute totals, and sort results.

By the end of this chapter, you will have:

- A `NestedLoopJoinExecutor` that combines rows from two tables -- O(n * m)
- A `HashJoinExecutor` that uses a hash table for faster joins -- O(n + m)
- An `AggregateExecutor` with GROUP BY and five aggregation functions (COUNT, SUM, AVG, MIN, MAX)
- A `SortExecutor` that collects all rows and sorts them with a custom comparator
- A deep understanding of HashMap, the Entry API, Vec sorting, and the Ord trait

---
