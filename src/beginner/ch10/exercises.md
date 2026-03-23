## Exercise 1: Define the Row Type and Executor Trait

**Goal:** Create the foundational types: a `Row` type, an error type, and the `Executor` trait that every operator will implement.

### Step 1: Create the executor module

Create `src/executor.rs` and register it:

```rust
// src/lib.rs -- add this line
pub mod executor;
```

### Step 2: Define the Row type

A row is simply a list of values -- one value per column. The column order matches the schema.

```rust
// src/executor.rs

use crate::planner::{Expression, BinaryOperator, UnaryOperator, Plan};
use std::collections::HashMap;
use std::fmt;

/// A value in our database. Every cell in every row is one of these.
/// You should already have this in your codebase from earlier chapters.
/// If not, define it here.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Boolean(bool),
    Integer(i64),
    Float(f64),
    String(String),
}

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

/// A row of values, one per column.
///
/// Think of a row as one line in a spreadsheet.
/// If the table has columns [id, name, age], then
/// a row might be [Integer(1), String("Alice"), Integer(30)].
/// The order must match the column names.
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
        let parts: Vec<String> = self.values.iter()
            .map(|v| format!("{}", v))
            .collect();
        write!(f, "({})", parts.join(", "))
    }
}
```

The `Display` trait lets us print rows in a readable format. The `map` and `collect` pattern converts each value to a string and then joins them with commas.

### Step 3: Define the error type

Things can go wrong during execution: a column might not exist, types might not match, division by zero, etc. We define an enum for all the error cases:

```rust
// src/executor.rs (continued)

/// Errors that can happen during query execution.
#[derive(Debug, Clone, PartialEq)]
pub enum ExecutorError {
    /// A column name in the query does not exist in the current schema.
    ColumnNotFound(String),
    /// Type mismatch: tried to add a string to an integer, etc.
    TypeError(String),
    /// The FROM table does not exist.
    TableNotFound(String),
    /// Tried to divide by zero.
    DivisionByZero,
    /// Catch-all for other errors.
    Internal(String),
}

impl fmt::Display for ExecutorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExecutorError::ColumnNotFound(col) => {
                write!(f, "column not found: {}", col)
            }
            ExecutorError::TypeError(msg) => {
                write!(f, "type error: {}", msg)
            }
            ExecutorError::TableNotFound(table) => {
                write!(f, "table not found: {}", table)
            }
            ExecutorError::DivisionByZero => {
                write!(f, "division by zero")
            }
            ExecutorError::Internal(msg) => {
                write!(f, "internal error: {}", msg)
            }
        }
    }
}
```

### Step 4: Define the Executor trait

This is the core abstraction. Every query operator -- scan, filter, project, join, sort -- will implement this trait:

```rust
// src/executor.rs (continued)

/// The core executor interface -- the Volcano model.
///
/// Every operator implements this trait. Calling `next()` produces
/// the next row in the result, or `None` when done.
///
/// Operators compose: a FilterExecutor wraps a ScanExecutor,
/// a ProjectExecutor wraps a FilterExecutor, and so on.
pub trait Executor {
    /// Return the next row, or None if no more rows.
    ///
    /// The three possible returns:
    ///   Ok(Some(row))  -- here is the next row
    ///   Ok(None)       -- no more rows; done
    ///   Err(e)         -- something went wrong
    fn next(&mut self) -> Result<Option<Row>, ExecutorError>;

    /// Return the column names for the rows this executor produces.
    ///
    /// This is needed so that expressions like ColumnRef("age")
    /// can find the right value in each row.
    fn columns(&self) -> &[String];
}
```

Why `Result<Option<Row>, ExecutorError>` instead of just `Option<Row>`?

We need three states:
- `Ok(Some(row))` -- here is a row
- `Ok(None)` -- all done, no more rows
- `Err(error)` -- something broke

Without `Result`, we would have to encode errors as special rows or use panics. Without `Option`, we would need a separate `has_next()` method, which leads to subtle bugs.

Why `columns()`? When we evaluate `ColumnRef("age")`, we need to know which position in the row vector corresponds to "age." If the columns are `["id", "name", "age"]`, then "age" is at index 2. The `columns()` method provides this mapping.

> **What just happened?**
>
> We defined the contract that every executor must follow: produce rows one at a time via `next()`, and tell us the column names via `columns()`. This is the Volcano model in code. The beauty is that any executor can wrap any other executor -- as long as they both implement this trait, they can be composed.

> **Common Mistakes**
>
> 1. **Calling `next()` after it returns `None`**: This should not crash. A well-implemented executor returns `None` forever after it is exhausted. Our tests will verify this.
>
> 2. **Mismatch between `columns()` and row length**: If `columns()` returns 3 names but `next()` returns rows with 4 values, expressions will index out of bounds. Keep them consistent.

---

## Exercise 2: Implement ScanExecutor

