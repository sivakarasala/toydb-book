# Chapter 11: SQL Features — Joins, Aggregations, GROUP BY

Your executor can scan tables, filter rows, and project columns. That covers a surprising amount of SQL, but it misses the operations that make relational databases relational: combining data from multiple tables (joins), summarizing groups of rows (aggregations), and ordering results. Without joins, every query reads from a single table. Without aggregations, you cannot answer "how many users are over 30?" Without ORDER BY, results come back in storage order — which might be insertion order, or might be random after deletions and compactions.

This chapter extends the executor with four new operators. Each one is another struct implementing the `Executor` trait, composing with the operators you already have. By the end, your database will handle queries like `SELECT department, COUNT(*), AVG(salary) FROM employees JOIN departments ON employees.dept_id = departments.id GROUP BY department ORDER BY COUNT(*) DESC`.

The spotlight concept is **collections and algorithms** — `HashMap` for grouping and hash joins, `BTreeMap` for ordered output, `Vec` for sorting, the `Entry` API for ergonomic accumulation, and custom comparators with `Ord`.

By the end of this chapter, you will have:

- A `NestedLoopJoinExecutor` that combines rows from two tables — O(n*m)
- A `HashJoinExecutor` that builds a hash table on the smaller table and probes with the larger — O(n+m)
- An `AggregateExecutor` with GROUP BY and five aggregation functions (COUNT, SUM, AVG, MIN, MAX)
- A `SortExecutor` that collects all rows and sorts them with a custom comparator
- A deep understanding of HashMap, BTreeMap, Entry API, and custom Ord implementations

---
