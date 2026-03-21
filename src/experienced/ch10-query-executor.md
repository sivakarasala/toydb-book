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

## Spotlight: Advanced Iterators

Every chapter has one spotlight concept. This chapter's spotlight is **advanced iterators** — moving beyond basic `for` loops and `.map()` chains into custom iterator implementations, composition patterns, and the design philosophy behind Rust's iterator system.

### Review: the Iterator trait

You have seen Rust's `Iterator` trait before:

```rust
pub trait Iterator {
    type Item;
    fn next(&mut self) -> Option<Self::Item>;
}
```

One method. One associated type. That is the entire contract. Everything else — `map`, `filter`, `take`, `chain`, `collect`, `fold`, `zip`, `enumerate`, `peekable` — is built on top of this single method. The standard library provides over 70 adaptor methods on `Iterator`, all implemented in terms of `next()`.

### Custom iterators: beyond Vec and slices

Most Rust iterators you have used come from collections — `vec.iter()`, `map.keys()`, `string.chars()`. But any struct can be an iterator. All it needs is `impl Iterator` with a `next()` method:

```rust
/// An iterator that counts from a start value, incrementing by a step.
struct StepCounter {
    current: i64,
    step: i64,
    end: i64,
}

impl StepCounter {
    fn new(start: i64, end: i64, step: i64) -> Self {
        StepCounter { current: start, step, end }
    }
}

impl Iterator for StepCounter {
    type Item = i64;

    fn next(&mut self) -> Option<i64> {
        if self.current >= self.end {
            return None;
        }
        let value = self.current;
        self.current += self.step;
        Some(value)
    }
}

fn main() {
    let evens: Vec<i64> = StepCounter::new(0, 10, 2).collect();
    println!("{:?}", evens); // [0, 2, 4, 6, 8]
}
```

The iterator holds mutable state (`current`) and produces values on demand. Nothing is computed until `next()` is called. This is exactly how our database executors will work — each executor holds state (a position in a table, a buffer of rows, a child executor) and produces rows one at a time.

### Composing iterators: the adaptor pattern

The power of iterators comes from composition. Each adaptor wraps an existing iterator and transforms its output:

```rust
let numbers = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

let result: Vec<i64> = numbers.iter()
    .filter(|&&n| n % 2 == 0)      // keep even numbers
    .map(|&n| n * n)                // square each one
    .take(3)                         // stop after 3 results
    .collect();                      // gather into a Vec

println!("{:?}", result); // [4, 16, 36]
```

This pipeline does NOT:
1. Filter all 10 numbers, creating a temporary Vec of evens
2. Square all evens, creating another temporary Vec
3. Take 3 from the squared Vec

Instead, it pulls one number at a time through the entire chain. `take(3)` calls `map.next()`, which calls `filter.next()`, which calls `iter.next()`. When `take` has accumulated 3 results, it stops — numbers 7 through 10 are never even examined. This is lazy evaluation, and it is exactly how the Volcano model works.

### Peekable: looking ahead without consuming

Sometimes you need to look at the next value without advancing the iterator. `Peekable` wraps any iterator and adds a `peek()` method:

```rust
let mut iter = vec![1, 2, 3].into_iter().peekable();

// Look at the next value without consuming it
assert_eq!(iter.peek(), Some(&1));
assert_eq!(iter.peek(), Some(&1)); // still 1 — peek does not advance

// Now consume it
assert_eq!(iter.next(), Some(1));
assert_eq!(iter.peek(), Some(&2)); // now it's 2
```

This is useful when parsing or processing sequences where decisions depend on the upcoming value. Our SQL lexer used `peek()` in Chapter 6 to decide whether `>` is `GreaterThan` or `>=` is `GreaterOrEqual`.

### Chain: concatenating iterators

`chain` links two iterators end-to-end:

```rust
let first = vec![1, 2, 3];
let second = vec![4, 5, 6];

let all: Vec<i32> = first.into_iter().chain(second).collect();
println!("{:?}", all); // [1, 2, 3, 4, 5, 6]
```

In a database, you might use `chain` to implement `UNION` — appending the results of one query to another.

### Lazy vs eager: why it matters for databases

Consider a table with 10 million rows and a query that selects the first 5 rows matching a condition:

```sql
SELECT name FROM users WHERE age > 30 LIMIT 5;
```

**Eager evaluation** (what Python/JavaScript do by default): scan all 10 million rows, filter down to matching rows (maybe 3 million), then take 5. You touched 10 million rows to produce 5.

**Lazy evaluation** (the Volcano model): pull one row from the scan, check the filter, if it passes, add to results. Repeat until you have 5 results. If the first 5 matching rows are in the first 100 rows of the table, you only read 100 rows — not 10 million.

This is why every production database uses an iterator-based (pull) model. The operator at the top of the tree controls how many rows flow through the system. A `LIMIT 5` at the top means the entire pipeline stops after producing 5 rows, regardless of how large the table is.

> **Coming from JS/Python/Go?**
>
> | Concept | JavaScript | Python | Go | Rust |
> |---------|-----------|--------|-----|------|
> | Custom iterator | `[Symbol.iterator]() { return { next() {} } }` | `__iter__` + `__next__` | No built-in; use channels | `impl Iterator` |
> | Lazy chain | None built-in (arrays are eager) | Generators (`yield`) | Channels | `.filter().map().take()` |
> | Peek ahead | No built-in | `itertools.peekable()` | No built-in | `.peekable()` |
> | Collect results | `Array.from(iter)` | `list(iter)` | `for range` into slice | `.collect::<Vec<_>>()` |
> | Early termination | `.find()` stops | `next(filter(...))` | `break` in loop | `.take(n)`, `.find()` |
>
> The biggest difference: Rust's iterator adaptors are zero-cost abstractions. The compiler inlines the entire chain of `.filter().map().take()` into a single loop with no intermediate allocations, no virtual dispatch, no heap-allocated closures. The generated machine code is identical to a hand-written `for` loop with `if` statements. JavaScript and Python iterators carry per-element overhead from function calls and dynamic dispatch.

---

## Exercise 1: Define the Executor Trait and Row Type

**Goal:** Create the foundational types for the executor: a `Row` type, an error type, and the `Executor` trait that every operator will implement.

### Step 1: Recall your types

By now your project has these types from earlier chapters. Here is the subset the executor needs:

```rust
// src/types.rs (from Chapters 2-4, 8-9)

/// A database value. Every cell in every row is one of these.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Boolean(bool),
    Integer(i64),
    Float(f64),
    String(String),
}
```

And from the planner/optimizer (Chapters 8-9):

```rust
// src/planner.rs

#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    Literal(Value),
    ColumnRef(String),
    BinaryOp {
        left: Box<Expression>,
        op: BinaryOperator,
        right: Box<Expression>,
    },
    UnaryOp {
        op: UnaryOperator,
        operand: Box<Expression>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum BinaryOperator {
    Add, Subtract, Multiply, Divide,
    Equal, NotEqual, LessThan, GreaterThan, LessOrEqual, GreaterOrEqual,
    And, Or,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOperator {
    Not, Negate,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Plan {
    Scan { table: String },
    Filter { predicate: Expression, source: Box<Plan> },
    Project { columns: Vec<Expression>, source: Box<Plan> },
    EmptyResult,
}
```