**Goal:** Build the first executor -- a `ScanExecutor` that reads rows from an in-memory table and yields them one at a time.

### Step 1: Define a simple storage interface

The executor needs data to read. For now, we use a simple in-memory storage: a HashMap mapping table names to their rows and column names.

```rust
// src/executor.rs (continued)

/// In-memory storage: maps table names to their data.
///
/// In a real database, this would read from disk. We use an
/// in-memory HashMap to keep things simple and focus on the
/// executor logic.
pub struct Storage {
    tables: HashMap<String, TableData>,
}

/// Data for a single table: column names and rows.
pub struct TableData {
    pub column_names: Vec<String>,
    pub rows: Vec<Row>,
}

impl Storage {
    /// Create an empty storage with no tables.
    pub fn new() -> Self {
        Storage {
            tables: HashMap::new(),
        }
    }

    /// Create a table with the given column names (no rows yet).
    pub fn create_table(&mut self, name: &str, columns: Vec<String>) {
        self.tables.insert(name.to_string(), TableData {
            column_names: columns,
            rows: Vec::new(),
        });
    }

    /// Insert a row into an existing table.
    pub fn insert_row(&mut self, table: &str, row: Row) -> Result<(), ExecutorError> {
        let table_data = self.tables.get_mut(table)
            .ok_or_else(|| ExecutorError::TableNotFound(table.to_string()))?;
        table_data.rows.push(row);
        Ok(())
    }

    /// Look up a table by name.
    pub fn get_table(&self, name: &str) -> Option<&TableData> {
        self.tables.get(name)
    }
}
```

Two new patterns here:

1. **`get_mut`**: Like `get`, but returns a mutable reference. We need this because `insert_row` modifies the table's row list.

2. **`ok_or_else`**: Converts an `Option<T>` to a `Result<T, E>`. If the option is `Some`, it becomes `Ok`. If it is `None`, it calls the closure to create an error and returns `Err`. This is a concise way to handle "table not found."

### Step 2: Build the ScanExecutor

The `ScanExecutor` reads all rows from a table, one at a time:

```rust
// src/executor.rs (continued)

/// Scans all rows from a table, yielding them one at a time.
///
/// This is always a leaf node in the executor tree -- it is
/// the only executor that reads directly from storage. Every
/// other executor wraps a ScanExecutor (or wraps something
/// that wraps a ScanExecutor).
pub struct ScanExecutor {
    /// The rows to yield (copied from storage at construction time).
    rows: Vec<Row>,
    /// Where we are in the list. Advances with each next() call.
    position: usize,
    /// Column names for this table.
    column_names: Vec<String>,
}

impl ScanExecutor {
    /// Create a new ScanExecutor for the given table.
    ///
    /// Returns an error if the table does not exist.
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
```

Notice that we **clone** the rows from storage. This means the ScanExecutor has its own copy of the data. In a real database, you would read from disk page by page instead of copying everything into memory. But for learning, this is simpler.

Now implement the `Executor` trait:

```rust
impl Executor for ScanExecutor {
    fn next(&mut self) -> Result<Option<Row>, ExecutorError> {
        // Are we past the end?
        if self.position >= self.rows.len() {
            return Ok(None);  // No more rows
        }

        // Get the current row and advance the position
        let row = self.rows[self.position].clone();
        self.position += 1;
        Ok(Some(row))
    }

    fn columns(&self) -> &[String] {
        &self.column_names
    }
}
```

This is remarkably similar to our `StepCounter` iterator from the spotlight section. The pattern is the same:

1. Check if we are done (`position >= len`)
2. If not, get the current value and advance the position
3. Return the value wrapped in `Ok(Some(...))`

> **What just happened?**
>
> The `ScanExecutor` works like a cursor moving through a list. Each call to `next()` returns the row at the current position and moves the cursor forward. When the cursor reaches the end, it returns `None` forever. The executor does not know who is calling it or what they will do with the rows. It simply waits to be asked. This is the pull model -- the consumer controls the flow.

