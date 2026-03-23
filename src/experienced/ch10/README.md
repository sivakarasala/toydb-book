# Chapter 10: Query Executor — The Volcano Model

Your database can parse SQL, plan queries, and optimize plans. But it still cannot answer a single question. The optimizer produces a beautiful tree of `Plan` nodes — `Project(Filter(Scan))` — yet no code actually reads rows from storage, evaluates predicates, or selects columns. The plan is a blueprint. The executor is the construction crew.

This chapter builds an iterator-based query executor using the Volcano model — the same architecture used by PostgreSQL, MySQL, SQLite, and most production databases. Each operator (Scan, Filter, Project) implements a single method: `next()`. It returns one row at a time. Operators compose: a `FilterExecutor` wraps a `ScanExecutor`, pulling rows from it and passing through only those that satisfy a predicate. A `ProjectExecutor` wraps a `FilterExecutor`, selecting specific columns from each row. The entire query becomes a chain of iterators, evaluated lazily from top to bottom.

The spotlight concept is **advanced iterators** — custom `Iterator` implementations, composing iterators, lazy versus eager evaluation, and why the pull-based model matters for databases that process tables with millions of rows.

By the end of this chapter, you will have:

- A `Row` type representing a single database row as a vector of `Value`s
- A `trait Executor` with `fn next(&mut self) -> Result<Option<Row>, ExecutorError>`
- A `ScanExecutor` that reads rows from an in-memory table
- A `FilterExecutor` that evaluates expressions against each row and passes through matches
- A `ProjectExecutor` that selects specific columns from each row
- A pipeline builder that converts an optimized `Plan` into a nested executor tree
- A working end-to-end query: SQL string in, rows out

---