### Step 2: Define the Row type

A `Row` in the executor is a vector of `Value`s, one per column. We also need to know the column names so that `ColumnRef("name")` expressions can find the right value:

Create `src/executor.rs`:

```rust
// src/executor.rs

use crate::types::Value;
use crate::planner::{Expression, BinaryOperator, UnaryOperator, Plan};
use std::fmt;

/// A row of values, one per column.
/// The column order matches the column_names in the ResultSet.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub values: Vec<Value>,
}

impl Row {
    pub fn new(values: Vec<Value>) -> Self {
        Row { values }
    }
}

impl fmt::Display for Row {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let parts: Vec<String> = self.values.iter().map(|v| format!("{}", v)).collect();
        write!(f, "({})", parts.join(", "))
    }
}
```

We also need a `Display` impl for `Value` if you do not have one already:

```rust
// src/types.rs — add this impl

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Null => write!(f, "NULL"),
            Value::Boolean(b) => write!(f, "{}", b),
            Value::Integer(i) => write!(f, "{}", i),
            Value::Float(fl) => write!(f, "{:.2}", fl),
            Value::String(s) => write!(f, "{}", s),
        }
    }
}
```

### Step 3: Define the error type

```rust
// src/executor.rs (continued)

/// Errors that can occur during query execution.
#[derive(Debug, Clone, PartialEq)]
pub enum ExecutorError {
    /// Referenced column not found in the current row's schema.
    ColumnNotFound(String),
    /// Type mismatch during expression evaluation (e.g., adding a string to an integer).
    TypeError(String),
    /// Table not found in the storage layer.
    TableNotFound(String),
    /// Division by zero.
    DivisionByZero,
    /// Generic execution error.
    Internal(String),
}

impl fmt::Display for ExecutorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExecutorError::ColumnNotFound(col) => write!(f, "column not found: {}", col),
            ExecutorError::TypeError(msg) => write!(f, "type error: {}", msg),
            ExecutorError::TableNotFound(table) => write!(f, "table not found: {}", table),
            ExecutorError::DivisionByZero => write!(f, "division by zero"),
            ExecutorError::Internal(msg) => write!(f, "internal error: {}", msg),
        }
    }
}
```

### Step 4: Define the Executor trait

Now the core abstraction. Every query operator — scan, filter, project, join, sort — will implement this trait:

```rust
// src/executor.rs (continued)

/// The core executor interface — the Volcano model.
///
/// Every operator implements this trait. Calling `next()` produces the
/// next row in the result, or `None` when the operator is exhausted.
/// Operators compose: a FilterExecutor wraps a ScanExecutor,
/// a ProjectExecutor wraps a FilterExecutor, and so on.
pub trait Executor {
    /// Returns the next row, or None if no more rows are available.
    fn next(&mut self) -> Result<Option<Row>, ExecutorError>;

    /// Returns the column names for the rows this executor produces.
    /// This is needed so that ColumnRef expressions can resolve column names to indices.
    fn columns(&self) -> &[String];
}
```

Two methods. `next()` is the Volcano model — pull one row at a time. `columns()` provides schema information so that expression evaluation can map column names to positions.

### Step 5: Understand the design

Why `Result<Option<Row>, ExecutorError>` instead of just `Option<Row>`?

- `Ok(Some(row))` — here is the next row
- `Ok(None)` — no more rows; the executor is exhausted
- `Err(e)` — something went wrong (type error, missing column, division by zero)

This three-state return is essential. Without `Result`, errors would need to be encoded as special rows or panics. Without `Option`, the executor would need a separate `has_next()` method, which leads to off-by-one bugs and breaks the composability of the Volcano model.

Compare this to Rust's standard `Iterator` trait, which returns `Option<Self::Item>`. Our executor is essentially `Iterator<Item = Result<Row, ExecutorError>>`, but we define a custom trait because:

1. We need the `columns()` method for schema resolution
2. The semantics are clearer — this is a database operator, not a general-purpose iterator
3. We may add methods later (`reset()`, `explain()`, `statistics()`)

```
Expected output after this exercise:
(no terminal output — this is a type/trait definition exercise)
```

<details>
<summary>Hint: If you are unsure where to put these types</summary>

The `Row` type goes in `src/executor.rs` because it is the executor's unit of output. `Value` stays in `src/types.rs` because it is shared across the whole system (storage, serialization, executor). The `Executor` trait goes in `src/executor.rs` alongside `Row` and `ExecutorError`. Later, individual executors (Scan, Filter, Project) can live in the same file or in submodules like `src/executor/scan.rs`.

</details>

---

## Exercise 2: Implement ScanExecutor

**Goal:** Build the first executor — a `ScanExecutor` that reads rows from an in-memory table and yields them one at a time.

### Step 1: Define a simple storage interface

The executor needs to read rows from somewhere. For now, we will use a simple in-memory storage that maps table names to their rows and column names:

```rust
// src/executor.rs (continued)

use std::collections::HashMap;

/// A simple in-memory storage for the executor to read from.
/// Maps table names to (column_names, rows).
pub struct Storage {
    tables: HashMap<String, TableData>,
}

pub struct TableData {
    pub column_names: Vec<String>,
    pub rows: Vec<Row>,
}

impl Storage {
    pub fn new() -> Self {
        Storage {
            tables: HashMap::new(),
        }
    }

    pub fn create_table(&mut self, name: &str, columns: Vec<String>) {
        self.tables.insert(name.to_string(), TableData {
            column_names: columns,
            rows: Vec::new(),
        });
    }

    pub fn insert_row(&mut self, table: &str, row: Row) -> Result<(), ExecutorError> {
        let table_data = self.tables.get_mut(table)
            .ok_or_else(|| ExecutorError::TableNotFound(table.to_string()))?;
        table_data.rows.push(row);
        Ok(())
    }

    pub fn get_table(&self, name: &str) -> Option<&TableData> {
        self.tables.get(name)
    }
}
```

This is intentionally simple. In a real database, the storage engine would read from disk, manage pages, handle concurrency. Our in-memory storage lets us focus on the executor logic without worrying about I/O.

### Step 2: Implement ScanExecutor

The `ScanExecutor` reads all rows from a table, yielding them one at a time:

```rust
// src/executor.rs (continued)

/// Scans all rows from a table, yielding them one at a time.
/// This is the leaf node in every executor tree — it is the only
/// executor that touches storage directly.
pub struct ScanExecutor {
    /// The rows to yield (cloned from storage at construction time).
    rows: Vec<Row>,
    /// Current position in the row vector.
    position: usize,
    /// Column names for this table.
    column_names: Vec<String>,
}

impl ScanExecutor {
    pub fn new(storage: &Storage, table: &str) -> Result<Self, ExecutorError> {
        let table_data = storage.get_table(table)
            .ok_or_else(|| ExecutorError::TableNotFound(table.to_string()))?;

        Ok(ScanExecutor {
            rows: table_data.rows.clone(),
            position: 0,
            column_names: table_data.column_names.clone(),
        })
    }
}

impl Executor for ScanExecutor {
    fn next(&mut self) -> Result<Option<Row>, ExecutorError> {
        if self.position >= self.rows.len() {
            return Ok(None);
        }
        let row = self.rows[self.position].clone();
        self.position += 1;
        Ok(Some(row))
    }

    fn columns(&self) -> &[String] {
        &self.column_names
    }
}
```