### Step 3: Test the ScanExecutor

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// Create a sample storage with a "users" table.
    fn sample_storage() -> Storage {
        let mut storage = Storage::new();
        storage.create_table("users", vec![
            "id".to_string(),
            "name".to_string(),
            "age".to_string(),
        ]);
        storage.insert_row("users", Row::new(vec![
            Value::Integer(1),
            Value::String("Alice".to_string()),
            Value::Integer(30),
        ])).unwrap();
        storage.insert_row("users", Row::new(vec![
            Value::Integer(2),
            Value::String("Bob".to_string()),
            Value::Integer(25),
        ])).unwrap();
        storage.insert_row("users", Row::new(vec![
            Value::Integer(3),
            Value::String("Carol".to_string()),
            Value::Integer(35),
        ])).unwrap();
        storage.insert_row("users", Row::new(vec![
            Value::Integer(4),
            Value::String("Dave".to_string()),
            Value::Integer(28),
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

        // Exhausted -- should return None
        assert_eq!(scan.next().unwrap(), None);
        // Safe to call again
        assert_eq!(scan.next().unwrap(), None);
    }

    #[test]
    fn test_scan_missing_table() {
        let storage = Storage::new();
        let result = ScanExecutor::new(&storage, "nonexistent");
        assert_eq!(
            result.unwrap_err(),
            ExecutorError::TableNotFound("nonexistent".to_string()),
        );
    }

    #[test]
    fn test_scan_columns() {
        let storage = sample_storage();
        let scan = ScanExecutor::new(&storage, "users").unwrap();
        assert_eq!(scan.columns(), &["id", "name", "age"]);
    }
}
```

```
$ cargo test test_scan
running 3 tests
test executor::tests::test_scan_executor ... ok
test executor::tests::test_scan_missing_table ... ok
test executor::tests::test_scan_columns ... ok

test result: ok. 3 passed; 0 failed
```

> **Common Mistakes**
>
> 1. **Off-by-one error**: Make sure you increment `position` AFTER reading the row, not before. If you increment first, you skip the first row.
>
> 2. **Returning `Err` when exhausted**: An exhausted executor is not an error. Return `Ok(None)`, not `Err(...)`. The difference matters to callers: `None` means "done successfully," `Err` means "something broke."

<details>
<summary>Hint: If the test fails with "cannot find type Value"</summary>

Make sure your `Value` type is accessible. If you defined it in `src/types.rs`, use `use crate::types::Value;` in your executor module. If you are putting everything in one file, make sure the type is defined above where you use it. Rust requires items to be visible before they can be used.

</details>

---

## Exercise 3: Implement the Expression Evaluator

**Goal:** Build a function that evaluates an expression against a row. This is the engine that powers filtering -- given a row and an expression like `age > 30`, it computes the result.

### Step 1: The evaluate function

This function takes an expression, a row, and the column names, and produces a value:

```rust
// src/executor.rs (continued)

/// Evaluate an expression against a row.
///
/// This is the heart of the executor. Every filter condition,
/// every projection expression, every join predicate goes
/// through this function.
///
/// The `columns` slice maps positions to names, so that
/// `ColumnRef("age")` can find "age" is at index 2.
pub fn evaluate(
    expr: &Expression,
    row: &Row,
    columns: &[String],
) -> Result<Value, ExecutorError> {
    match expr {
        // Literal values are already evaluated
        Expression::Literal(v) => Ok(v.clone()),

        // Column references: look up the column position, then get the value
        Expression::ColumnRef(name) => {
            // Find which index this column name corresponds to
            let index = columns.iter()
                .position(|c| c == name)
                .ok_or_else(|| ExecutorError::ColumnNotFound(name.clone()))?;
            Ok(row.values[index].clone())
        }

        // Binary operations: evaluate both sides, then compute
        Expression::BinaryOp { left, op, right } => {
            let left_val = evaluate(left, row, columns)?;
            let right_val = evaluate(right, row, columns)?;
            eval_binary_op(&left_val, op, &right_val)
        }

        // Unary operations: evaluate the operand, then compute
        Expression::UnaryOp { op, operand } => {
            let val = evaluate(operand, row, columns)?;
            eval_unary_op(op, &val)
        }
    }
}
```

Let us trace through evaluating `age > 30` against a row `[1, "Alice", 30]` with columns `["id", "name", "age"]`:

1. Match `BinaryOp { left: ColumnRef("age"), op: GreaterThan, right: Literal(30) }`
2. Evaluate left: `ColumnRef("age")`. Find "age" in columns -- it is at index 2. Get `row.values[2]` = `Integer(30)`.
3. Evaluate right: `Literal(Integer(30))`. Return `Integer(30)`.
4. Call `eval_binary_op(Integer(30), GreaterThan, Integer(30))` -- returns `Boolean(false)`.

The `?` operator is used three times here. Each time, if the result is `Err`, the function returns immediately with that error. If it is `Ok`, the value inside is extracted and used. This is how errors propagate in Rust without exceptions.

### Step 2: Binary operation evaluation

```rust
/// Evaluate a binary operation on two values.
fn eval_binary_op(
    left: &Value,
    op: &BinaryOperator,
    right: &Value,
) -> Result<Value, ExecutorError> {
    match op {
        // --- Arithmetic ---
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

        // --- Comparisons ---
        BinaryOperator::Equal => Ok(Value::Boolean(left == right)),
        BinaryOperator::NotEqual => Ok(Value::Boolean(left != right)),

        BinaryOperator::LessThan => compare_values(left, right, |a, b| a < b),
        BinaryOperator::GreaterThan => compare_values(left, right, |a, b| a > b),
        BinaryOperator::LessOrEqual => compare_values(left, right, |a, b| a <= b),
        BinaryOperator::GreaterOrEqual => compare_values(left, right, |a, b| a >= b),

        // --- Logical ---
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
```

Notice the division case: we check for zero FIRST, before the type-specific arms. The pattern `(_, Value::Integer(0))` matches any left value with a right value of zero. The `_` is a wildcard -- it matches anything.

The `if *f == 0.0` is a **guard clause** on a match arm. It adds an extra condition beyond the pattern.

### Step 3: Comparison helper

```rust
/// Compare two values using a comparison function.
///
/// Handles numeric type coercion: Integer and Float can be
/// compared by converting Integer to Float.
fn compare_values<F>(
    left: &Value,
    right: &Value,
    cmp: F,
) -> Result<Value, ExecutorError>
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
            // String comparison uses lexicographic ordering
            let a_val = a.len() as f64;
            let b_val = b.len() as f64;
            Ok(Value::Boolean(cmp(a_val, b_val)))
        }
        _ => Err(ExecutorError::TypeError(
            format!("cannot compare {:?} and {:?}", left, right)
        )),
    }
}
```

This function takes a comparison function as a parameter. The `where F: Fn(f64, f64) -> bool` says "F is any function or closure that takes two `f64` values and returns a `bool`." This lets us reuse the same code for `<`, `>`, `<=`, and `>=`.

The `*a as f64` syntax dereferences `a` (getting the `i64` value) and converts it to `f64`. This is called a **type cast** and uses the `as` keyword.

### Step 4: Unary operation evaluation

```rust
/// Evaluate a unary operation on a value.
fn eval_unary_op(
    op: &UnaryOperator,
    val: &Value,
) -> Result<Value, ExecutorError> {
    match (op, val) {
        (UnaryOperator::Not, Value::Boolean(b)) => Ok(Value::Boolean(!b)),
        (UnaryOperator::Negate, Value::Integer(n)) => Ok(Value::Integer(-n)),
        (UnaryOperator::Negate, Value::Float(f)) => Ok(Value::Float(-f)),
        _ => Err(ExecutorError::TypeError(
            format!("cannot apply {:?} to {:?}", op, val)
        )),
    }
}
```

### Step 5: Test the evaluator

```rust
#[cfg(test)]
mod evaluator_tests {
    use super::*;
    use crate::planner::Expression;

    fn test_row() -> (Row, Vec<String>) {
        let row = Row::new(vec![
            Value::Integer(1),
            Value::String("Alice".to_string()),
            Value::Integer(30),
        ]);
        let columns = vec![
            "id".to_string(),
            "name".to_string(),
            "age".to_string(),
        ];
        (row, columns)
    }

    #[test]
    fn eval_literal() {
        let (row, columns) = test_row();
        let expr = Expression::Literal(Value::Integer(42));
        let result = evaluate(&expr, &row, &columns).unwrap();
        assert_eq!(result, Value::Integer(42));
    }

    #[test]
    fn eval_column_ref() {
        let (row, columns) = test_row();
        let expr = Expression::ColumnRef("age".to_string());
        let result = evaluate(&expr, &row, &columns).unwrap();
        assert_eq!(result, Value::Integer(30));
    }

    #[test]
    fn eval_column_not_found() {
        let (row, columns) = test_row();
        let expr = Expression::ColumnRef("email".to_string());
        let result = evaluate(&expr, &row, &columns);
        assert_eq!(
            result.unwrap_err(),
            ExecutorError::ColumnNotFound("email".to_string()),
        );
    }

    #[test]
    fn eval_comparison() {
        let (row, columns) = test_row();

        // age > 25 (age is 30, so this is true)
        let expr = Expression::BinaryOp {
            left: Box::new(Expression::ColumnRef("age".to_string())),
            op: BinaryOperator::GreaterThan,
            right: Box::new(Expression::Literal(Value::Integer(25))),
        };
        let result = evaluate(&expr, &row, &columns).unwrap();
        assert_eq!(result, Value::Boolean(true));

        // age > 35 (age is 30, so this is false)
        let expr2 = Expression::BinaryOp {
            left: Box::new(Expression::ColumnRef("age".to_string())),
            op: BinaryOperator::GreaterThan,
            right: Box::new(Expression::Literal(Value::Integer(35))),
        };
        let result2 = evaluate(&expr2, &row, &columns).unwrap();
        assert_eq!(result2, Value::Boolean(false));
    }

    #[test]
    fn eval_division_by_zero() {
        let (row, columns) = test_row();
        let expr = Expression::BinaryOp {
            left: Box::new(Expression::Literal(Value::Integer(10))),
            op: BinaryOperator::Divide,
            right: Box::new(Expression::Literal(Value::Integer(0))),
        };
        let result = evaluate(&expr, &row, &columns);
        assert_eq!(result.unwrap_err(), ExecutorError::DivisionByZero);
    }
}
```

```
$ cargo test evaluator_tests
running 5 tests
test executor::evaluator_tests::eval_literal ... ok
test executor::evaluator_tests::eval_column_ref ... ok
test executor::evaluator_tests::eval_column_not_found ... ok
test executor::evaluator_tests::eval_comparison ... ok
test executor::evaluator_tests::eval_division_by_zero ... ok

test result: ok. 5 passed; 0 failed
```

---

## Exercise 4: Implement FilterExecutor

**Goal:** Build a `FilterExecutor` that wraps another executor, evaluates a predicate expression against each row, and only passes through rows that match.

### Step 1: Define the FilterExecutor struct

```rust
// src/executor.rs (continued)

/// Wraps another executor and filters its output.
///
/// For each row from the child executor, evaluates the predicate.
/// If the predicate returns true, the row passes through.
/// If false or NULL, the row is skipped.
///
/// Think of a FilterExecutor like a bouncer at a club. The bouncer
/// checks each person (row) against the criteria (predicate) and
/// only lets matching ones through.
pub struct FilterExecutor {
    /// The executor we are filtering.
    source: Box<dyn Executor>,
    /// The condition to check each row against.
    predicate: Expression,
}
```

Notice `Box<dyn Executor>` -- we are using the trait object pattern from Chapter 9. The `FilterExecutor` does not know what concrete type its source is. It could be a `ScanExecutor`, another `FilterExecutor`, or anything that implements `Executor`. This is what makes composition possible.

### Step 2: Implement the constructor

```rust
impl FilterExecutor {
    pub fn new(source: Box<dyn Executor>, predicate: Expression) -> Self {
        FilterExecutor { source, predicate }
    }
}
```

### Step 3: Implement the Executor trait

```rust
impl Executor for FilterExecutor {
    fn next(&mut self) -> Result<Option<Row>, ExecutorError> {
        // Keep pulling rows from the source until we find one
        // that matches the predicate, or the source is exhausted.
        loop {
            // Pull the next row from our child executor
            match self.source.next()? {
                // No more rows -- we are done too
                None => return Ok(None),

                // Got a row -- check the predicate
                Some(row) => {
                    // Evaluate the predicate against this row
                    let result = evaluate(
                        &self.predicate,
                        &row,
                        self.source.columns(),
                    )?;

                    match result {
                        // Predicate is true -- pass this row through
                        Value::Boolean(true) => return Ok(Some(row)),
                        // Predicate is false or NULL -- skip this row
                        Value::Boolean(false) | Value::Null => continue,
                        // Not a boolean -- that is a type error
                        other => return Err(ExecutorError::TypeError(
                            format!(
                                "filter predicate must be boolean, got {:?}",
                                other
                            )
                        )),
                    }
                }
            }
        }
    }

    fn columns(&self) -> &[String] {
        // A filter does not change the schema -- same columns as source
        self.source.columns()
    }
}
```

Let us trace through how this works:

1. The caller calls `filter.next()`
2. The filter calls `self.source.next()` to get a row from its child
3. If the child returns `None`, the filter also returns `None` (done)
4. If the child returns a row, the filter evaluates the predicate
5. If the predicate is `true`, return the row to the caller
6. If the predicate is `false`, go back to step 2 (loop) and get the next row
7. Repeat until a matching row is found or the source is exhausted

The `loop` is essential. A filter might need to skip many rows before finding a match. The loop keeps pulling from the source until either a match is found or there are no more rows.

> **What just happened?**
>
> The FilterExecutor wraps another executor and acts as a gatekeeper. It pulls rows from its source, checks each one against the predicate, and only passes through matches. Rows that do not match are silently discarded. The caller has no idea how many rows were skipped -- it just gets the next matching row. This is the Volcano model in action: each operator is a self-contained unit that pulls from its child.

### Step 4: Test the FilterExecutor

```rust
#[cfg(test)]
mod filter_tests {
    use super::*;

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

        // Alice (30) passes
        let row1 = filter.next().unwrap().unwrap();
        assert_eq!(row1.values[1], Value::String("Alice".to_string()));

        // Bob (25) and Dave (28) are skipped
        // Carol (35) passes
        let row2 = filter.next().unwrap().unwrap();
        assert_eq!(row2.values[1], Value::String("Carol".to_string()));

        // No more matching rows
        assert_eq!(filter.next().unwrap(), None);
    }

    #[test]
    fn test_filter_no_matches() {
        let storage = sample_storage();
        let scan = ScanExecutor::new(&storage, "users").unwrap();

        // Filter: age > 100 (nobody matches)
        let predicate = Expression::BinaryOp {
            left: Box::new(Expression::ColumnRef("age".to_string())),
            op: BinaryOperator::GreaterThan,
            right: Box::new(Expression::Literal(Value::Integer(100))),
        };

        let mut filter = FilterExecutor::new(Box::new(scan), predicate);

        // No rows match
        assert_eq!(filter.next().unwrap(), None);
    }
}
```

```
$ cargo test filter_tests
running 2 tests
test executor::filter_tests::test_filter_executor ... ok
test executor::filter_tests::test_filter_no_matches ... ok

