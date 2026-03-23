# Chapter 9: Query Optimizer

Your database can lex SQL into tokens, parse tokens into an AST, and convert the AST into a query plan. But the plan it produces is naive. `SELECT name FROM users WHERE 1 + 1 = 2` generates a plan that scans every row in the `users` table, computes `1 + 1` for each row, compares it to `2`, and only then returns the `name` column. That is absurd. A human can see instantly that `1 + 1 = 2` is always true, so the filter should be removed entirely. The difference between a database that runs this query in 2 microseconds and one that scans a million rows is not a smarter executor — it is a smarter optimizer.

Query optimization is the art of rewriting a plan into a different plan that produces the same results but does less work. The optimizer never changes what the query returns. It changes how the database computes the answer. This is the chapter where your database starts thinking before it acts.

By the end of this chapter, you will have:

- A `trait OptimizerRule` that defines a single transformation on a plan tree
- A `Vec<Box<dyn OptimizerRule>>` that stores heterogeneous rules and applies them in sequence
- A constant folding rule that evaluates expressions like `1 + 1` and `3 > 5` at plan time
- A filter pushdown rule that moves filters closer to their data source
- A full compilation pipeline: SQL string to optimized plan in a single function call
- A deep understanding of trait objects, dynamic dispatch, and when to choose them over generics

---