### Step 3: Understand the pull model

Notice what the `ScanExecutor` does NOT do:

- It does not return all rows at once (eager evaluation)
- It does not push rows to downstream operators
- It does not know who is calling it or what they will do with the rows

It simply waits to be asked. When `next()` is called, it returns the next row. When there are no more rows, it returns `None`. This is the pull model — the consumer controls the flow.

This matters when a `LIMIT 5` is at the top of the pipeline. The limit operator calls `next()` five times, gets five rows, and stops. The scan executor never reads rows 6 through 10 million. In an eager model, the scan would materialize the entire table before the limit could act.

### Step 4: Test it

```rust
// src/executor.rs — tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Value;

    fn sample_storage() -> Storage {
        let mut storage = Storage::new();
        storage.create_table("users", vec![
            "id".to_string(),
            "name".to_string(),
            "age".to_string(),
        ]);
        storage.insert_row("users", Row::new(vec![
            Value::Integer(1), Value::String("Alice".to_string()), Value::Integer(30),
        ])).unwrap();
        storage.insert_row("users", Row::new(vec![
            Value::Integer(2), Value::String("Bob".to_string()), Value::Integer(25),
        ])).unwrap();
        storage.insert_row("users", Row::new(vec![
            Value::Integer(3), Value::String("Carol".to_string()), Value::Integer(35),
        ])).unwrap();
        storage.insert_row("users", Row::new(vec![
            Value::Integer(4), Value::String("Dave".to_string()), Value::Integer(28),
        ])).unwrap();
        storage
    }

    #[test]
    fn test_scan_executor() {
        let storage = sample_storage();
        let mut scan = ScanExecutor::new(&storage, "users").unwrap();

        // Pull all four rows
        let row1 = scan.next().unwrap().unwrap();
        assert_eq!(row1.values[1], Value::String("Alice".to_string()));

        let row2 = scan.next().unwrap().unwrap();
        assert_eq!(row2.values[1], Value::String("Bob".to_string()));

        let row3 = scan.next().unwrap().unwrap();
        assert_eq!(row3.values[1], Value::String("Carol".to_string()));

        let row4 = scan.next().unwrap().unwrap();
        assert_eq!(row4.values[1], Value::String("Dave".to_string()));

        // Exhausted
        assert_eq!(scan.next().unwrap(), None);
        assert_eq!(scan.next().unwrap(), None); // safe to call again
    }

    #[test]
    fn test_scan_missing_table() {
        let storage = Storage::new();
        let result = ScanExecutor::new(&storage, "nonexistent");
        assert_eq!(result.unwrap_err(), ExecutorError::TableNotFound("nonexistent".to_string()));
    }
}
```

```
Expected output:
$ cargo test test_scan
running 2 tests
test executor::tests::test_scan_executor ... ok
test executor::tests::test_scan_missing_table ... ok
test result: ok. 2 passed; 0 failed
```

<details>
<summary>Hint: If the test fails with "cannot find type Value"</summary>

Make sure `src/types.rs` is declared as a module in `src/lib.rs` (or `src/main.rs` if you are using a binary crate). The executor module imports from `crate::types::Value`, so the types module must be visible. If you have been putting everything in a single file, now is the time to split into modules — refer to the project structure from earlier chapters.

</details>

---

## Exercise 3: Implement FilterExecutor

**Goal:** Build a `FilterExecutor` that wraps another executor, evaluates a predicate expression against each row, and passes through only matching rows.

### Step 1: Build the expression evaluator

Before we can filter rows, we need to evaluate expressions. Given a row and a list of column names, evaluate an expression like `age > 30`:

```rust
// src/executor.rs (continued)

/// Evaluate an expression against a row.
///
/// The `columns` slice maps column indices to names, so that
/// `ColumnRef("age")` can find the right value in the row.
pub fn evaluate(
    expr: &Expression,
    row: &Row,
    columns: &[String],
) -> Result<Value, ExecutorError> {
    match expr {
        Expression::Literal(v) => Ok(v.clone()),

        Expression::ColumnRef(name) => {
            let index = columns.iter()
                .position(|c| c == name)
                .ok_or_else(|| ExecutorError::ColumnNotFound(name.clone()))?;
            Ok(row.values[index].clone())
        }

        Expression::BinaryOp { left, op, right } => {
            let left_val = evaluate(left, row, columns)?;
            let right_val = evaluate(right, row, columns)?;
            eval_binary_op(&left_val, op, &right_val)
        }

        Expression::UnaryOp { op, operand } => {
            let val = evaluate(operand, row, columns)?;
            eval_unary_op(op, &val)
        }
    }
}

fn eval_binary_op(
    left: &Value,
    op: &BinaryOperator,
    right: &Value,
) -> Result<Value, ExecutorError> {
    match op {
        // Arithmetic operations
        BinaryOperator::Add => match (left, right) {
            (Value::Integer(a), Value::Integer(b)) => Ok(Value::Integer(a + b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a + b)),
            (Value::Integer(a), Value::Float(b)) => Ok(Value::Float(*a as f64 + b)),
            (Value::Float(a), Value::Integer(b)) => Ok(Value::Float(a + *b as f64)),
            _ => Err(ExecutorError::TypeError(
                format!("cannot add {:?} and {:?}", left, right)
            )),
        },

        BinaryOperator::Subtract => match (left, right) {
            (Value::Integer(a), Value::Integer(b)) => Ok(Value::Integer(a - b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a - b)),
            (Value::Integer(a), Value::Float(b)) => Ok(Value::Float(*a as f64 - b)),
            (Value::Float(a), Value::Integer(b)) => Ok(Value::Float(a - *b as f64)),
            _ => Err(ExecutorError::TypeError(
                format!("cannot subtract {:?} from {:?}", right, left)
            )),
        },

        BinaryOperator::Multiply => match (left, right) {
            (Value::Integer(a), Value::Integer(b)) => Ok(Value::Integer(a * b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a * b)),
            (Value::Integer(a), Value::Float(b)) => Ok(Value::Float(*a as f64 * b)),
            (Value::Float(a), Value::Integer(b)) => Ok(Value::Float(a * *b as f64)),
            _ => Err(ExecutorError::TypeError(
                format!("cannot multiply {:?} and {:?}", left, right)
            )),
        },

        BinaryOperator::Divide => match (left, right) {
            (_, Value::Integer(0)) => Err(ExecutorError::DivisionByZero),
            (_, Value::Float(f)) if *f == 0.0 => Err(ExecutorError::DivisionByZero),
            (Value::Integer(a), Value::Integer(b)) => Ok(Value::Integer(a / b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a / b)),
            (Value::Integer(a), Value::Float(b)) => Ok(Value::Float(*a as f64 / b)),
            (Value::Float(a), Value::Integer(b)) => Ok(Value::Float(a / *b as f64)),
            _ => Err(ExecutorError::TypeError(
                format!("cannot divide {:?} by {:?}", left, right)
            )),
        },

        // Comparison operations
        BinaryOperator::Equal => Ok(Value::Boolean(left == right)),
        BinaryOperator::NotEqual => Ok(Value::Boolean(left != right)),

        BinaryOperator::LessThan => eval_comparison(left, right, |a, b| a < b),
        BinaryOperator::GreaterThan => eval_comparison(left, right, |a, b| a > b),
        BinaryOperator::LessOrEqual => eval_comparison(left, right, |a, b| a <= b),
        BinaryOperator::GreaterOrEqual => eval_comparison(left, right, |a, b| a >= b),

        // Logical operations
        BinaryOperator::And => match (left, right) {
            (Value::Boolean(a), Value::Boolean(b)) => Ok(Value::Boolean(*a && *b)),
            _ => Err(ExecutorError::TypeError(
                format!("AND requires booleans, got {:?} and {:?}", left, right)
            )),
        },

        BinaryOperator::Or => match (left, right) {
            (Value::Boolean(a), Value::Boolean(b)) => Ok(Value::Boolean(*a || *b)),
            _ => Err(ExecutorError::TypeError(
                format!("OR requires booleans, got {:?} and {:?}", left, right)
            )),
        },
    }
}

/// Helper for comparison operations — handles numeric type coercion.
fn eval_comparison<F>(left: &Value, right: &Value, cmp: F) -> Result<Value, ExecutorError>
where
    F: Fn(f64, f64) -> bool,
{
    match (left, right) {
        (Value::Integer(a), Value::Integer(b)) => {
            Ok(Value::Boolean(cmp(*a as f64, *b as f64)))
        }
        (Value::Float(a), Value::Float(b)) => {
            Ok(Value::Boolean(cmp(*a, *b)))
        }
        (Value::Integer(a), Value::Float(b)) => {
            Ok(Value::Boolean(cmp(*a as f64, *b)))
        }
        (Value::Float(a), Value::Integer(b)) => {
            Ok(Value::Boolean(cmp(*a, *b as f64)))
        }
        (Value::String(a), Value::String(b)) => {
            // Lexicographic comparison for strings
            let a_f = a.len() as f64;
            let b_f = b.len() as f64;
            // Actually compare strings properly
            Ok(Value::Boolean(match (a.as_str(), b.as_str()) {
                (a, b) => cmp(0.0, if a < b { 1.0 } else if a > b { -1.0 } else { 0.0 }.neg()),
            }))
        }
        _ => Err(ExecutorError::TypeError(
            format!("cannot compare {:?} and {:?}", left, right)
        )),
    }
}
```

