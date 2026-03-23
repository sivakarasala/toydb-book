## What You Built

In this chapter, you:

1. **Defined the `Executor` trait** — a two-method interface (`next` and `columns`) that every query operator implements, embodying the Volcano model
2. **Built `ScanExecutor`** — reads rows from in-memory storage one at a time, the leaf node of every executor tree
3. **Built `FilterExecutor`** — evaluates predicate expressions against each row, passing through only matches, with a loop that pulls from the child until it finds a match or exhausts the source
4. **Built `ProjectExecutor`** — evaluates column expressions against each row, producing new rows with selected columns, deriving output schema from expressions
5. **Built the expression evaluator** — `evaluate()` function handling literals, column references, binary operations (arithmetic, comparison, logical), and unary operations with type coercion
6. **Built `build_executor`** — converts a `Plan` tree into a nested `Executor` tree, bridging the optimizer and the executor
7. **Built `ResultSet`** — collects all rows from an executor and pretty-prints them as a table

Your database can now answer questions. Give it `SELECT name FROM users WHERE age > 28`, and it returns `Alice, Carol`. The pipeline — lex, parse, plan, optimize, execute — is complete for simple queries.

Chapter 11 extends the executor with operators for the hard parts: joins (combining rows from multiple tables), aggregations (COUNT, SUM, AVG), GROUP BY, and ORDER BY. The Volcano model's composability means each new operator is just another struct implementing `Executor`.

---

### DS Deep Dive

Our executor evaluates expressions by walking the `Expression` tree recursively. Production databases compile expressions into bytecode or machine code for faster evaluation. This deep dive explores expression compilation, JIT (just-in-time) compilation in databases, and how the tradeoff between interpretation and compilation changes based on query complexity and data volume.

**-> [Expression Evaluation — "The Row Assembly Line"](../ds-narratives/ch10-expression-evaluation.md)**

---

### Reference implementation

The files you built in this chapter correspond to these files in the reference codebase:

| Your file | Reference |
|-----------|-----------|
| `src/executor.rs` — `Executor` trait | [`src/sql/execution/mod.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/sql/execution/mod.rs) — `Executor` trait |
| `src/executor.rs` — `ScanExecutor` | [`src/sql/execution/source.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/sql/execution/source.rs) — `Scan` |
| `src/executor.rs` — `FilterExecutor` | [`src/sql/execution/transform.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/sql/execution/transform.rs) — `Filter` |
| `src/executor.rs` — `ProjectExecutor` | [`src/sql/execution/transform.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/sql/execution/transform.rs) — `Projection` |
| `src/executor.rs` — `evaluate()` | [`src/sql/types/expression.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/sql/types/expression.rs) — `evaluate()` |
| `src/executor.rs` — `build_executor()` | [`src/sql/execution/mod.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/sql/execution/mod.rs) — `build()` |
