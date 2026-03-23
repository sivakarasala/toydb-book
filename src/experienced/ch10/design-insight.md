## Design Insight: Deep Modules

In *A Philosophy of Software Design*, Ousterhout defines a **deep module** as one with a simple interface hiding significant complexity. A **shallow module** has a complex interface relative to its functionality.

The `Executor` trait is a deep module. Its interface is two methods:

```rust
pub trait Executor {
    fn next(&mut self) -> Result<Option<Row>, ExecutorError>;
    fn columns(&self) -> &[String];
}
```

Behind this simple interface, each executor hides significant complexity:

- `ScanExecutor` hides storage access, row iteration, and schema resolution
- `FilterExecutor` hides expression evaluation, type coercion, NULL handling, and the loop that pulls rows until a match is found
- `ProjectExecutor` hides expression evaluation, column name derivation, and row reconstruction

The caller of any executor sees the same interface: call `next()`, get a row or `None`. It does not matter if the executor is reading from disk, evaluating a complex predicate, performing a join, or sorting a million rows. The interface is the same.

Compare this to a shallow module design where each operator exposes its internals:

```rust
// Shallow: the caller must understand each operator's implementation
let scan = ScanExecutor::new(&storage, "users")?;
let rows = scan.read_all_rows()?;
let filtered = filter_rows(&rows, &predicate, &scan.columns())?;
let projected = project_rows(&filtered, &expressions, &scan.columns())?;
```

This exposes three concerns to the caller: row materialization, filtering, and projection. The caller must manage the data flow between them. Adding a new operator (sort, join, limit) requires changing the caller.

With deep modules:

```rust
// Deep: the caller sees a uniform interface
let executor = build_executor(plan, &storage)?;
let result = ResultSet::collect_from(executor)?;
```

Two lines. Adding sort, join, limit, aggregation — none of these change the caller. The plan gets more complex, the executor tree gets deeper, but the interface stays the same.

This is why the Volcano model has survived for 30 years. Not because it is the fastest execution model (vectorized execution is faster). But because its interface is deep — one method hides an arbitrary amount of complexity — and that simplicity composes beautifully.

---