Wait — that string comparison is getting convoluted. Let us simplify. Comparisons should use `PartialOrd` naturally. Here is a cleaner approach:

```rust
/// Helper for comparison operations — handles numeric type coercion
/// and string lexicographic comparison.
fn eval_comparison<F>(left: &Value, right: &Value, int_cmp: F) -> Result<Value, ExecutorError>
where
    F: Fn(i128, i128) -> bool + Copy,
{
    match (left, right) {
        (Value::Integer(a), Value::Integer(b)) => {
            Ok(Value::Boolean(int_cmp(*a as i128, *b as i128)))
        }
        (Value::Float(a), Value::Float(b)) => {
            // Use total_cmp for consistent ordering
            Ok(Value::Boolean(int_cmp(
                (*a * 1_000_000.0) as i128,
                (*b * 1_000_000.0) as i128,
            )))
        }
        (Value::Integer(a), Value::Float(b)) => {
            Ok(Value::Boolean(int_cmp(
                (*a as f64 * 1_000_000.0) as i128,
                (*b * 1_000_000.0) as i128,
            )))
        }
        (Value::Float(a), Value::Integer(b)) => {
            Ok(Value::Boolean(int_cmp(
                (*a * 1_000_000.0) as i128,
                (*b as f64 * 1_000_000.0) as i128,
            )))
        }
        (Value::String(a), Value::String(b)) => {
            // Lexicographic comparison
            let ord = a.cmp(b);
            Ok(Value::Boolean(int_cmp(
                ord as i128,
                0,
            )))
        }
        _ => Err(ExecutorError::TypeError(
            format!("cannot compare {:?} and {:?}", left, right)
        )),
    }
}
```

Actually, let us make this much simpler. The cleanest approach is to convert values to a comparable form:

```rust
/// Compare two values using the given comparison function.
/// Handles numeric coercion (Integer + Float) and string comparison.
fn eval_comparison(
    left: &Value,
    right: &Value,
    op: &BinaryOperator,
) -> Result<Value, ExecutorError> {
    let ordering = match (left, right) {
        (Value::Integer(a), Value::Integer(b)) => a.cmp(b),
        (Value::Float(a), Value::Float(b)) => a.partial_cmp(b)
            .unwrap_or(std::cmp::Ordering::Equal),
        (Value::Integer(a), Value::Float(b)) => (*a as f64).partial_cmp(b)
            .unwrap_or(std::cmp::Ordering::Equal),
        (Value::Float(a), Value::Integer(b)) => a.partial_cmp(&(*b as f64))
            .unwrap_or(std::cmp::Ordering::Equal),
        (Value::String(a), Value::String(b)) => a.cmp(b),
        _ => return Err(ExecutorError::TypeError(
            format!("cannot compare {:?} and {:?}", left, right)
        )),
    };

    let result = match op {
        BinaryOperator::LessThan => ordering == std::cmp::Ordering::Less,
        BinaryOperator::GreaterThan => ordering == std::cmp::Ordering::Greater,
        BinaryOperator::LessOrEqual => ordering != std::cmp::Ordering::Greater,
        BinaryOperator::GreaterOrEqual => ordering != std::cmp::Ordering::Less,
        _ => unreachable!("eval_comparison called with non-comparison operator"),
    };

    Ok(Value::Boolean(result))
}
```

Now update `eval_binary_op` to use this:

```rust
BinaryOperator::LessThan
| BinaryOperator::GreaterThan
| BinaryOperator::LessOrEqual
| BinaryOperator::GreaterOrEqual => eval_comparison(left, right, op),
```

And for the unary operations:

```rust
fn eval_unary_op(op: &UnaryOperator, val: &Value) -> Result<Value, ExecutorError> {
    match op {
        UnaryOperator::Not => match val {
            Value::Boolean(b) => Ok(Value::Boolean(!b)),
            _ => Err(ExecutorError::TypeError(
                format!("NOT requires a boolean, got {:?}", val)
            )),
        },
        UnaryOperator::Negate => match val {
            Value::Integer(i) => Ok(Value::Integer(-i)),
            Value::Float(f) => Ok(Value::Float(-f)),
            _ => Err(ExecutorError::TypeError(
                format!("cannot negate {:?}", val)
            )),
        },
    }
}
```

### Step 2: Build the FilterExecutor

Now we can evaluate predicates. The `FilterExecutor` wraps a child executor and only passes through rows where the predicate evaluates to `true`:

