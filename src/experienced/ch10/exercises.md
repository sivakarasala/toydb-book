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
