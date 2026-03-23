## What We Built

In this chapter, you built a working query executor using the Volcano model. Here is what you accomplished:

1. **Row and Value types** -- the fundamental data types of the executor
2. **Executor trait** -- `next()` + `columns()`, the Volcano model interface
3. **ScanExecutor** -- reads rows from storage one at a time
4. **Expression evaluator** -- evaluates expressions like `age > 30` against rows
5. **FilterExecutor** -- wraps a source executor, passes through only matching rows
6. **ProjectExecutor** -- wraps a source executor, keeps only selected columns
7. **build_executor** -- converts a Plan tree into a nested executor chain
8. **EmptyExecutor** -- produces no rows (for optimized-away queries)

The Rust concepts you deepened:

- **Custom iterators** -- implementing `next()` with mutable state to produce values on demand
- **Iterator composition** -- wrapping one iterator/executor inside another to build pipelines
- **Lazy evaluation** -- nothing is computed until `next()` is called; early termination is free
- **`Box<dyn Trait>`** -- executors are trait objects, enabling composition without knowing concrete types
- **Collecting Results** -- using `.collect::<Result<Vec<_>, _>>()` to short-circuit on the first error
- **The `?` operator** -- propagating errors concisely through the call chain

The executor is where your database comes alive. For the first time, you can feed in a plan and get actual rows back. The pipeline SQL string -> tokens -> AST -> plan -> optimized plan -> rows is complete. In the next chapter, we add joins and aggregations.

---