```rust
// src/executor.rs (continued)

/// Filters rows from a child executor based on a predicate expression.
///
/// For each row pulled from the child, the predicate is evaluated.
/// If it returns Boolean(true), the row is yielded.
/// If it returns Boolean(false) or Null, the row is skipped.
/// Any other result is a type error.
pub struct FilterExecutor {
    /// The child executor providing input rows.
    source: Box<dyn Executor>,
    /// The predicate to evaluate against each row.
    predicate: Expression,
}

impl FilterExecutor {
    pub fn new(source: Box<dyn Executor>, predicate: Expression) -> Self {
        FilterExecutor { source, predicate }
    }
}

impl Executor for FilterExecutor {
    fn next(&mut self) -> Result<Option<Row>, ExecutorError> {
        // Pull rows from the child until we find one that passes the predicate,
        // or until the child is exhausted.
        loop {
            match self.source.next()? {
                None => return Ok(None), // child exhausted
                Some(row) => {
                    let result = evaluate(
                        &self.predicate,
                        &row,
                        self.source.columns(),
                    )?;

                    match result {
                        Value::Boolean(true) => return Ok(Some(row)),
                        Value::Boolean(false) | Value::Null => continue, // skip
                        other => return Err(ExecutorError::TypeError(
                            format!(
                                "WHERE predicate must evaluate to a boolean, got {:?}",
                                other,
                            )
                        )),
                    }
                }
            }
        }
    }

    fn columns(&self) -> &[String] {
        // A filter does not change the schema — same columns as the child
        self.source.columns()
    }
}
```

### Step 3: Understand the loop

The `loop` in `next()` is critical. A filter cannot simply call `self.source.next()` once and return the result — the row might not pass the predicate. It must keep pulling rows until it finds one that matches or the source is exhausted.

This is the same pattern as Rust's standard `Iterator::filter()`:

```rust
// What Iterator::filter does internally (simplified)
fn next(&mut self) -> Option<Self::Item> {
    loop {
        match self.iter.next() {
            None => return None,
            Some(item) => {
                if (self.predicate)(&item) {
                    return Some(item);
                }
                // else: continue the loop, pull the next item
            }
        }
    }
}
```

Our `FilterExecutor.next()` is essentially this pattern with `Result` error handling added.

### Step 4: Test the filter

```rust
#[cfg(test)]
mod tests {
    // ... (previous test code) ...

    #[test]
    fn test_filter_executor() {
        let storage = sample_storage();
        let scan = ScanExecutor::new(&storage, "users").unwrap();

        // Filter: age > 28
        let predicate = Expression::BinaryOp {
            left: Box::new(Expression::ColumnRef("age".to_string())),
            op: BinaryOperator::GreaterThan,
            right: Box::new(Expression::Literal(Value::Integer(28))),
        };

        let mut filter = FilterExecutor::new(Box::new(scan), predicate);

        // Alice (30) passes, Bob (25) fails, Carol (35) passes, Dave (28) fails
        let row1 = filter.next().unwrap().unwrap();
        assert_eq!(row1.values[1], Value::String("Alice".to_string()));
        assert_eq!(row1.values[2], Value::Integer(30));

        let row2 = filter.next().unwrap().unwrap();
        assert_eq!(row2.values[1], Value::String("Carol".to_string()));
        assert_eq!(row2.values[2], Value::Integer(35));

        // Exhausted — Bob and Dave were filtered out
        assert_eq!(filter.next().unwrap(), None);
    }

    #[test]
    fn test_filter_no_matches() {
        let storage = sample_storage();
        let scan = ScanExecutor::new(&storage, "users").unwrap();

        // Filter: age > 100 — nobody qualifies
        let predicate = Expression::BinaryOp {
            left: Box::new(Expression::ColumnRef("age".to_string())),
            op: BinaryOperator::GreaterThan,
            right: Box::new(Expression::Literal(Value::Integer(100))),
        };

        let mut filter = FilterExecutor::new(Box::new(scan), predicate);
        assert_eq!(filter.next().unwrap(), None);
    }

    #[test]
    fn test_evaluate_column_ref() {
        let row = Row::new(vec![
            Value::Integer(1),
            Value::String("Alice".to_string()),
            Value::Integer(30),
        ]);
        let columns = vec!["id".to_string(), "name".to_string(), "age".to_string()];

        let result = evaluate(
            &Expression::ColumnRef("name".to_string()),
            &row,
            &columns,
        ).unwrap();

        assert_eq!(result, Value::String("Alice".to_string()));
    }

    #[test]
    fn test_evaluate_missing_column() {
        let row = Row::new(vec![Value::Integer(1)]);
        let columns = vec!["id".to_string()];

        let result = evaluate(
            &Expression::ColumnRef("nonexistent".to_string()),
            &row,
            &columns,
        );

        assert_eq!(
            result.unwrap_err(),
            ExecutorError::ColumnNotFound("nonexistent".to_string()),
        );
    }
}
```

```
Expected output:
$ cargo test test_filter
running 2 tests
test executor::tests::test_filter_executor ... ok
test executor::tests::test_filter_no_matches ... ok
test result: ok. 2 passed; 0 failed

$ cargo test test_evaluate
running 2 tests
test executor::tests::test_evaluate_column_ref ... ok
test executor::tests::test_evaluate_missing_column ... ok
test result: ok. 2 passed; 0 failed
```

<details>
<summary>Hint: If the filter passes the wrong rows</summary>

Check your `eval_comparison` function. The most common bug is getting the ordering backwards — `GreaterThan` should return `true` when `left > right`, not `right > left`. Test with simple cases first: `evaluate(5 > 3)` should return `Boolean(true)`, `evaluate(3 > 5)` should return `Boolean(false)`.

Also check that `ColumnRef("age")` resolves to the correct index. If your columns are `["id", "name", "age"]`, then "age" is at index 2. A mismatch between column order in the schema and value order in the row will cause incorrect comparisons.

</details>

---

## Exercise 4: Implement ProjectExecutor and Wire the Full Pipeline

**Goal:** Build a `ProjectExecutor` for column selection, then wire up the complete pipeline from SQL string to result rows.

### Step 1: Implement ProjectExecutor

The `ProjectExecutor` takes a list of column expressions and evaluates each one against every row, producing a new row with only the selected columns:

```rust
// src/executor.rs (continued)

/// Projects (selects) specific columns from each row.
///
/// Given column expressions like [ColumnRef("name"), ColumnRef("age")],
/// this executor transforms each input row into a new row containing
/// only the values of those columns.
pub struct ProjectExecutor {
    /// The child executor providing input rows.
    source: Box<dyn Executor>,
    /// The expressions to evaluate for each output column.
    expressions: Vec<Expression>,
    /// The output column names.
    output_columns: Vec<String>,
}

impl ProjectExecutor {
    pub fn new(
        source: Box<dyn Executor>,
        expressions: Vec<Expression>,
    ) -> Self {
        // Derive output column names from the expressions
        let output_columns: Vec<String> = expressions.iter()
            .enumerate()
            .map(|(i, expr)| match expr {
                Expression::ColumnRef(name) => name.clone(),
                _ => format!("column_{}", i),
            })
            .collect();

        ProjectExecutor {
            source,
            expressions,
            output_columns,
        }
    }
}

impl Executor for ProjectExecutor {
    fn next(&mut self) -> Result<Option<Row>, ExecutorError> {
        match self.source.next()? {
            None => Ok(None),
            Some(row) => {
                let source_columns = self.source.columns();
                let projected_values: Result<Vec<Value>, ExecutorError> = self
                    .expressions
                    .iter()
                    .map(|expr| evaluate(expr, &row, source_columns))
                    .collect();

                Ok(Some(Row::new(projected_values?)))
            }
        }
    }

    fn columns(&self) -> &[String] {
        &self.output_columns
    }
}
```

