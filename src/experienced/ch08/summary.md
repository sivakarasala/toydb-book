## What You Built

In this chapter, you:

1. **Defined `Plan` nodes** — a tree of execution instructions (`Scan`, `Filter`, `Project`, `Insert`, `Update`, `Delete`, `CreateTable`) using recursive enums with `Box<Plan>`
2. **Built a `Schema` catalog** — a `HashMap`-backed registry of tables and columns, with lookup methods using iterator methods like `find()`, `any()`, and `map()`
3. **Built the `Planner`** — transforms AST `Statement`s into validated `Plan` trees, resolving table names, validating columns, and type-checking expressions
4. **Added schema validation** — catches missing tables, missing columns, type mismatches, and invalid column counts with descriptive `PlanError` messages
5. **Implemented `Display` for `Plan`** — prints the plan tree as an indented structure (like SQL `EXPLAIN`), using recursive formatting with depth tracking
6. **Practiced iterators and closures** throughout — `filter_map`, `map`, `collect`, `extend`, `any`, `find`, and the critical `collect::<Result<Vec<_>, _>>()` pattern

The planner is the third stage in the SQL pipeline: Lexer -> Parser -> Planner. The plan it produces is a validated, self-contained description of what the executor needs to do. In Chapter 9, we will add an optimizer that takes these plans and makes them faster. In Chapter 10, the executor will walk the plan tree and produce actual results.

---

### DS Deep Dive

Ready to go deeper? This chapter's data structure deep dive explores tree transformations in detail — building a generic tree mapper, comparing pre-order vs post-order transforms, and showing how compiler passes use the same AST-to-IR pattern that our planner uses.

**-> [Tree Transformations: AST to Plan](../../ds-narratives/ch08-tree-transformations.md)**

---

### Reference

The files you built in this chapter:

| Your file | Purpose |
|-----------|---------|
| `src/types.rs` | `Value` and `DataType` enums (from earlier chapters) |
| `src/sql/ast.rs` | AST types: `Statement`, `Expression`, `SelectColumn`, `Operator`, `ColumnDef` (from Chapter 7) |
| `src/sql/schema.rs` | `Schema` catalog with `Table` and `Column` metadata |
| `src/sql/plan.rs` | `Plan` enum, `ColumnInfo`, and `Display` implementation |
| `src/sql/planner.rs` | `Planner` struct, `PlanError`, expression validation, and type inference |
