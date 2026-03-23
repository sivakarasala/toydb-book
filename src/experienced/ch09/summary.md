## What You Built

In this chapter, you:

1. **Defined the `OptimizerRule` trait** — a two-method interface (`name` and `optimize`) that every optimization rule implements, demonstrating trait objects and dynamic dispatch
2. **Built the `Optimizer` struct** — stores `Vec<Box<dyn OptimizerRule>>`, applies rules in sequence, and reports which rules fired, demonstrating heterogeneous collections through trait objects
3. **Implemented constant folding** — evaluates constant expressions at plan time, removes always-true filters, replaces always-false filters with `EmptyResult`, demonstrating recursive tree transformation
4. **Implemented filter pushdown** — moves filter nodes past project nodes when safe, reducing the rows processed by expensive operations, demonstrating plan tree rewriting with safety checks
5. **Wired the full pipeline** — SQL string to optimized plan through lexer, parser, planner, and optimizer, demonstrating module composition

Your database no longer blindly executes the plan the planner produces. It thinks first. `WHERE 1 + 1 = 2` is eliminated before execution begins. Filters are moved to where they can do the most good. And the framework is extensible — adding a new rule means implementing a trait and adding one line to `default_optimizer()`.

Chapter 10 builds the query executor that takes these optimized plans and actually runs them against the storage engine. The plans your optimizer produces will determine how efficiently the executor works — a well-optimized plan means less disk I/O, less memory, and faster results.

---

### DS Deep Dive

Our optimizer applies rules in a fixed order: constant folding, then filter pushdown. Production optimizers explore a search space of possible plans and use dynamic programming to find the cheapest one. This deep dive explores the Cascades framework, top-down vs bottom-up optimization, and how cost models combine cardinality estimation with I/O and CPU cost functions.

**-> [Query Optimization Theory -- "The Plan Space Explorer"](../ds-narratives/ch09-query-optimization.md)**

---

### Reference implementation

The files you built in this chapter correspond to these files in the reference codebase:

| Your file | Reference |
|-----------|-----------|
| `src/optimizer.rs` — `OptimizerRule` trait | [`src/sql/planner/optimizer.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/sql/planner/optimizer.rs) — optimization rules |
| `src/optimizer.rs` — `ConstantFolding` | [`src/sql/planner/optimizer.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/sql/planner/optimizer.rs) — `ConstantFolder` |
| `src/optimizer.rs` — `FilterPushdown` | [`src/sql/planner/optimizer.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/sql/planner/optimizer.rs) — `FilterPushdown` |
| `src/optimizer.rs` — `fold_constants()` | [`src/sql/planner/optimizer.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/sql/planner/optimizer.rs) — constant evaluation |
| `src/planner.rs` — `Plan` enum | [`src/sql/planner/plan.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/sql/planner/plan.rs) — `Node` enum |