### Step 2: Understand the projection

A `ProjectExecutor` is simpler than a filter — it does not skip rows, it transforms them. Every input row produces exactly one output row with (potentially) fewer or different columns.

The key insight is that projection expressions are not limited to simple column references. You could project computed expressions:

```sql
SELECT name, age * 2 AS double_age FROM users;
```

Our `evaluate` function handles this naturally — `Expression::BinaryOp { ColumnRef("age"), Multiply, Literal(2) }` evaluates to the doubled age for each row.

Notice how `columns()` returns the *output* column names, which may differ from the *input* column names. If the source has columns `["id", "name", "age"]` and the project selects `["name"]`, then `columns()` returns `["name"]`. This is important for nested pipelines — a filter above a projection must use the projection's output column names, not the original table's columns.

### Step 3: Build the pipeline builder

Now the exciting part. We convert an optimized `Plan` tree into a nested `Executor` tree:

```rust
// src/executor.rs (continued)

/// Converts a Plan tree into a nested Executor tree.
///
/// This is the bridge between the planner/optimizer and the executor.
/// Each Plan node becomes the corresponding Executor:
///   Plan::Scan      -> ScanExecutor
///   Plan::Filter    -> FilterExecutor(build(source))
///   Plan::Project   -> ProjectExecutor(build(source))
///   Plan::EmptyResult -> EmptyExecutor
pub fn build_executor(
    plan: Plan,
    storage: &Storage,
) -> Result<Box<dyn Executor>, ExecutorError> {
    match plan {
        Plan::Scan { table } => {
            let scan = ScanExecutor::new(storage, &table)?;
            Ok(Box::new(scan))
        }

        Plan::Filter { predicate, source } => {
            let child = build_executor(*source, storage)?;
            Ok(Box::new(FilterExecutor::new(child, predicate)))
        }

        Plan::Project { columns, source } => {
            let child = build_executor(*source, storage)?;
            Ok(Box::new(ProjectExecutor::new(child, columns)))
        }

        Plan::EmptyResult => {
            Ok(Box::new(EmptyExecutor))
        }
    }
}

/// An executor that produces no rows. Used for queries the optimizer
/// determined will match nothing (e.g., WHERE FALSE).
struct EmptyExecutor;

impl Executor for EmptyExecutor {
    fn next(&mut self) -> Result<Option<Row>, ExecutorError> {
        Ok(None)
    }

    fn columns(&self) -> &[String] {
        &[]
    }
}
```

### Step 4: A convenience function for collecting all results

```rust
// src/executor.rs (continued)

/// A complete result set — column names plus all rows.
#[derive(Debug)]
pub struct ResultSet {
    pub columns: Vec<String>,
    pub rows: Vec<Row>,
}

impl ResultSet {
    /// Execute the given executor to completion, collecting all rows.
    pub fn collect_from(mut executor: Box<dyn Executor>) -> Result<Self, ExecutorError> {
        let columns = executor.columns().to_vec();
        let mut rows = Vec::new();

        while let Some(row) = executor.next()? {
            rows.push(row);
        }

        Ok(ResultSet { columns, rows })
    }

    /// Pretty-print the result set as a table.
    pub fn display(&self) -> String {
        if self.columns.is_empty() && self.rows.is_empty() {
            return "(empty result)".to_string();
        }

        // Calculate column widths
        let mut widths: Vec<usize> = self.columns.iter()
            .map(|c| c.len())
            .collect();

        for row in &self.rows {
            for (i, val) in row.values.iter().enumerate() {
                if i < widths.len() {
                    widths[i] = widths[i].max(format!("{}", val).len());
                }
            }
        }

        let mut output = String::new();

        // Header
        let header: Vec<String> = self.columns.iter()
            .enumerate()
            .map(|(i, c)| format!("{:width$}", c, width = widths[i]))
            .collect();
        output.push_str(&header.join(" | "));
        output.push('\n');

        // Separator
        let sep: Vec<String> = widths.iter()
            .map(|w| "-".repeat(*w))
            .collect();
        output.push_str(&sep.join("-+-"));
        output.push('\n');

        // Rows
        for row in &self.rows {
            let cells: Vec<String> = row.values.iter()
                .enumerate()
                .map(|(i, v)| {
                    let width = if i < widths.len() { widths[i] } else { 0 };
                    format!("{:width$}", format!("{}", v), width = width)
                })
                .collect();
            output.push_str(&cells.join(" | "));
            output.push('\n');
        }

        // Row count
        output.push_str(&format!("({} rows)\n", self.rows.len()));

        output
    }
}
```

### Step 5: Wire it all together

Now let us run a complete query:

```rust
// src/executor.rs — complete pipeline test

#[cfg(test)]
mod tests {
    // ... (previous tests) ...

    #[test]
    fn test_full_pipeline() {
        let storage = sample_storage();

        // SELECT name FROM users WHERE age > 28
        //
        // Plan tree (from planner/optimizer):
        //   Project(columns: [name])
        //     Filter(predicate: age > 28)
        //       Scan(table: users)
        let plan = Plan::Project {
            columns: vec![Expression::ColumnRef("name".to_string())],
            source: Box::new(Plan::Filter {
                predicate: Expression::BinaryOp {
                    left: Box::new(Expression::ColumnRef("age".to_string())),
                    op: BinaryOperator::GreaterThan,
                    right: Box::new(Expression::Literal(Value::Integer(28))),
                },
                source: Box::new(Plan::Scan {
                    table: "users".to_string(),
                }),
            }),
        };

        let executor = build_executor(plan, &storage).unwrap();
        let result = ResultSet::collect_from(executor).unwrap();

        assert_eq!(result.columns, vec!["name".to_string()]);
        assert_eq!(result.rows.len(), 2);
        assert_eq!(result.rows[0].values[0], Value::String("Alice".to_string()));
        assert_eq!(result.rows[1].values[0], Value::String("Carol".to_string()));

        // Pretty-print
        println!("{}", result.display());
    }

    #[test]
    fn test_empty_result_plan() {
        let storage = sample_storage();

        // The optimizer might produce EmptyResult for WHERE FALSE
        let plan = Plan::EmptyResult;
        let executor = build_executor(plan, &storage).unwrap();
        let result = ResultSet::collect_from(executor).unwrap();

        assert_eq!(result.rows.len(), 0);
    }

    #[test]
    fn test_scan_all_columns() {
        let storage = sample_storage();

        // SELECT * FROM users (just a Scan, no Project)
        let plan = Plan::Scan { table: "users".to_string() };
        let executor = build_executor(plan, &storage).unwrap();
        let result = ResultSet::collect_from(executor).unwrap();

        assert_eq!(result.columns, vec![
            "id".to_string(), "name".to_string(), "age".to_string(),
        ]);
        assert_eq!(result.rows.len(), 4);
        println!("{}", result.display());
    }

    #[test]
    fn test_project_computed_expression() {
        let storage = sample_storage();

        // SELECT name, age + 10 FROM users
        let plan = Plan::Project {
            columns: vec![
                Expression::ColumnRef("name".to_string()),
                Expression::BinaryOp {
                    left: Box::new(Expression::ColumnRef("age".to_string())),
                    op: BinaryOperator::Add,
                    right: Box::new(Expression::Literal(Value::Integer(10))),
                },
            ],
            source: Box::new(Plan::Scan {
                table: "users".to_string(),
            }),
        };

        let executor = build_executor(plan, &storage).unwrap();
        let result = ResultSet::collect_from(executor).unwrap();

        assert_eq!(result.rows[0].values, vec![
            Value::String("Alice".to_string()),
            Value::Integer(40), // 30 + 10
        ]);
        assert_eq!(result.rows[1].values, vec![
            Value::String("Bob".to_string()),
            Value::Integer(35), // 25 + 10
        ]);
    }
}
```

