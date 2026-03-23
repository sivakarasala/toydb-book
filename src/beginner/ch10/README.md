# Chapter 10: Query Executor -- The Volcano Model

Your database can parse SQL, plan queries, and optimize plans. But it still cannot answer a single question. The optimizer produces a beautiful tree of `Plan` nodes -- `Project(Filter(Scan))` -- yet no code actually reads rows from storage, evaluates conditions, or selects columns. The plan is a blueprint. The executor is the construction crew that follows the blueprint and builds the actual result.

Think of it this way: the plan says WHAT to do ("scan the users table, keep rows where age > 18, return only the name column"). The executor actually DOES it -- it reads the data, checks the conditions, and produces rows one at a time.

This chapter builds an iterator-based query executor using the **Volcano model** -- the same architecture used by PostgreSQL, MySQL, SQLite, and most production databases. The core idea is simple: each operator (Scan, Filter, Project) produces rows one at a time through a `next()` method. Operators compose: a `FilterExecutor` wraps a `ScanExecutor`, pulling rows and passing through only those that match. A `ProjectExecutor` wraps a `FilterExecutor`, keeping only specific columns.

By the end of this chapter, you will have:

- A `Row` type representing a single database row as a vector of values
- An `Executor` trait with `fn next(&mut self) -> Result<Option<Row>, ExecutorError>`
- A `ScanExecutor` that reads rows from an in-memory table
- A `FilterExecutor` that evaluates expressions against each row and passes through matches
- A `ProjectExecutor` that selects specific columns from each row
- A working end-to-end pipeline: plan in, rows out

---
