# Chapter 9: Query Optimizer

In the last chapter, we built a query planner. Given `SELECT name FROM users WHERE age > 18`, the planner can produce a plan tree: Project(Filter(Scan)). But the plans it produces are naive. Consider this query:

```sql
SELECT name FROM users WHERE 1 + 1 = 2
```

The planner creates a filter that, for every single row in the `users` table, computes `1 + 1`, compares the result to `2`, and only then decides whether to keep the row. If your table has a million rows, that is a million additions and a million comparisons -- all for an expression that is always true. A human can see instantly that `1 + 1 = 2` is always true, so the filter should be removed entirely.

This is what an optimizer does. It rewrites a plan into a different plan that produces the exact same results but does less work. Think of it like a GPS rerouting you around traffic -- you end up at the same destination, but you get there faster because you took a smarter route.

This chapter builds a query optimizer. Along the way, you will learn one of Rust's most powerful features: **trait objects and dynamic dispatch**. This is how Rust lets you store different types in the same collection and call methods on them without knowing the concrete type at compile time.

By the end of this chapter, you will have:

- A `trait OptimizerRule` that defines a single transformation on a plan tree
- A `Vec<Box<dyn OptimizerRule>>` that stores different rules and applies them in sequence
- A constant folding rule that evaluates expressions like `1 + 1` at plan time
- A filter pushdown rule that moves filters closer to their data source
- A deep understanding of trait objects, `Box<dyn Trait>`, static vs dynamic dispatch, and vtables

---