```
Expected output:
$ cargo test test_full_pipeline -- --nocapture
running 1 test
name
-----
Alice
Carol
(2 rows)

test executor::tests::test_full_pipeline ... ok

$ cargo test test_scan_all -- --nocapture
running 1 test
id | name  | age
---+-------+----
1  | Alice | 30
2  | Bob   | 25
3  | Carol | 35
4  | Dave  | 28
(4 rows)

test executor::tests::test_scan_all_columns ... ok
```

### Step 6: Connect to the full compilation pipeline

If you built the lexer, parser, planner, and optimizer in Chapters 6-9, you can now wire the full pipeline:

```rust
// src/lib.rs or src/main.rs

use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::planner::{Planner, Schema};
use crate::optimizer::Optimizer;
use crate::executor::{build_executor, ResultSet, Storage};

/// Execute a SQL query end-to-end: SQL string → result set.
pub fn execute_query(
    sql: &str,
    schema: &Schema,
    storage: &Storage,
) -> Result<ResultSet, String> {
    // 1. Lex
    let tokens = Lexer::new(sql).lex()
        .map_err(|e| format!("Lex error: {}", e))?;

    // 2. Parse
    let statement = Parser::new(tokens).parse()
        .map_err(|e| format!("Parse error: {}", e))?;

    // 3. Plan
    let plan = Planner::new(schema).plan(statement)
        .map_err(|e| format!("Plan error: {}", e))?;

    // 4. Optimize
    let optimizer = Optimizer::default_optimizer();
    let optimized = optimizer.optimize(plan).plan;

    // 5. Execute
    let executor = build_executor(optimized, storage)
        .map_err(|e| format!("Execution error: {}", e))?;

    ResultSet::collect_from(executor)
        .map_err(|e| format!("Execution error: {}", e))
}
```

Five stages, one line each. SQL string goes in, `ResultSet` comes out. Each stage has a single responsibility: the lexer does not know about plans, the optimizer does not know about storage, the executor does not know about SQL syntax.

<details>
<summary>Hint: If ProjectExecutor returns wrong column names</summary>

The `columns()` method on `ProjectExecutor` must return the *output* column names, not the input column names. If you are projecting `[ColumnRef("name")]`, then `columns()` should return `["name"]`, not `["id", "name", "age"]`. The `output_columns` field computed in `ProjectExecutor::new` handles this — but make sure you are reading from `self.output_columns`, not from `self.source.columns()`.

A subtle bug: if you call `self.source.columns()` inside `next()` after the source has been consumed, it should still return the correct schema. Our implementation stores column names separately from the data, so this works. But be aware that some executor designs might not guarantee this.

</details>

---

## Rust Gym

### Drill 1: Iterator Adaptors

Without running the code, predict the output. Then verify.

```rust
fn main() {
    let data = vec![
        ("Alice", 30),
        ("Bob", 25),
        ("Carol", 35),
        ("Dave", 28),
        ("Eve", 22),
    ];

    let result: Vec<&str> = data.iter()
        .filter(|(_, age)| *age >= 28)
        .map(|(name, _)| *name)
        .take(2)
        .collect();

    println!("{:?}", result);
}
```

<details>
<summary>Solution</summary>

```
["Alice", "Carol"]
```

The pipeline processes elements one at a time:
1. `("Alice", 30)` — passes filter (30 >= 28), mapped to "Alice", `take` accepts (1 of 2)
2. `("Bob", 25)` — fails filter (25 < 28), skipped
3. `("Carol", 35)` — passes filter (35 >= 28), mapped to "Carol", `take` accepts (2 of 2)
4. `take` is satisfied — stops pulling. Dave and Eve are never examined.

This is identical to `SELECT name FROM users WHERE age >= 28 LIMIT 2`. The Volcano model works exactly the same way — the `LIMIT` operator at the top stops pulling after 2 rows.

</details>

### Drill 2: Custom Iterator

Implement a `FibonacciIterator` that yields Fibonacci numbers. It should be an infinite iterator (never returns `None`).

```rust
struct FibonacciIterator {
    // what fields do you need?
}

impl FibonacciIterator {
    fn new() -> Self {
        todo!()
    }
}

impl Iterator for FibonacciIterator {
    type Item = u64;

    fn next(&mut self) -> Option<u64> {
        todo!()
    }
}

fn main() {
    let fibs: Vec<u64> = FibonacciIterator::new().take(10).collect();
    println!("{:?}", fibs);
    // Expected: [0, 1, 1, 2, 3, 5, 8, 13, 21, 34]
}
```

<details>
<summary>Solution</summary>

```rust
struct FibonacciIterator {
    a: u64,
    b: u64,
}

impl FibonacciIterator {
    fn new() -> Self {
        FibonacciIterator { a: 0, b: 1 }
    }
}

impl Iterator for FibonacciIterator {
    type Item = u64;

    fn next(&mut self) -> Option<u64> {
        let value = self.a;
        let next = self.a + self.b;
        self.a = self.b;
        self.b = next;
        Some(value) // always returns Some — infinite iterator
    }
}

fn main() {
    let fibs: Vec<u64> = FibonacciIterator::new().take(10).collect();
    println!("{:?}", fibs);
    // [0, 1, 1, 2, 3, 5, 8, 13, 21, 34]

    // Because it's infinite, you MUST use .take() or .find() or similar
    // .collect() on an infinite iterator would loop forever
}
```

Key insight: the iterator is infinite — `next()` always returns `Some`. This is safe because Rust's lazy evaluation means nothing runs until pulled. You control termination with `.take(10)`, `.find(|&n| n > 100)`, or similar adaptors. Our `ScanExecutor` is a finite iterator (returns `None` when the table is exhausted), but the concept is the same.

</details>

### Drill 3: Composing Executors

Without running the code, trace through the executor pipeline and determine which rows are produced:

```rust
// Table: products
// | id | name      | price | category    |
// |----|-----------|-------|-------------|
// | 1  | Widget    | 25    | electronics |
// | 2  | Gadget    | 75    | electronics |
// | 3  | Doohickey | 10    | accessories |
// | 4  | Thingamob | 50    | electronics |
// | 5  | Gizmo     | 30    | accessories |

// Plan: SELECT name, price FROM products WHERE category = 'electronics' AND price > 30

// What rows does the executor produce?
```