test result: ok. 2 passed; 0 failed
```

> **Common Mistakes**
>
> 1. **Forgetting the loop**: Without the `loop`, the filter would only check one row and return `None` if it did not match. The loop is what makes it skip non-matching rows.
>
> 2. **Using `source.columns()` after moving source**: The `columns()` call inside `evaluate()` borrows `self.source`. Since we already have `&mut self` from `next()`, Rust checks that these borrows do not conflict. They are fine because `columns()` takes `&self` (shared borrow) while `next()` needs `&mut self` (exclusive borrow), but we call `columns()` only to read the column names -- we are not calling `next()` at the same time.

---

## Exercise 5: Implement ProjectExecutor

**Goal:** Build a `ProjectExecutor` that wraps another executor and keeps only specific columns from each row.

### Step 1: Define the ProjectExecutor struct

```rust
// src/executor.rs (continued)

/// Wraps another executor and selects specific columns.
///
/// Given a row with columns [id, name, age] and a projection
/// of [name, age], this executor produces rows with just [name, age].
///
/// Think of it like a camera frame: the full scene is there, but
/// you only capture what is inside the frame.
pub struct ProjectExecutor {
    /// The executor we are projecting.
    source: Box<dyn Executor>,
    /// The expressions to evaluate for each output column.
    /// Usually these are just ColumnRef("name"), but they could
    /// be computed expressions like BinaryOp(age, +, 1).
    expressions: Vec<Expression>,
    /// The output column names.
    output_columns: Vec<String>,
}
```

### Step 2: Implement the constructor and trait

```rust
impl ProjectExecutor {
    pub fn new(
        source: Box<dyn Executor>,
        expressions: Vec<Expression>,
    ) -> Self {
        // Derive output column names from the expressions.
        // For ColumnRef("name"), the output column is "name".
        // For other expressions, use a placeholder like "expr_0".
        let output_columns = expressions.iter().enumerate()
            .map(|(i, expr)| {
                match expr {
                    Expression::ColumnRef(name) => name.clone(),
                    _ => format!("expr_{}", i),
                }
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
        // Pull a row from the source
        match self.source.next()? {
            None => Ok(None),
            Some(row) => {
                // Evaluate each projection expression against the row
                let values: Result<Vec<Value>, ExecutorError> = self.expressions
                    .iter()
                    .map(|expr| evaluate(expr, &row, self.source.columns()))
                    .collect();

                Ok(Some(Row::new(values?)))
            }
        }
    }

    fn columns(&self) -> &[String] {
        &self.output_columns
    }
}
```

Let us unpack the `map` and `collect` pattern here. It is doing something clever:

1. `self.expressions.iter()` iterates over the projection expressions
2. `.map(|expr| evaluate(expr, &row, ...))` evaluates each expression, producing an iterator of `Result<Value, ExecutorError>`
3. `.collect::<Result<Vec<Value>, ExecutorError>>()` collects all results. Here is the magic: if ALL evaluations succeed, you get `Ok(vec![val1, val2, ...])`. If ANY evaluation fails, you immediately get `Err(first_error)`.

This `collect` on Results is a powerful pattern. It is equivalent to:

```rust,ignore
// What collect() does for us:
let mut values = Vec::new();
for expr in &self.expressions {
    match evaluate(expr, &row, self.source.columns()) {
        Ok(v) => values.push(v),
        Err(e) => return Err(e),  // stop at first error
    }
}
```

But the iterator version is more concise.

> **What just happened?**
>
> The ProjectExecutor pulls a row from its source, then evaluates each projection expression against that row to produce a narrower row. If the source row has columns [id, name, age] and the projection asks for [name, age], the output row will have just [name, age]. The key insight: projection does not skip rows (unlike filter). Every input row produces exactly one output row. It just changes which columns are included.

### Step 3: Test the ProjectExecutor

```rust
#[cfg(test)]
mod project_tests {
    use super::*;

    #[test]
    fn test_project_executor() {
        let storage = sample_storage();
        let scan = ScanExecutor::new(&storage, "users").unwrap();

        // Project: keep only name and age
        let expressions = vec![
            Expression::ColumnRef("name".to_string()),
            Expression::ColumnRef("age".to_string()),
        ];

        let mut project = ProjectExecutor::new(Box::new(scan), expressions);

        // Check that columns changed
        assert_eq!(project.columns(), &["name", "age"]);

        // First row: Alice, 30
        let row1 = project.next().unwrap().unwrap();
        assert_eq!(row1.values.len(), 2);
        assert_eq!(row1.values[0], Value::String("Alice".to_string()));
        assert_eq!(row1.values[1], Value::Integer(30));

        // Second row: Bob, 25
        let row2 = project.next().unwrap().unwrap();
        assert_eq!(row2.values[0], Value::String("Bob".to_string()));
        assert_eq!(row2.values[1], Value::Integer(25));
    }

    #[test]
    fn test_project_with_computed_column() {
        let storage = sample_storage();
        let scan = ScanExecutor::new(&storage, "users").unwrap();

        // Project: name, age + 1
        let expressions = vec![
            Expression::ColumnRef("name".to_string()),
            Expression::BinaryOp {
                left: Box::new(Expression::ColumnRef("age".to_string())),
                op: BinaryOperator::Add,
                right: Box::new(Expression::Literal(Value::Integer(1))),
            },
        ];

        let mut project = ProjectExecutor::new(Box::new(scan), expressions);

        let row1 = project.next().unwrap().unwrap();
        assert_eq!(row1.values[0], Value::String("Alice".to_string()));
        assert_eq!(row1.values[1], Value::Integer(31)); // 30 + 1
    }
}
```

```
$ cargo test project_tests
running 2 tests
test executor::project_tests::test_project_executor ... ok
test executor::project_tests::test_project_with_computed_column ... ok

test result: ok. 2 passed; 0 failed
```

---

## Exercise 6: Chaining Executors -- The Full Pipeline

**Goal:** Chain Scan, Filter, and Project together to execute a complete query.

### Step 1: Build the chain

The beauty of the Volcano model is composition. Each executor wraps another, forming a chain:

```
SELECT name FROM users WHERE age > 28

Plan tree:               Executor chain:
  Project [name]           ProjectExecutor
    Filter (age > 28)        FilterExecutor
      Scan users               ScanExecutor
```

```rust
#[cfg(test)]
mod pipeline_tests {
    use super::*;

    #[test]
    fn test_full_pipeline() {
        let storage = sample_storage();

        // Step 1: Scan the users table
        let scan = ScanExecutor::new(&storage, "users").unwrap();

        // Step 2: Filter to age > 28
        let predicate = Expression::BinaryOp {
            left: Box::new(Expression::ColumnRef("age".to_string())),
            op: BinaryOperator::GreaterThan,
            right: Box::new(Expression::Literal(Value::Integer(28))),
        };
        let filter = FilterExecutor::new(Box::new(scan), predicate);

        // Step 3: Project to just the name column
        let expressions = vec![
            Expression::ColumnRef("name".to_string()),
        ];
        let mut project = ProjectExecutor::new(Box::new(filter), expressions);

        // Pull results
        let row1 = project.next().unwrap().unwrap();
        assert_eq!(row1.values, vec![Value::String("Alice".to_string())]);

        let row2 = project.next().unwrap().unwrap();
        assert_eq!(row2.values, vec![Value::String("Carol".to_string())]);

        assert_eq!(project.next().unwrap(), None);
    }
}
```

Let us trace the flow for the first row:

```
1. project.next() calls filter.next()
2.   filter.next() calls scan.next()
3.     scan returns Row(1, "Alice", 30)
4.   filter evaluates "age > 28" → 30 > 28 → true
5.   filter returns Row(1, "Alice", 30)
6. project evaluates ColumnRef("name") → "Alice"
7. project returns Row("Alice")
```

For the second row:

```
1. project.next() calls filter.next()
2.   filter.next() calls scan.next()
3.     scan returns Row(2, "Bob", 25)
4.   filter evaluates "age > 28" → 25 > 28 → false → SKIP
5.   filter.next() calls scan.next() again (the loop continues)
6.     scan returns Row(3, "Carol", 35)
7.   filter evaluates "age > 28" → 35 > 28 → true
8.   filter returns Row(3, "Carol", 35)
9. project evaluates ColumnRef("name") → "Carol"
10. project returns Row("Carol")
```

Notice how Bob was skipped silently by the filter. The project executor never saw Bob's row. And Dave (age 28) will be skipped too because 28 is not greater than 28.

> **What just happened?**
>
> We chained three executors together: Scan feeds into Filter feeds into Project. Each one pulls from the one below, one row at a time. The beauty is that each executor is independent -- ScanExecutor does not know it is being filtered, FilterExecutor does not know its output is being projected. They just implement the `Executor` trait and let the composition handle the rest. This is the power of the Volcano model.

### Step 2: Build a plan-to-executor converter

To make things convenient, let us write a function that takes a `Plan` and creates the corresponding executor chain:

```rust
// src/executor.rs (continued)

/// Convert an optimized Plan tree into a nested executor tree.
///
/// This is the bridge between the planner/optimizer and the executor.
/// Given a Plan, it builds the corresponding chain of executors.
pub fn build_executor(
    plan: &Plan,
    storage: &Storage,
) -> Result<Box<dyn Executor>, ExecutorError> {
    match plan {
        Plan::Scan { table } => {
            let scan = ScanExecutor::new(storage, table)?;
            Ok(Box::new(scan))
        }

        Plan::Filter { predicate, source } => {
            let child = build_executor(source, storage)?;
            Ok(Box::new(FilterExecutor::new(child, predicate.clone())))
        }

        Plan::Project { columns, source } => {
            let child = build_executor(source, storage)?;
            Ok(Box::new(ProjectExecutor::new(child, columns.clone())))
        }

        Plan::EmptyResult => {
            // An empty result produces no rows.
            // We can use an empty scan as a stand-in.
            Ok(Box::new(EmptyExecutor))
        }
    }
}

/// An executor that produces no rows. Used for EmptyResult plans.
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

This function is recursive, just like the plan tree. A `Filter` plan node creates a `FilterExecutor` that wraps the executor for its source. A `Project` creates a `ProjectExecutor` wrapping its source. The recursion bottoms out at `Scan` (which reads from storage) and `EmptyResult` (which produces nothing).

### Step 3: Test the converter

```rust
#[cfg(test)]
mod build_tests {
    use super::*;

    #[test]
    fn test_build_executor_from_plan() {
        let storage = sample_storage();

        // Build a plan: Project([name], Filter(age > 28, Scan(users)))
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

        // Build the executor from the plan
        let mut executor = build_executor(&plan, &storage).unwrap();

        // Should get Alice and Carol
        let row1 = executor.next().unwrap().unwrap();
        assert_eq!(row1.values, vec![Value::String("Alice".to_string())]);

        let row2 = executor.next().unwrap().unwrap();
        assert_eq!(row2.values, vec![Value::String("Carol".to_string())]);

        assert_eq!(executor.next().unwrap(), None);
    }

    #[test]
    fn test_empty_result_produces_nothing() {
        let storage = sample_storage();
        let plan = Plan::EmptyResult;

        let mut executor = build_executor(&plan, &storage).unwrap();
        assert_eq!(executor.next().unwrap(), None);
    }
}
```

```
$ cargo test build_tests
running 2 tests
test executor::build_tests::test_build_executor_from_plan ... ok
test executor::build_tests::test_empty_result_produces_nothing ... ok

test result: ok. 2 passed; 0 failed
```

> **Common Mistakes**
>
> 1. **Cloning the plan fields**: `build_executor` takes `&Plan` (a reference), but `FilterExecutor::new` takes ownership of the `Expression`. We use `.clone()` to create owned copies. In a production database, you might use `Arc` or restructure to avoid cloning.
>
> 2. **Returning the wrong executor type**: `build_executor` returns `Box<dyn Executor>`. This means every executor is boxed, which adds a heap allocation. For a simple query with 3 operators, that is 3 heap allocations -- completely negligible.

---

## Exercise 7: Collecting All Results (Challenge)

**Goal:** Write a helper function that pulls all rows from an executor into a `Vec`. This is useful for testing and for queries that need all results at once (like sorting, which we will add in Chapter 11).

```rust
/// Pull all rows from an executor into a Vec.
///
/// This consumes the executor by calling next() until it returns None.
/// Be careful with large tables -- this loads everything into memory!
pub fn collect_all(
    executor: &mut dyn Executor,
) -> Result<Vec<Row>, ExecutorError> {
    let mut rows = Vec::new();
    loop {
        match executor.next()? {
            Some(row) => rows.push(row),
            None => return Ok(rows),
        }
    }
}
```

<details>
<summary>Hint: Testing collect_all</summary>

```rust
#[test]
fn test_collect_all() {
    let storage = sample_storage();
    let scan = ScanExecutor::new(&storage, "users").unwrap();

    let predicate = Expression::BinaryOp {
        left: Box::new(Expression::ColumnRef("age".to_string())),
        op: BinaryOperator::GreaterThan,
        right: Box::new(Expression::Literal(Value::Integer(28))),
    };
    let mut filter = FilterExecutor::new(Box::new(scan), predicate);

    let rows = collect_all(&mut filter).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].values[1], Value::String("Alice".to_string()));
    assert_eq!(rows[1].values[1], Value::String("Carol".to_string()));
}
```

</details>

---

## Key Takeaways

1. **The Volcano model uses pull-based iteration.** Each operator implements `next()` and pulls from its child. The top operator controls how much data flows. This enables `LIMIT` to stop early.

2. **Composition is the design pattern.** Executors wrap executors. `Project(Filter(Scan))` is three structs nested inside each other via `Box<dyn Executor>`. Each one is independent and reusable.

3. **Lazy evaluation saves work.** In a pipeline of Scan -> Filter -> Project, no row is projected until it passes the filter. If the filter rejects 90% of rows, the project does 90% less work.

4. **`Box<dyn Executor>` enables the nesting.** Without trait objects, you could not store a `ScanExecutor` and a `FilterExecutor` in the same field type. Dynamic dispatch is the small cost that enables this powerful composition.

5. **Expression evaluation is recursive.** Just like the optimizer walks the expression tree to fold constants, the executor walks the expression tree to compute values. The structure is the same; only the leaf behavior differs (the optimizer returns constants, the executor looks up column values from the current row).