<details>
<summary>Solution</summary>

The pipeline is:

```
Project(name, price)
  Filter(category = 'electronics' AND price > 30)
    Scan(products)
```

Processing each row through Filter:
1. Widget: category='electronics' AND 25 > 30 → false AND false → **skip**
2. Gadget: category='electronics' AND 75 > 30 → true AND true → **pass**
3. Doohickey: category='accessories' AND 10 > 30 → false AND false → **skip**
4. Thingamob: category='electronics' AND 50 > 30 → true AND true → **pass**
5. Gizmo: category='accessories' AND 30 > 30 → false AND false → **skip**

Then Project selects only name and price:

```
name      | price
----------+------
Gadget    | 75
Thingamob | 50
(2 rows)
```

The execution order in the Volcano model:
1. Project calls Filter.next()
2. Filter calls Scan.next() → Widget → evaluates predicate → false → calls Scan.next() again
3. Scan returns Gadget → predicate → true → Filter returns Gadget to Project
4. Project evaluates [ColumnRef("name"), ColumnRef("price")] → ("Gadget", 75)

The scan executor never "knows" that only 2 rows will make it through. It dutifully returns all 5 rows, one at a time, when asked.

</details>

---

## DSA in Context: The Volcano Model

The Volcano model (also called the iterator model or pull-based execution) was formalized by Goetz Graefe in his 1994 paper "Volcano — An Extensible and Parallel Query Evaluation System." Nearly every relational database uses some variant of this model. Understanding it gives you vocabulary for database internals interviews and system design discussions.

### Pull-based vs push-based execution

In the **pull model** (Volcano), the top operator drives execution. It calls `next()` on its child, which calls `next()` on its child, and so on down to the scan:

```
          Project.next()
              |
              v
          Filter.next()
              |
              v
          Scan.next()
              |
              v
          [Storage]
```

Control flows top-down. Data flows bottom-up. This is the model we built.

In the **push model**, the scan drives execution. It reads rows and pushes them to the next operator, which pushes results to the next, and so on up to the root:

```
          [Storage]
              |
              v
          Scan.produce()
              |
              v
          Filter.consume() -> Filter.produce()
              |
              v
          Project.consume() -> Project.produce()
```

The push model can be more efficient for complex pipelines because it avoids the overhead of function calls up and down the tree on every row. Modern databases like HyPer and Peloton use push-based or hybrid models.

### Why Volcano won (for teaching)

The pull model is simpler to implement and reason about:

1. **Each operator is self-contained.** A `FilterExecutor` knows nothing about what is above or below it. It just pulls from its child and yields rows.

2. **Composition is natural.** Building `Project(Filter(Scan))` is just nesting constructors. Adding a new operator type does not affect existing operators.

3. **Lazy evaluation is automatic.** If the top operator stops pulling (LIMIT, early termination), the pipeline stops. No cancellation protocol needed.

4. **Error handling is straightforward.** Errors propagate up through `Result` values. No callbacks, no error channels, no exception handling across thread boundaries.

The tradeoff is performance: each `next()` call is a virtual function call (because our executors are behind `Box<dyn Executor>`), and virtual calls are harder for the CPU to predict than direct calls. For tables with millions of rows, this per-row overhead adds up. That is why production databases use techniques like vectorized execution (processing batches of rows instead of one at a time) to amortize the call overhead.

### Complexity analysis

For a query `SELECT columns FROM table WHERE predicate`:

| Operation | Time complexity |
|-----------|----------------|
| Full table scan | O(n) where n = rows in table |
| Filter (no index) | O(n) — must examine every row |
| Project | O(k) per row where k = number of output columns |
| Overall | O(n * k) — linear in table size |

The Volcano model does not change the asymptotic complexity — it changes the constant factor and the memory usage. An eager evaluation stores all intermediate results in memory. The Volcano model uses O(1) memory per operator (just the current row), regardless of table size. For a 10-million-row table, the eager approach might need gigabytes of intermediate memory; the Volcano approach needs only a few kilobytes.

---

## System Design Corner: Vectorized vs Tuple-at-a-Time Execution

The Volcano model we built is **tuple-at-a-time** — each `next()` call produces exactly one row. Modern analytical databases use **vectorized execution**, where each `next()` call produces a *batch* of rows (typically 1,000-4,000 at a time).

### Why vectorize?

The overhead of the tuple-at-a-time model is not the algorithm — it is the function calls. For a query scanning 10 million rows through 5 operators, that is 50 million virtual function calls. Each call involves:

1. Indirect jump (vtable lookup for `dyn Executor`)
2. Function prologue/epilogue
3. Branch prediction miss (the CPU cannot predict which executor will be called)
4. Pipeline stall (modern CPUs execute instructions out of order, but indirect jumps break the pipeline)

Vectorized execution amortizes this overhead:

```rust
// Tuple-at-a-time: 10 million function calls
for _ in 0..10_000_000 {
    let row = executor.next()?;
}

// Vectorized: 10,000 function calls (1,000 rows per batch)
for _ in 0..10_000 {
    let batch: Vec<Row> = executor.next_batch(1000)?;
    // Process 1000 rows in a tight loop (no virtual dispatch)
}
```

Within each batch, the executor processes rows in a tight `for` loop — no virtual dispatch, no indirect jumps. The CPU can predict the loop, prefetch data, and use SIMD instructions (processing 4 or 8 values simultaneously).

### Column stores vs row stores

Our storage is a **row store** — each row contains all columns: `[1, "Alice", 30]`. Column stores flip this: each column is stored separately.

```
Row store:
  row 0: [1, "Alice", 30]
  row 1: [2, "Bob",   25]
  row 2: [3, "Carol", 35]

Column store:
  id:   [1, 2, 3]
  name: ["Alice", "Bob", "Carol"]
  age:  [30, 25, 35]
```

Column stores are better for analytical queries that touch few columns across many rows (`SELECT AVG(age) FROM users` reads only the `age` column). Row stores are better for transactional queries that touch all columns of few rows (`SELECT * FROM users WHERE id = 3`).

### Production database architectures

| Database | Execution model | Storage model | Notes |
|----------|----------------|---------------|-------|
| PostgreSQL | Tuple-at-a-time (Volcano) | Row store | Most traditional RDBMS |
| MySQL | Tuple-at-a-time | Row store (InnoDB) | |
| SQLite | Bytecode interpreter | Row store (B-tree pages) | Not Volcano — uses a VM |
| DuckDB | Vectorized | Column store | Designed for analytics |
| ClickHouse | Vectorized | Column store | High-performance analytics |
| CockroachDB | Vectorized | Row store (LSM) | Distributed SQL |
| TiDB | Chunk-based (vectorized) | Row store (TiKV) | Distributed SQL |

> **Interview talking point:** *"Our query executor uses the Volcano model — each operator implements a `next()` method that yields one row at a time. Operators compose into a pipeline: Project(Filter(Scan)). This gives us lazy evaluation (LIMIT stops the pipeline early), O(1) memory per operator, and clean error propagation through Result. For production workloads, I would consider vectorized execution to amortize the per-row virtual dispatch overhead — processing batches of 1,000 rows at a time reduces function call overhead by 1,000x and enables SIMD optimization."*

---

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
