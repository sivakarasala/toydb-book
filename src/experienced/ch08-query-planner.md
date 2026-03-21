# Chapter 8: Query Planner

You have a lexer that turns SQL strings into tokens and a parser that arranges those tokens into an AST. If you stopped here, you could walk the AST and immediately start reading and writing data. Many toy databases do exactly this. But every production database has a stage between parsing and execution: the query planner. The planner takes the AST — which describes WHAT the user asked for — and produces a plan — which describes HOW to get it. This separation is one of the most important architectural decisions in database design, and this chapter shows you why.

The planner resolves table names against a schema catalog, validates that columns actually exist, type-checks expressions, and assembles a tree of plan nodes. The result is a self-contained instruction set that an executor can process without ever looking at the original SQL again.

By the end of this chapter, you will have:

- A `Plan` enum representing execution plan nodes (Scan, Filter, Project, Insert, Update, Delete, CreateTable)
- A `Schema` catalog that knows which tables and columns exist
- A `Planner` that transforms a parsed `Statement` into a validated `Plan`
- Schema validation with descriptive error messages for missing tables and columns
- A `Display` implementation that prints plans as indented trees (like SQL's EXPLAIN)
- A deep understanding of Rust iterators, closures, and iterator adaptors

---

## Spotlight: Iterators & Closures

Every chapter has one spotlight concept. This chapter's spotlight is **iterators and closures** — the feature that makes Rust's data processing feel functional while remaining zero-cost.

### The Iterator trait

At its core, Rust's iterator system is a single trait with a single required method:

```rust
trait Iterator {
    type Item;
    fn next(&mut self) -> Option<Self::Item>;
}
```

That is the entire contract. Call `next()`, and you get `Some(value)` if there is more data, or `None` when the iterator is exhausted. Every `for` loop in Rust is syntactic sugar for calling `next()` until `None`:

```rust
let names = vec!["Alice", "Bob", "Carol"];

// This:
for name in &names {
    println!("{}", name);
}

// Is equivalent to:
let mut iter = names.iter();
while let Some(name) = iter.next() {
    println!("{}", name);
}
```

### Creating custom iterators

Any type can become iterable by implementing `Iterator`. Here is a counter that counts from a start value up to (but not including) an end value:

```rust
struct Counter {
    current: u32,
    end: u32,
}

impl Counter {
    fn new(start: u32, end: u32) -> Self {
        Counter { current: start, end }
    }
}

impl Iterator for Counter {
    type Item = u32;

    fn next(&mut self) -> Option<u32> {
        if self.current < self.end {
            let value = self.current;
            self.current += 1;
            Some(value)
        } else {
            None
        }
    }
}

fn main() {
    let counter = Counter::new(3, 7);
    let values: Vec<u32> = counter.collect();
    println!("{:?}", values); // [3, 4, 5, 6]
}
```

The key insight: the iterator holds mutable state (`current`) and produces values lazily. Nothing is computed until `next()` is called. This matters for the query planner — plan nodes will eventually be iterators that produce rows on demand.

### Closures

A closure is an anonymous function that captures variables from its surrounding scope:

```rust
let threshold = 18;
let is_adult = |age: i32| age >= threshold;   // captures `threshold`
println!("{}", is_adult(21));  // true
println!("{}", is_adult(15));  // false
```

Rust has three closure traits, based on how the closure uses captured variables:

```rust
// Fn — borrows captured variables immutably (can be called many times)
let name = String::from("Alice");
let greet = || println!("Hello, {}", name);     // borrows `name`
greet();
greet();   // OK — Fn can be called repeatedly
println!("{}", name);  // OK — name is still usable

// FnMut — borrows captured variables mutably (can be called many times)
let mut count = 0;
let mut increment = || { count += 1; count };   // mutably borrows `count`
println!("{}", increment());  // 1
println!("{}", increment());  // 2

// FnOnce — takes ownership of captured variables (can be called only once)
let name = String::from("Alice");
let consume = move || {
    let owned = name;  // moves `name` into the closure
    println!("Consumed: {}", owned);
};
consume();
// consume();  // ERROR — already consumed
// println!("{}", name);  // ERROR — name was moved
```

The `move` keyword forces a closure to take ownership of all captured variables, even if it only needs a reference. This is essential when sending closures to other threads or storing them in structs that outlive the current scope.

### Iterator adaptors

Iterator adaptors are methods on `Iterator` that transform one iterator into another. They are lazy — no work happens until you consume the result:

```rust
let numbers = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

// .filter() keeps elements matching a predicate
let evens: Vec<&i32> = numbers.iter()
    .filter(|n| **n % 2 == 0)
    .collect();
// [2, 4, 6, 8, 10]

// .map() transforms each element
let doubled: Vec<i32> = numbers.iter()
    .map(|n| n * 2)
    .collect();
// [2, 4, 6, 8, 10, 12, 14, 16, 18, 20]

// Chain them together
let result: Vec<i32> = numbers.iter()
    .filter(|n| **n % 2 == 0)    // keep evens
    .map(|n| n * n)               // square them
    .collect();
// [4, 16, 36, 64, 100]

// .flat_map() maps and flattens in one step
let words = vec!["hello world", "foo bar"];
let chars: Vec<char> = words.iter()
    .flat_map(|s| s.chars())
    .collect();
// ['h', 'e', 'l', 'l', 'o', ' ', 'w', 'o', 'r', 'l', 'd', ' ', 'f', 'o', 'o', ' ', 'b', 'a', 'r']

// .enumerate() pairs each element with its index
let indexed: Vec<(usize, &i32)> = numbers.iter()
    .enumerate()
    .collect();
// [(0, 1), (1, 2), (2, 3), ...]

// .zip() pairs elements from two iterators
let names = vec!["Alice", "Bob", "Carol"];
let ages = vec![30, 25, 28];
let pairs: Vec<(&str, &i32)> = names.iter()
    .copied()
    .zip(ages.iter())
    .collect();
// [("Alice", 30), ("Bob", 25), ("Carol", 28)]
```

### Collecting into different types

The `.collect()` method is generic — it can produce different collection types based on the type annotation:

```rust
let numbers = vec![1, 2, 3, 4, 5];

// Collect into a Vec
let v: Vec<i32> = numbers.iter().copied().collect();

// Collect into a HashSet
use std::collections::HashSet;
let s: HashSet<i32> = numbers.iter().copied().collect();

// Collect into a HashMap
use std::collections::HashMap;
let names = vec!["Alice", "Bob"];
let ages = vec![30, 25];
let map: HashMap<&str, i32> = names.into_iter()
    .zip(ages.into_iter())
    .collect();

// Collect Results — stops at the first error
let strings = vec!["1", "2", "oops", "4"];
let parsed: Result<Vec<i32>, _> = strings.iter()
    .map(|s| s.parse::<i32>())
    .collect();
// Err(ParseIntError)
```

That last example — collecting `Result`s — is particularly useful in the planner. When validating a list of columns, you want to stop at the first invalid column and return an error. `.collect::<Result<Vec<_>, _>>()` does exactly this.

> **Coming from other languages?**
>
> | Concept | JavaScript | Python | Go | Rust |
> |---------|-----------|--------|-----|------|
> | Transform elements | `arr.map(x => x * 2)` | `[x * 2 for x in arr]` | `for _, v := range arr { ... }` | `iter.map(\|x\| x * 2)` |
> | Filter elements | `arr.filter(x => x > 0)` | `[x for x in arr if x > 0]` | `for _, v := range arr { if v > 0 { ... } }` | `iter.filter(\|x\| **x > 0)` |
> | Chaining | `arr.filter(...).map(...)` | Nested comprehension or generator pipeline | Manual loop composition | `iter.filter(...).map(...)` |
> | Laziness | Eager (arrays) | Lazy (generators) | N/A (manual loops) | Lazy (adaptors) until `.collect()` |
> | Anonymous functions | `(x) => x + 1` | `lambda x: x + 1` | `func(x int) int { return x + 1 }` | `\|x\| x + 1` |
> | Capture semantics | Automatic (closure) | Automatic (closure) | Automatic (closure) | Explicit (`move` for ownership) |
>
> The biggest difference: Rust iterators are **lazy and zero-cost**. The chain `.filter().map().collect()` compiles to a single loop — the compiler fuses the operations. In JavaScript, `arr.filter().map()` creates two intermediate arrays. Rust creates zero.

---

## Exercise 1: Define Plan Nodes

**Goal:** Define the `Plan` enum — a tree of nodes describing how to execute a query.

### Step 1: The types we need

Before defining the plan, we need supporting types from earlier chapters. If you have been following along, these already exist. If not, here are the definitions the planner depends on:

```rust
// src/types.rs (from Chapters 2-4)

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Boolean(bool),
    Integer(i64),
    Float(f64),
    String(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum DataType {
    Boolean,
    Integer,
    Float,
    String,
}
```

And the AST types from Chapter 7:

```rust
// src/sql/ast.rs (from Chapter 7)

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Select {
        columns: Vec<SelectColumn>,
        from: String,
        where_clause: Option<Expression>,
    },
    Insert {
        table: String,
        columns: Vec<String>,
        values: Vec<Vec<Expression>>,
    },
    Update {
        table: String,
        assignments: Vec<(String, Expression)>,
        where_clause: Option<Expression>,
    },
    Delete {
        table: String,
        where_clause: Option<Expression>,
    },
    CreateTable {
        name: String,
        columns: Vec<ColumnDef>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum SelectColumn {
    AllColumns,              // *
    Named(String),           // column_name
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    Literal(Value),
    Column(String),
    BinaryOp {
        left: Box<Expression>,
        op: Operator,
        right: Box<Expression>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Operator {
    Eq,
    NotEq,
    Lt,
    Gt,
    LtEq,
    GtEq,
    And,
    Or,
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ColumnDef {
    pub name: String,
    pub data_type: DataType,
    pub nullable: bool,
    pub primary_key: bool,
}
```

### Step 2: Define the Plan enum

Create `src/sql/plan.rs`:

```rust
// src/sql/plan.rs

use crate::sql::ast::{ColumnDef, Expression, Operator};
use crate::types::{DataType, Value};

/// A plan node represents a single step in query execution.
/// Plans form a tree: a Project node contains a Filter node,
/// which contains a Scan node, and so on.
#[derive(Debug, Clone, PartialEq)]
pub enum Plan {
    /// Scan all rows from a table.
    Scan {
        table: String,
        columns: Vec<ColumnInfo>,
    },

    /// Filter rows from a source plan using a predicate.
    Filter {
        source: Box<Plan>,
        predicate: Expression,
    },

    /// Project (select) specific columns from a source plan.
    Project {
        source: Box<Plan>,
        columns: Vec<String>,
    },

    /// Insert rows into a table.
    Insert {
        table: String,
        columns: Vec<String>,
        rows: Vec<Vec<Value>>,
    },

    /// Update rows in a table.
    Update {
        table: String,
        source: Box<Plan>,
        assignments: Vec<(String, Expression)>,
    },

    /// Delete rows from a table.
    Delete {
        table: String,
        source: Box<Plan>,
    },

    /// Create a new table.
    CreateTable {
        name: String,
        columns: Vec<ColumnDef>,
    },
}

/// Metadata about a column, resolved from the schema catalog.
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: DataType,
    pub nullable: bool,
    pub primary_key: bool,
}
```

### Why `Box<Plan>`?

The `Plan` enum is recursive — a `Filter` contains another `Plan`, which might itself be a `Filter` containing another `Plan`. Without `Box`, Rust cannot compute the size of `Plan` at compile time (it would be infinite). `Box<Plan>` puts the inner plan on the heap, so the `Filter` variant only stores a pointer (8 bytes) instead of the entire subtree.

This is the same recursive tree pattern from Chapter 7's `Expression` type, now applied to plan nodes.

### The plan as a tree

A query like `SELECT name FROM users WHERE age > 18` becomes:

```
Project [name]
  Filter (age > 18)
    Scan users [id, name, age, email]
```

The tree reads bottom-up: first scan all rows from `users`, then keep only rows where `age > 18`, then extract just the `name` column. Each node transforms the stream of rows from the node below it.

This bottom-up reading is important: the executor will process the tree from the leaves (Scan) upward. The planner builds the tree top-down (from the outermost operation inward), but execution flows bottom-up.

### Step 3: Test the plan structure

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plan_construction() {
        // Build: Project [name] -> Filter (age > 18) -> Scan users
        let scan = Plan::Scan {
            table: "users".to_string(),
            columns: vec![
                ColumnInfo {
                    name: "id".to_string(),
                    data_type: DataType::Integer,
                    nullable: false,
                    primary_key: true,
                },
                ColumnInfo {
                    name: "name".to_string(),
                    data_type: DataType::String,
                    nullable: false,
                    primary_key: false,
                },
                ColumnInfo {
                    name: "age".to_string(),
                    data_type: DataType::Integer,
                    nullable: true,
                    primary_key: false,
                },
            ],
        };

        let filter = Plan::Filter {
            source: Box::new(scan),
            predicate: Expression::BinaryOp {
                left: Box::new(Expression::Column("age".to_string())),
                op: Operator::Gt,
                right: Box::new(Expression::Literal(Value::Integer(18))),
            },
        };

        let project = Plan::Project {
            source: Box::new(filter),
            columns: vec!["name".to_string()],
        };

        // Verify the structure
        match &project {
            Plan::Project { columns, source } => {
                assert_eq!(columns, &vec!["name".to_string()]);
                match source.as_ref() {
                    Plan::Filter { predicate, source } => {
                        match source.as_ref() {
                            Plan::Scan { table, .. } => {
                                assert_eq!(table, "users");
                            }
                            _ => panic!("expected Scan"),
                        }
                    }
                    _ => panic!("expected Filter"),
                }
            }
            _ => panic!("expected Project"),
        }
    }

    #[test]
    fn test_insert_plan() {
        let plan = Plan::Insert {
            table: "users".to_string(),
            columns: vec!["name".to_string(), "age".to_string()],
            rows: vec![
                vec![Value::String("Alice".to_string()), Value::Integer(30)],
                vec![Value::String("Bob".to_string()), Value::Integer(25)],
            ],
        };

        match &plan {
            Plan::Insert { table, columns, rows } => {
                assert_eq!(table, "users");
                assert_eq!(columns.len(), 2);
                assert_eq!(rows.len(), 2);
            }
            _ => panic!("expected Insert"),
        }
    }
}
```

Expected output:

```
running 2 tests
test sql::plan::tests::test_plan_construction ... ok
test sql::plan::tests::test_insert_plan ... ok
test result: ok. 2 passed; 0 failed
```

---

## Exercise 2: The Planner

**Goal:** Build a `Planner` struct that transforms a parsed `Statement` (AST) into a validated `Plan`.

### Step 1: The Schema catalog

Before the planner can validate queries, it needs to know what tables and columns exist. This is the schema catalog:

```rust
// src/sql/schema.rs

use crate::types::DataType;
use std::collections::HashMap;

/// Metadata about a single column in a table.
#[derive(Debug, Clone)]
pub struct Column {
    pub name: String,
    pub data_type: DataType,
    pub nullable: bool,
    pub primary_key: bool,
}

/// Metadata about a table.
#[derive(Debug, Clone)]
pub struct Table {
    pub name: String,
    pub columns: Vec<Column>,
}

impl Table {
    /// Look up a column by name.
    pub fn get_column(&self, name: &str) -> Option<&Column> {
        self.columns.iter().find(|c| c.name == name)
    }

    /// Check if a column exists.
    pub fn has_column(&self, name: &str) -> bool {
        self.columns.iter().any(|c| c.name == name)
    }

    /// Return all column names.
    pub fn column_names(&self) -> Vec<String> {
        self.columns.iter().map(|c| c.name.clone()).collect()
    }
}

/// The schema catalog — knows all tables and their columns.
#[derive(Debug, Clone)]
pub struct Schema {
    tables: HashMap<String, Table>,
}

impl Schema {
    pub fn new() -> Self {
        Schema {
            tables: HashMap::new(),
        }
    }

    /// Register a table in the catalog.
    pub fn add_table(&mut self, table: Table) {
        self.tables.insert(table.name.clone(), table);
    }

    /// Remove a table from the catalog.
    pub fn remove_table(&mut self, name: &str) -> bool {
        self.tables.remove(name).is_some()
    }

    /// Look up a table by name.
    pub fn get_table(&self, name: &str) -> Option<&Table> {
        self.tables.get(name)
    }

    /// Check if a table exists.
    pub fn has_table(&self, name: &str) -> bool {
        self.tables.contains_key(name)
    }

    /// List all table names (sorted for deterministic output).
    pub fn table_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.tables.keys().cloned().collect();
        names.sort();
        names
    }
}
```

Notice how `Table::get_column` and `Table::has_column` use iterator methods. `find()` returns the first element matching a predicate (as `Option<&T>`), and `any()` returns `true` if any element matches. These are more expressive than manual loops and communicate intent clearly.

### Step 2: The Planner

```rust
// src/sql/planner.rs

use crate::sql::ast::*;
use crate::sql::plan::*;
use crate::sql::schema::Schema;
use crate::types::Value;
use std::fmt;

/// Errors that can occur during planning.
#[derive(Debug, Clone, PartialEq)]
pub enum PlanError {
    TableNotFound(String),
    TableAlreadyExists(String),
    ColumnNotFound { column: String, table: String },
    InvalidColumnCount { expected: usize, got: usize },
    TypeError(String),
}

impl fmt::Display for PlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlanError::TableNotFound(name) => {
                write!(f, "table '{}' does not exist", name)
            }
            PlanError::TableAlreadyExists(name) => {
                write!(f, "table '{}' already exists", name)
            }
            PlanError::ColumnNotFound { column, table } => {
                write!(f, "column '{}' does not exist in table '{}'", column, table)
            }
            PlanError::InvalidColumnCount { expected, got } => {
                write!(
                    f,
                    "expected {} values per row, got {}",
                    expected, got
                )
            }
            PlanError::TypeError(msg) => write!(f, "type error: {}", msg),
        }
    }
}

/// The query planner transforms an AST Statement into a validated Plan.
pub struct Planner<'a> {
    schema: &'a Schema,
}

impl<'a> Planner<'a> {
    pub fn new(schema: &'a Schema) -> Self {
        Planner { schema }
    }

    /// Plan a statement — the main entry point.
    pub fn plan(&self, statement: Statement) -> Result<Plan, PlanError> {
        match statement {
            Statement::Select {
                columns,
                from,
                where_clause,
            } => self.plan_select(columns, from, where_clause),

            Statement::Insert {
                table,
                columns,
                values,
            } => self.plan_insert(table, columns, values),

            Statement::Update {
                table,
                assignments,
                where_clause,
            } => self.plan_update(table, assignments, where_clause),

            Statement::Delete {
                table,
                where_clause,
            } => self.plan_delete(table, where_clause),

            Statement::CreateTable { name, columns } => {
                self.plan_create_table(name, columns)
            }
        }
    }
}
```

The `Planner` borrows the `Schema` with a lifetime `'a`. This means the planner cannot outlive the schema — a compile-time guarantee that prevents dangling references. The planner is short-lived: you create it, plan one or more statements, and drop it.

### Step 3: Planning SELECT

```rust
impl<'a> Planner<'a> {
    fn plan_select(
        &self,
        columns: Vec<SelectColumn>,
        from: String,
        where_clause: Option<Expression>,
    ) -> Result<Plan, PlanError> {
        // Step 1: Resolve the table
        let table = self.schema.get_table(&from).ok_or_else(|| {
            PlanError::TableNotFound(from.clone())
        })?;

        // Step 2: Build the column info for the Scan node
        let column_infos: Vec<ColumnInfo> = table
            .columns
            .iter()
            .map(|c| ColumnInfo {
                name: c.name.clone(),
                data_type: c.data_type.clone(),
                nullable: c.nullable,
                primary_key: c.primary_key,
            })
            .collect();

        // Step 3: Start with a Scan node
        let mut plan = Plan::Scan {
            table: from.clone(),
            columns: column_infos,
        };

        // Step 4: If there is a WHERE clause, wrap in a Filter
        if let Some(predicate) = where_clause {
            self.validate_expression(&predicate, table)?;
            plan = Plan::Filter {
                source: Box::new(plan),
                predicate,
            };
        }

        // Step 5: If specific columns are requested, wrap in a Project
        let project_columns = self.resolve_select_columns(&columns, table)?;
        if project_columns.len() < table.columns.len() {
            plan = Plan::Project {
                source: Box::new(plan),
                columns: project_columns,
            };
        }

        Ok(plan)
    }

    /// Resolve SELECT column references.
    /// `SELECT *` expands to all columns; named columns are validated.
    fn resolve_select_columns(
        &self,
        columns: &[SelectColumn],
        table: &crate::sql::schema::Table,
    ) -> Result<Vec<String>, PlanError> {
        let mut result = Vec::new();

        for col in columns {
            match col {
                SelectColumn::AllColumns => {
                    // * expands to all columns in the table
                    result.extend(table.column_names());
                }
                SelectColumn::Named(name) => {
                    if !table.has_column(name) {
                        return Err(PlanError::ColumnNotFound {
                            column: name.clone(),
                            table: table.name.clone(),
                        });
                    }
                    result.push(name.clone());
                }
            }
        }

        Ok(result)
    }
}
```

Follow the flow: the planner starts at the bottom of the plan tree (Scan) and wraps nodes around it. This is building the tree inside-out: Scan first, then Filter wraps around it, then Project wraps around that. The result reads top-down but is built bottom-up.

Notice the iterator usage in `resolve_select_columns`: `.extend()` appends all elements from `table.column_names()` to the result vector. This is cleaner than a manual loop with `push()`.

### Step 4: Planning INSERT

```rust
impl<'a> Planner<'a> {
    fn plan_insert(
        &self,
        table_name: String,
        columns: Vec<String>,
        values: Vec<Vec<Expression>>,
    ) -> Result<Plan, PlanError> {
        let table = self.schema.get_table(&table_name).ok_or_else(|| {
            PlanError::TableNotFound(table_name.clone())
        })?;

        // Validate that all specified columns exist
        for col in &columns {
            if !table.has_column(col) {
                return Err(PlanError::ColumnNotFound {
                    column: col.clone(),
                    table: table_name.clone(),
                });
            }
        }

        // Validate row widths match column count
        let expected_width = columns.len();
        let rows: Vec<Vec<Value>> = values
            .into_iter()
            .map(|row| {
                if row.len() != expected_width {
                    return Err(PlanError::InvalidColumnCount {
                        expected: expected_width,
                        got: row.len(),
                    });
                }
                // Evaluate constant expressions to values
                row.into_iter()
                    .map(|expr| self.eval_constant(expr))
                    .collect::<Result<Vec<Value>, PlanError>>()
            })
            .collect::<Result<Vec<Vec<Value>>, PlanError>>()?;

        Ok(Plan::Insert {
            table: table_name,
            columns,
            rows,
        })
    }

    /// Evaluate a constant expression to a Value.
    /// In INSERT statements, values must be constants (no column references).
    fn eval_constant(&self, expr: Expression) -> Result<Value, PlanError> {
        match expr {
            Expression::Literal(value) => Ok(value),
            Expression::Column(name) => {
                Err(PlanError::TypeError(format!(
                    "column reference '{}' is not allowed in INSERT VALUES",
                    name
                )))
            }
            Expression::BinaryOp { left, op, right } => {
                let left_val = self.eval_constant(*left)?;
                let right_val = self.eval_constant(*right)?;
                self.eval_binary_op(&left_val, &op, &right_val)
            }
        }
    }

    /// Evaluate a binary operation on two constant values.
    fn eval_binary_op(
        &self,
        left: &Value,
        op: &Operator,
        right: &Value,
    ) -> Result<Value, PlanError> {
        match (left, op, right) {
            (Value::Integer(a), Operator::Add, Value::Integer(b)) => {
                Ok(Value::Integer(a + b))
            }
            (Value::Integer(a), Operator::Sub, Value::Integer(b)) => {
                Ok(Value::Integer(a - b))
            }
            (Value::Integer(a), Operator::Mul, Value::Integer(b)) => {
                Ok(Value::Integer(a * b))
            }
            (Value::Integer(a), Operator::Div, Value::Integer(b)) => {
                if *b == 0 {
                    Err(PlanError::TypeError("division by zero".to_string()))
                } else {
                    Ok(Value::Integer(a / b))
                }
            }
            (Value::Float(a), Operator::Add, Value::Float(b)) => {
                Ok(Value::Float(a + b))
            }
            _ => Err(PlanError::TypeError(format!(
                "cannot apply {:?} to {:?} and {:?}",
                op, left, right
            ))),
        }
    }
}
```

Study the nested `.collect()` calls in `plan_insert`. The outer `.collect::<Result<Vec<Vec<Value>>, PlanError>>()` collects a sequence of `Result<Vec<Value>, PlanError>` into a single `Result<Vec<Vec<Value>>, PlanError>`. If any row fails validation, the entire result is `Err`. This is the "collect Results" pattern from the Spotlight section — it replaces what would be a nested loop with early-return error handling.

### Step 5: Planning UPDATE and DELETE

```rust
impl<'a> Planner<'a> {
    fn plan_update(
        &self,
        table_name: String,
        assignments: Vec<(String, Expression)>,
        where_clause: Option<Expression>,
    ) -> Result<Plan, PlanError> {
        let table = self.schema.get_table(&table_name).ok_or_else(|| {
            PlanError::TableNotFound(table_name.clone())
        })?;

        // Validate that all assignment targets exist
        for (col, _) in &assignments {
            if !table.has_column(col) {
                return Err(PlanError::ColumnNotFound {
                    column: col.clone(),
                    table: table_name.clone(),
                });
            }
        }

        // Validate expressions in assignments
        for (_, expr) in &assignments {
            self.validate_expression(expr, table)?;
        }

        // Build the source plan (Scan + optional Filter)
        let column_infos: Vec<ColumnInfo> = table
            .columns
            .iter()
            .map(|c| ColumnInfo {
                name: c.name.clone(),
                data_type: c.data_type.clone(),
                nullable: c.nullable,
                primary_key: c.primary_key,
            })
            .collect();

        let mut source = Plan::Scan {
            table: table_name.clone(),
            columns: column_infos,
        };

        if let Some(predicate) = where_clause {
            self.validate_expression(&predicate, table)?;
            source = Plan::Filter {
                source: Box::new(source),
                predicate,
            };
        }

        Ok(Plan::Update {
            table: table_name,
            source: Box::new(source),
            assignments,
        })
    }

    fn plan_delete(
        &self,
        table_name: String,
        where_clause: Option<Expression>,
    ) -> Result<Plan, PlanError> {
        let table = self.schema.get_table(&table_name).ok_or_else(|| {
            PlanError::TableNotFound(table_name.clone())
        })?;

        let column_infos: Vec<ColumnInfo> = table
            .columns
            .iter()
            .map(|c| ColumnInfo {
                name: c.name.clone(),
                data_type: c.data_type.clone(),
                nullable: c.nullable,
                primary_key: c.primary_key,
            })
            .collect();

        let mut source = Plan::Scan {
            table: table_name.clone(),
            columns: column_infos,
        };

        if let Some(predicate) = where_clause {
            self.validate_expression(&predicate, table)?;
            source = Plan::Filter {
                source: Box::new(source),
                predicate,
            };
        }

        Ok(Plan::Delete {
            table: table_name,
            source: Box::new(source),
        })
    }

    fn plan_create_table(
        &self,
        name: String,
        columns: Vec<ColumnDef>,
    ) -> Result<Plan, PlanError> {
        if self.schema.has_table(&name) {
            return Err(PlanError::TableAlreadyExists(name));
        }

        Ok(Plan::CreateTable { name, columns })
    }
}
```

Notice the repeated pattern: resolve the table, build column infos, create a Scan, optionally wrap in a Filter. In the next chapter on query optimization, you will factor this out. For now, the repetition keeps each method self-contained and easy to follow.

### Step 6: Test the planner

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::sql::schema::{Column, Table};
    use crate::types::DataType;

    /// Helper: create a schema with a "users" table.
    fn test_schema() -> Schema {
        let mut schema = Schema::new();
        schema.add_table(Table {
            name: "users".to_string(),
            columns: vec![
                Column {
                    name: "id".to_string(),
                    data_type: DataType::Integer,
                    nullable: false,
                    primary_key: true,
                },
                Column {
                    name: "name".to_string(),
                    data_type: DataType::String,
                    nullable: false,
                    primary_key: false,
                },
                Column {
                    name: "age".to_string(),
                    data_type: DataType::Integer,
                    nullable: true,
                    primary_key: false,
                },
                Column {
                    name: "email".to_string(),
                    data_type: DataType::String,
                    nullable: true,
                    primary_key: false,
                },
            ],
        });
        schema
    }

    #[test]
    fn test_plan_select_all() {
        let schema = test_schema();
        let planner = Planner::new(&schema);

        let stmt = Statement::Select {
            columns: vec![SelectColumn::AllColumns],
            from: "users".to_string(),
            where_clause: None,
        };

        let plan = planner.plan(stmt).unwrap();

        // SELECT * with no WHERE should be just a Scan (no Project wrapping)
        match &plan {
            Plan::Scan { table, columns } => {
                assert_eq!(table, "users");
                assert_eq!(columns.len(), 4);
            }
            _ => panic!("expected Scan, got {:?}", plan),
        }
    }

    #[test]
    fn test_plan_select_with_filter_and_project() {
        let schema = test_schema();
        let planner = Planner::new(&schema);

        let stmt = Statement::Select {
            columns: vec![SelectColumn::Named("name".to_string())],
            from: "users".to_string(),
            where_clause: Some(Expression::BinaryOp {
                left: Box::new(Expression::Column("age".to_string())),
                op: Operator::Gt,
                right: Box::new(Expression::Literal(Value::Integer(18))),
            }),
        };

        let plan = planner.plan(stmt).unwrap();

        // Should be Project -> Filter -> Scan
        match &plan {
            Plan::Project { columns, source } => {
                assert_eq!(columns, &vec!["name".to_string()]);
                match source.as_ref() {
                    Plan::Filter { source, .. } => {
                        match source.as_ref() {
                            Plan::Scan { table, .. } => {
                                assert_eq!(table, "users");
                            }
                            _ => panic!("expected Scan"),
                        }
                    }
                    _ => panic!("expected Filter"),
                }
            }
            _ => panic!("expected Project"),
        }
    }

    #[test]
    fn test_plan_select_unknown_table() {
        let schema = test_schema();
        let planner = Planner::new(&schema);

        let stmt = Statement::Select {
            columns: vec![SelectColumn::AllColumns],
            from: "orders".to_string(),
            where_clause: None,
        };

        let err = planner.plan(stmt).unwrap_err();
        assert_eq!(err, PlanError::TableNotFound("orders".to_string()));
    }

    #[test]
    fn test_plan_select_unknown_column() {
        let schema = test_schema();
        let planner = Planner::new(&schema);

        let stmt = Statement::Select {
            columns: vec![SelectColumn::Named("address".to_string())],
            from: "users".to_string(),
            where_clause: None,
        };

        let err = planner.plan(stmt).unwrap_err();
        assert_eq!(
            err,
            PlanError::ColumnNotFound {
                column: "address".to_string(),
                table: "users".to_string(),
            }
        );
    }

    #[test]
    fn test_plan_insert() {
        let schema = test_schema();
        let planner = Planner::new(&schema);

        let stmt = Statement::Insert {
            table: "users".to_string(),
            columns: vec!["name".to_string(), "age".to_string()],
            values: vec![vec![
                Expression::Literal(Value::String("Alice".to_string())),
                Expression::Literal(Value::Integer(30)),
            ]],
        };

        let plan = planner.plan(stmt).unwrap();
        match &plan {
            Plan::Insert { table, columns, rows } => {
                assert_eq!(table, "users");
                assert_eq!(columns.len(), 2);
                assert_eq!(rows.len(), 1);
                assert_eq!(rows[0][0], Value::String("Alice".to_string()));
                assert_eq!(rows[0][1], Value::Integer(30));
            }
            _ => panic!("expected Insert"),
        }
    }

    #[test]
    fn test_plan_create_table_already_exists() {
        let schema = test_schema();
        let planner = Planner::new(&schema);

        let stmt = Statement::CreateTable {
            name: "users".to_string(),
            columns: vec![],
        };

        let err = planner.plan(stmt).unwrap_err();
        assert_eq!(
            err,
            PlanError::TableAlreadyExists("users".to_string())
        );
    }
}
```

Expected output:

```
running 6 tests
test sql::planner::tests::test_plan_select_all ... ok
test sql::planner::tests::test_plan_select_with_filter_and_project ... ok
test sql::planner::tests::test_plan_select_unknown_table ... ok
test sql::planner::tests::test_plan_select_unknown_column ... ok
test sql::planner::tests::test_plan_insert ... ok
test sql::planner::tests::test_plan_create_table_already_exists ... ok
test result: ok. 6 passed; 0 failed
```

---

## Exercise 3: Schema Validation

**Goal:** Add expression validation to the planner — verify that column references in WHERE clauses and assignments point to real columns, and catch type errors.

### Step 1: Expression validation

Add this method to the `Planner`:

```rust
impl<'a> Planner<'a> {
    /// Validate that all column references in an expression exist in the table.
    fn validate_expression(
        &self,
        expr: &Expression,
        table: &crate::sql::schema::Table,
    ) -> Result<(), PlanError> {
        match expr {
            Expression::Literal(_) => Ok(()),

            Expression::Column(name) => {
                if table.has_column(name) {
                    Ok(())
                } else {
                    Err(PlanError::ColumnNotFound {
                        column: name.clone(),
                        table: table.name.clone(),
                    })
                }
            }

            Expression::BinaryOp { left, op, right } => {
                self.validate_expression(left, table)?;
                self.validate_expression(right, table)?;
                self.validate_binary_op_types(left, op, right, table)
            }
        }
    }

    /// Check that a binary operation makes sense type-wise.
    fn validate_binary_op_types(
        &self,
        left: &Expression,
        op: &Operator,
        right: &Expression,
        table: &crate::sql::schema::Table,
    ) -> Result<(), PlanError> {
        let left_type = self.infer_type(left, table);
        let right_type = self.infer_type(right, table);

        match (left_type, right_type) {
            // If we cannot infer a type, skip the check
            (None, _) | (_, None) => Ok(()),

            (Some(lt), Some(rt)) => {
                match op {
                    // Comparison operators work on matching types
                    Operator::Eq | Operator::NotEq | Operator::Lt
                    | Operator::Gt | Operator::LtEq | Operator::GtEq => {
                        if lt != rt {
                            Err(PlanError::TypeError(format!(
                                "cannot compare {:?} with {:?}",
                                lt, rt
                            )))
                        } else {
                            Ok(())
                        }
                    }

                    // Arithmetic operators require numeric types
                    Operator::Add | Operator::Sub
                    | Operator::Mul | Operator::Div => {
                        match (&lt, &rt) {
                            (DataType::Integer, DataType::Integer)
                            | (DataType::Float, DataType::Float)
                            | (DataType::Integer, DataType::Float)
                            | (DataType::Float, DataType::Integer) => Ok(()),
                            _ => Err(PlanError::TypeError(format!(
                                "cannot perform arithmetic on {:?} and {:?}",
                                lt, rt
                            ))),
                        }
                    }

                    // Logical operators require booleans
                    Operator::And | Operator::Or => {
                        if lt != DataType::Boolean || rt != DataType::Boolean {
                            Err(PlanError::TypeError(format!(
                                "logical operators require Boolean operands, got {:?} and {:?}",
                                lt, rt
                            )))
                        } else {
                            Ok(())
                        }
                    }
                }
            }
        }
    }

    /// Infer the data type of an expression (best-effort).
    fn infer_type(
        &self,
        expr: &Expression,
        table: &crate::sql::schema::Table,
    ) -> Option<DataType> {
        match expr {
            Expression::Literal(value) => match value {
                Value::Null => None,
                Value::Boolean(_) => Some(DataType::Boolean),
                Value::Integer(_) => Some(DataType::Integer),
                Value::Float(_) => Some(DataType::Float),
                Value::String(_) => Some(DataType::String),
            },

            Expression::Column(name) => {
                table.get_column(name).map(|c| c.data_type.clone())
            }

            Expression::BinaryOp { left, op, .. } => {
                match op {
                    // Comparison and logical operators produce booleans
                    Operator::Eq | Operator::NotEq | Operator::Lt
                    | Operator::Gt | Operator::LtEq | Operator::GtEq
                    | Operator::And | Operator::Or => {
                        Some(DataType::Boolean)
                    }
                    // Arithmetic operators preserve the input type
                    _ => self.infer_type(left, table),
                }
            }
        }
    }
}
```

### Step 2: Understanding the validation flow

The `validate_expression` method performs a depth-first traversal of the expression tree. For each node:

1. **Literals** are always valid — they do not reference the schema.
2. **Column references** are checked against the table's column list.
3. **Binary operations** recursively validate both sides, then check type compatibility.

This is tree validation via DFS: visit the left subtree, visit the right subtree, then validate the current node. The `?` operator short-circuits on the first error — if the left side references a non-existent column, we stop immediately without checking the right side.

The `infer_type` method is best-effort. For `NULL` it returns `None` (unknown type), and the type checker skips the comparison. This is intentional — `NULL` is compatible with any type in SQL. A production database would have a more sophisticated type system, but this captures the essential idea.

### Step 3: Collecting column references with iterators

Here is a utility function that collects all column names referenced in an expression. It demonstrates a recursive iterator pattern:

```rust
/// Extract all column names referenced in an expression.
pub fn extract_columns(expr: &Expression) -> Vec<String> {
    match expr {
        Expression::Literal(_) => vec![],
        Expression::Column(name) => vec![name.clone()],
        Expression::BinaryOp { left, right, .. } => {
            let mut cols = extract_columns(left);
            cols.extend(extract_columns(right));
            cols
        }
    }
}
```

This recursive function gathers column references by walking the expression tree. The `extend` call appends all elements from the right subtree's result into the left subtree's result. An alternative approach using `into_iter()` and `chain()`:

```rust
pub fn extract_columns_chained(expr: &Expression) -> Vec<String> {
    match expr {
        Expression::Literal(_) => vec![],
        Expression::Column(name) => vec![name.clone()],
        Expression::BinaryOp { left, right, .. } => {
            extract_columns_chained(left)
                .into_iter()
                .chain(extract_columns_chained(right))
                .collect()
        }
    }
}
```

Both produce the same result. The `chain()` version is more functional; the `extend()` version avoids the intermediate allocation that `chain().collect()` creates. In practice, expression trees are small enough that neither matters for performance.

### Step 4: Test validation

```rust
#[cfg(test)]
mod validation_tests {
    use super::*;
    use crate::sql::schema::{Column, Table};
    use crate::types::DataType;

    fn test_schema() -> Schema {
        let mut schema = Schema::new();
        schema.add_table(Table {
            name: "users".to_string(),
            columns: vec![
                Column {
                    name: "id".to_string(),
                    data_type: DataType::Integer,
                    nullable: false,
                    primary_key: true,
                },
                Column {
                    name: "name".to_string(),
                    data_type: DataType::String,
                    nullable: false,
                    primary_key: false,
                },
                Column {
                    name: "age".to_string(),
                    data_type: DataType::Integer,
                    nullable: true,
                    primary_key: false,
                },
                Column {
                    name: "active".to_string(),
                    data_type: DataType::Boolean,
                    nullable: false,
                    primary_key: false,
                },
            ],
        });
        schema
    }

    #[test]
    fn test_valid_where_clause() {
        let schema = test_schema();
        let planner = Planner::new(&schema);

        // SELECT * FROM users WHERE age > 18
        let stmt = Statement::Select {
            columns: vec![SelectColumn::AllColumns],
            from: "users".to_string(),
            where_clause: Some(Expression::BinaryOp {
                left: Box::new(Expression::Column("age".to_string())),
                op: Operator::Gt,
                right: Box::new(Expression::Literal(Value::Integer(18))),
            }),
        };

        assert!(planner.plan(stmt).is_ok());
    }

    #[test]
    fn test_invalid_column_in_where() {
        let schema = test_schema();
        let planner = Planner::new(&schema);

        // SELECT * FROM users WHERE salary > 50000
        // "salary" does not exist in the users table
        let stmt = Statement::Select {
            columns: vec![SelectColumn::AllColumns],
            from: "users".to_string(),
            where_clause: Some(Expression::BinaryOp {
                left: Box::new(Expression::Column("salary".to_string())),
                op: Operator::Gt,
                right: Box::new(Expression::Literal(Value::Integer(50000))),
            }),
        };

        let err = planner.plan(stmt).unwrap_err();
        assert_eq!(
            err,
            PlanError::ColumnNotFound {
                column: "salary".to_string(),
                table: "users".to_string(),
            }
        );
    }

    #[test]
    fn test_type_mismatch_in_comparison() {
        let schema = test_schema();
        let planner = Planner::new(&schema);

        // SELECT * FROM users WHERE age > 'hello'
        // Comparing Integer column with String literal
        let stmt = Statement::Select {
            columns: vec![SelectColumn::AllColumns],
            from: "users".to_string(),
            where_clause: Some(Expression::BinaryOp {
                left: Box::new(Expression::Column("age".to_string())),
                op: Operator::Gt,
                right: Box::new(Expression::Literal(Value::String("hello".to_string()))),
            }),
        };

        let err = planner.plan(stmt).unwrap_err();
        match err {
            PlanError::TypeError(msg) => {
                assert!(msg.contains("compare"), "expected comparison error: {}", msg);
            }
            _ => panic!("expected TypeError, got {:?}", err),
        }
    }

    #[test]
    fn test_invalid_column_in_update() {
        let schema = test_schema();
        let planner = Planner::new(&schema);

        // UPDATE users SET salary = 50000
        // "salary" does not exist
        let stmt = Statement::Update {
            table: "users".to_string(),
            assignments: vec![(
                "salary".to_string(),
                Expression::Literal(Value::Integer(50000)),
            )],
            where_clause: None,
        };

        let err = planner.plan(stmt).unwrap_err();
        assert_eq!(
            err,
            PlanError::ColumnNotFound {
                column: "salary".to_string(),
                table: "users".to_string(),
            }
        );
    }

    #[test]
    fn test_insert_wrong_column_count() {
        let schema = test_schema();
        let planner = Planner::new(&schema);

        // INSERT INTO users (name, age) VALUES ('Alice')
        // 2 columns but only 1 value
        let stmt = Statement::Insert {
            table: "users".to_string(),
            columns: vec!["name".to_string(), "age".to_string()],
            values: vec![vec![
                Expression::Literal(Value::String("Alice".to_string())),
            ]],
        };

        let err = planner.plan(stmt).unwrap_err();
        assert_eq!(
            err,
            PlanError::InvalidColumnCount {
                expected: 2,
                got: 1,
            }
        );
    }

    #[test]
    fn test_extract_columns() {
        // age > 18 AND name = 'Alice'
        let expr = Expression::BinaryOp {
            left: Box::new(Expression::BinaryOp {
                left: Box::new(Expression::Column("age".to_string())),
                op: Operator::Gt,
                right: Box::new(Expression::Literal(Value::Integer(18))),
            }),
            op: Operator::And,
            right: Box::new(Expression::BinaryOp {
                left: Box::new(Expression::Column("name".to_string())),
                op: Operator::Eq,
                right: Box::new(Expression::Literal(Value::String("Alice".to_string()))),
            }),
        };

        let cols = extract_columns(&expr);
        assert_eq!(cols, vec!["age".to_string(), "name".to_string()]);
    }
}
```

Expected output:

```
running 6 tests
test sql::planner::validation_tests::test_valid_where_clause ... ok
test sql::planner::validation_tests::test_invalid_column_in_where ... ok
test sql::planner::validation_tests::test_type_mismatch_in_comparison ... ok
test sql::planner::validation_tests::test_invalid_column_in_update ... ok
test sql::planner::validation_tests::test_insert_wrong_column_count ... ok
test sql::planner::validation_tests::test_extract_columns ... ok
test result: ok. 6 passed; 0 failed
```

---

## Exercise 4: Plan Display

**Goal:** Implement `Display` for `Plan` to print the plan tree as an indented structure, similar to SQL's `EXPLAIN` output.

### Step 1: The Display implementation

```rust
use std::fmt;

impl fmt::Display for Plan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.format(f, 0)
    }
}

impl Plan {
    /// Recursively format the plan tree with indentation.
    fn format(&self, f: &mut fmt::Formatter<'_>, indent: usize) -> fmt::Result {
        let prefix = "  ".repeat(indent);

        match self {
            Plan::Scan { table, columns } => {
                let col_names: Vec<&str> = columns
                    .iter()
                    .map(|c| c.name.as_str())
                    .collect();
                write!(f, "{}Scan {} [{}]", prefix, table, col_names.join(", "))
            }

            Plan::Filter { source, predicate } => {
                writeln!(f, "{}Filter ({})", prefix, format_expr(predicate))?;
                source.format(f, indent + 1)
            }

            Plan::Project { source, columns } => {
                writeln!(f, "{}Project [{}]", prefix, columns.join(", "))?;
                source.format(f, indent + 1)
            }

            Plan::Insert { table, columns, rows } => {
                write!(
                    f,
                    "{}Insert into {} [{}] ({} row{})",
                    prefix,
                    table,
                    columns.join(", "),
                    rows.len(),
                    if rows.len() == 1 { "" } else { "s" }
                )
            }

            Plan::Update { table, source, assignments } => {
                let targets: Vec<&str> = assignments
                    .iter()
                    .map(|(col, _)| col.as_str())
                    .collect();
                writeln!(
                    f,
                    "{}Update {} set [{}]",
                    prefix,
                    table,
                    targets.join(", ")
                )?;
                source.format(f, indent + 1)
            }

            Plan::Delete { table, source } => {
                writeln!(f, "{}Delete from {}", prefix, table)?;
                source.format(f, indent + 1)
            }

            Plan::CreateTable { name, columns } => {
                let col_defs: Vec<String> = columns
                    .iter()
                    .map(|c| format!("{} {:?}", c.name, c.data_type))
                    .collect();
                write!(
                    f,
                    "{}CreateTable {} ({})",
                    prefix,
                    name,
                    col_defs.join(", ")
                )
            }
        }
    }
}

/// Format an expression as a human-readable string.
fn format_expr(expr: &Expression) -> String {
    match expr {
        Expression::Literal(value) => match value {
            Value::Null => "NULL".to_string(),
            Value::Boolean(b) => b.to_string(),
            Value::Integer(n) => n.to_string(),
            Value::Float(f) => format!("{}", f),
            Value::String(s) => format!("'{}'", s),
        },

        Expression::Column(name) => name.clone(),

        Expression::BinaryOp { left, op, right } => {
            let op_str = match op {
                Operator::Eq => "=",
                Operator::NotEq => "!=",
                Operator::Lt => "<",
                Operator::Gt => ">",
                Operator::LtEq => "<=",
                Operator::GtEq => ">=",
                Operator::And => "AND",
                Operator::Or => "OR",
                Operator::Add => "+",
                Operator::Sub => "-",
                Operator::Mul => "*",
                Operator::Div => "/",
            };
            format!(
                "{} {} {}",
                format_expr(left),
                op_str,
                format_expr(right)
            )
        }
    }
}
```

### Step 2: Understanding the recursive formatting

The `format` method is a depth-first traversal of the plan tree. Each recursive call increases the indent level by 1, producing a tree that reads naturally from top to bottom:

```
Project [name]
  Filter (age > 18)
    Scan users [id, name, age, email]
```

The root (Project) has indent 0, Filter has indent 1, Scan has indent 2. This is the same structure that `EXPLAIN` shows in PostgreSQL or MySQL — each line represents a plan node, and indentation shows the parent-child relationship.

Notice the difference between `write!` and `writeln!`. Leaf nodes (Scan, Insert, CreateTable) use `write!` without a newline because they have no children. Inner nodes (Filter, Project, Update, Delete) use `writeln!` to end their own line, then call `source.format()` which writes the child.

### Step 3: Test the display output

```rust
#[cfg(test)]
mod display_tests {
    use super::*;
    use crate::types::DataType;

    #[test]
    fn test_display_select_plan() {
        let plan = Plan::Project {
            source: Box::new(Plan::Filter {
                source: Box::new(Plan::Scan {
                    table: "users".to_string(),
                    columns: vec![
                        ColumnInfo {
                            name: "id".to_string(),
                            data_type: DataType::Integer,
                            nullable: false,
                            primary_key: true,
                        },
                        ColumnInfo {
                            name: "name".to_string(),
                            data_type: DataType::String,
                            nullable: false,
                            primary_key: false,
                        },
                        ColumnInfo {
                            name: "age".to_string(),
                            data_type: DataType::Integer,
                            nullable: true,
                            primary_key: false,
                        },
                        ColumnInfo {
                            name: "email".to_string(),
                            data_type: DataType::String,
                            nullable: true,
                            primary_key: false,
                        },
                    ],
                }),
                predicate: Expression::BinaryOp {
                    left: Box::new(Expression::Column("age".to_string())),
                    op: Operator::Gt,
                    right: Box::new(Expression::Literal(Value::Integer(18))),
                },
            }),
            columns: vec!["name".to_string()],
        };

        let output = format!("{}", plan);
        let expected = "\
Project [name]
  Filter (age > 18)
    Scan users [id, name, age, email]";

        assert_eq!(output, expected);
        println!("{}", plan);
    }

    #[test]
    fn test_display_insert_plan() {
        let plan = Plan::Insert {
            table: "users".to_string(),
            columns: vec!["name".to_string(), "age".to_string()],
            rows: vec![
                vec![Value::String("Alice".to_string()), Value::Integer(30)],
                vec![Value::String("Bob".to_string()), Value::Integer(25)],
            ],
        };

        let output = format!("{}", plan);
        assert_eq!(output, "Insert into users [name, age] (2 rows)");
        println!("{}", plan);
    }

    #[test]
    fn test_display_delete_with_filter() {
        let plan = Plan::Delete {
            table: "users".to_string(),
            source: Box::new(Plan::Filter {
                source: Box::new(Plan::Scan {
                    table: "users".to_string(),
                    columns: vec![
                        ColumnInfo {
                            name: "id".to_string(),
                            data_type: DataType::Integer,
                            nullable: false,
                            primary_key: true,
                        },
                    ],
                }),
                predicate: Expression::BinaryOp {
                    left: Box::new(Expression::Column("id".to_string())),
                    op: Operator::Eq,
                    right: Box::new(Expression::Literal(Value::Integer(42))),
                },
            }),
        };

        let output = format!("{}", plan);
        let expected = "\
Delete from users
  Filter (id = 42)
    Scan users [id]";

        assert_eq!(output, expected);
        println!("{}", plan);
    }

    #[test]
    fn test_display_complex_where() {
        // WHERE age > 18 AND name = 'Alice'
        let plan = Plan::Filter {
            source: Box::new(Plan::Scan {
                table: "users".to_string(),
                columns: vec![
                    ColumnInfo {
                        name: "id".to_string(),
                        data_type: DataType::Integer,
                        nullable: false,
                        primary_key: true,
                    },
                    ColumnInfo {
                        name: "name".to_string(),
                        data_type: DataType::String,
                        nullable: false,
                        primary_key: false,
                    },
                    ColumnInfo {
                        name: "age".to_string(),
                        data_type: DataType::Integer,
                        nullable: true,
                        primary_key: false,
                    },
                ],
            }),
            predicate: Expression::BinaryOp {
                left: Box::new(Expression::BinaryOp {
                    left: Box::new(Expression::Column("age".to_string())),
                    op: Operator::Gt,
                    right: Box::new(Expression::Literal(Value::Integer(18))),
                }),
                op: Operator::And,
                right: Box::new(Expression::BinaryOp {
                    left: Box::new(Expression::Column("name".to_string())),
                    op: Operator::Eq,
                    right: Box::new(Expression::Literal(Value::String(
                        "Alice".to_string(),
                    ))),
                }),
            },
        };

        let output = format!("{}", plan);
        let expected = "\
Filter (age > 18 AND name = 'Alice')
  Scan users [id, name, age]";

        assert_eq!(output, expected);
        println!("{}", plan);
    }
}
```

Expected output:

```
running 4 tests
test sql::plan::display_tests::test_display_select_plan ... ok
Project [name]
  Filter (age > 18)
    Scan users [id, name, age, email]
test sql::plan::display_tests::test_display_insert_plan ... ok
Insert into users [name, age] (2 rows)
test sql::plan::display_tests::test_display_delete_with_filter ... ok
Delete from users
  Filter (id = 42)
    Scan users [id]
test sql::plan::display_tests::test_display_complex_where ... ok
Filter (age > 18 AND name = 'Alice')
  Scan users [id, name, age]
test result: ok. 4 passed; 0 failed
```

---

## Rust Gym

Three focused exercises to build iterator and closure fluency. All exercises use `std` only.

### Rep 1: Fibonacci Iterator

Build a custom iterator that produces Fibonacci numbers:

```rust
struct Fibonacci {
    a: u64,
    b: u64,
}

impl Fibonacci {
    fn new() -> Self {
        Fibonacci { a: 0, b: 1 }
    }
}

// TODO: Implement Iterator for Fibonacci
// Each call to next() should return the next Fibonacci number:
// 0, 1, 1, 2, 3, 5, 8, 13, 21, ...
```

<details>
<summary>Hint</summary>

The state update is: compute the next value as `a + b`, then shift: `a` becomes the old `b`, and `b` becomes the new sum. Since the iterator is infinite (Fibonacci numbers never end), `next()` always returns `Some(value)`.

</details>

<details>
<summary>Solution</summary>

```rust
impl Iterator for Fibonacci {
    type Item = u64;

    fn next(&mut self) -> Option<u64> {
        let value = self.a;
        let next = self.a + self.b;
        self.a = self.b;
        self.b = next;
        Some(value)
    }
}

fn main() {
    // First 10 Fibonacci numbers
    let fibs: Vec<u64> = Fibonacci::new().take(10).collect();
    println!("{:?}", fibs);
    // [0, 1, 1, 2, 3, 5, 8, 13, 21, 34]

    // Sum of first 20 Fibonacci numbers
    let sum: u64 = Fibonacci::new().take(20).sum();
    println!("Sum of first 20: {}", sum);
    // Sum of first 20: 17710

    // First Fibonacci number greater than 1000
    let big = Fibonacci::new().find(|&n| n > 1000);
    println!("First > 1000: {:?}", big);
    // First > 1000: Some(1597)
}
```

Key observations:
- `.take(10)` limits an infinite iterator to 10 elements.
- `.sum()` consumes the iterator and adds all elements.
- `.find()` returns the first element matching the predicate, wrapped in `Option`.
- Because `Fibonacci` always returns `Some(...)`, it is an infinite iterator. Without `.take()` or `.find()`, calling `.collect()` would run forever.

</details>

### Rep 2: Iterator Chain — Filter, Map, Collect

Given a list of strings representing log entries, extract and count the error messages:

```rust
fn process_logs(logs: &[&str]) -> Vec<String> {
    // TODO: Use an iterator chain to:
    // 1. Filter only lines starting with "ERROR"
    // 2. Map to extract just the message (after "ERROR: ")
    // 3. Convert to uppercase
    // 4. Collect into a Vec<String>
    todo!()
}

fn count_by_level(logs: &[&str]) -> Vec<(String, usize)> {
    // TODO: Use iterators to count how many logs exist at each level.
    // Return pairs like [("ERROR", 3), ("INFO", 5), ("WARN", 2)],
    // sorted alphabetically by level.
    todo!()
}

fn main() {
    let logs = vec![
        "INFO: server started",
        "ERROR: connection refused",
        "INFO: request received",
        "WARN: slow query (2.3s)",
        "ERROR: disk full",
        "INFO: request completed",
        "ERROR: timeout",
    ];

    let errors = process_logs(&logs);
    println!("Errors: {:?}", errors);

    let counts = count_by_level(&logs);
    println!("Counts: {:?}", counts);
}
```

<details>
<summary>Hint</summary>

For `process_logs`: chain `.filter()`, `.map()`, and `.collect()`. Use `str::strip_prefix("ERROR: ")` (returns `Option<&str>`) or check `starts_with()` then slice. For `count_by_level`: use a `HashMap` to count occurrences, then collect into a sorted `Vec`.

</details>

<details>
<summary>Solution</summary>

```rust
use std::collections::HashMap;

fn process_logs(logs: &[&str]) -> Vec<String> {
    logs.iter()
        .filter_map(|line| line.strip_prefix("ERROR: "))
        .map(|msg| msg.to_uppercase())
        .collect()
}

fn count_by_level(logs: &[&str]) -> Vec<(String, usize)> {
    let mut counts: HashMap<String, usize> = HashMap::new();

    for log in logs {
        // Extract the level (everything before the first ':')
        if let Some(level) = log.split(':').next() {
            *counts.entry(level.trim().to_string()).or_insert(0) += 1;
        }
    }

    let mut result: Vec<(String, usize)> = counts.into_iter().collect();
    result.sort_by(|a, b| a.0.cmp(&b.0));
    result
}

fn main() {
    let logs = vec![
        "INFO: server started",
        "ERROR: connection refused",
        "INFO: request received",
        "WARN: slow query (2.3s)",
        "ERROR: disk full",
        "INFO: request completed",
        "ERROR: timeout",
    ];

    let errors = process_logs(&logs);
    println!("Errors: {:?}", errors);
    // Errors: ["CONNECTION REFUSED", "DISK FULL", "TIMEOUT"]

    let counts = count_by_level(&logs);
    println!("Counts: {:?}", counts);
    // Counts: [("ERROR", 3), ("INFO", 3), ("WARN", 1)]
}
```

Note the use of `filter_map` — it combines `filter` and `map` in one step. `strip_prefix` returns `Some(&str)` if the prefix matched (with the prefix removed), or `None` if it did not. `filter_map` keeps only the `Some` values and unwraps them. This is a common Rust idiom: "try to transform each element, keep only the successes."

</details>

### Rep 3: Tree Walker Iterator

Build a custom iterator that performs a depth-first traversal of a binary tree:

```rust
#[derive(Debug)]
struct TreeNode {
    value: i32,
    left: Option<Box<TreeNode>>,
    right: Option<Box<TreeNode>>,
}

impl TreeNode {
    fn leaf(value: i32) -> Self {
        TreeNode { value, left: None, right: None }
    }

    fn with_children(value: i32, left: TreeNode, right: TreeNode) -> Self {
        TreeNode {
            value,
            left: Some(Box::new(left)),
            right: Some(Box::new(right)),
        }
    }
}

struct TreeIter<'a> {
    stack: Vec<&'a TreeNode>,
}

// TODO: Implement TreeIter::new() and Iterator for TreeIter
// The iterator should yield values in pre-order: root, left, right
```

<details>
<summary>Hint</summary>

Pre-order DFS uses a stack. Push the root first. On each `next()` call, pop a node, push its right child first (so left is processed first), then push its left child. Return the popped node's value.

</details>

<details>
<summary>Solution</summary>

```rust
impl<'a> TreeIter<'a> {
    fn new(root: &'a TreeNode) -> Self {
        TreeIter { stack: vec![root] }
    }
}

impl<'a> Iterator for TreeIter<'a> {
    type Item = i32;

    fn next(&mut self) -> Option<i32> {
        let node = self.stack.pop()?;

        // Push right first so left is processed first (LIFO)
        if let Some(right) = &node.right {
            self.stack.push(right);
        }
        if let Some(left) = &node.left {
            self.stack.push(left);
        }

        Some(node.value)
    }
}

fn main() {
    //       1
    //      / \
    //     2   3
    //    / \
    //   4   5
    let tree = TreeNode::with_children(
        1,
        TreeNode::with_children(
            2,
            TreeNode::leaf(4),
            TreeNode::leaf(5),
        ),
        TreeNode::leaf(3),
    );

    let values: Vec<i32> = TreeIter::new(&tree).collect();
    println!("Pre-order: {:?}", values);
    // Pre-order: [1, 2, 4, 5, 3]

    // Use iterator adaptors on the tree
    let sum: i32 = TreeIter::new(&tree).sum();
    println!("Sum: {}", sum);
    // Sum: 15

    let evens: Vec<i32> = TreeIter::new(&tree)
        .filter(|v| v % 2 == 0)
        .collect();
    println!("Even values: {:?}", evens);
    // Even values: [2, 4]
}
```

This is powerful: once you implement `Iterator`, you get `.sum()`, `.filter()`, `.map()`, `.any()`, `.find()`, and dozens of other methods for free. The tree walker produces values lazily — if you only need `.find(|v| v > 3)`, it stops traversal as soon as it finds 4, without visiting the rest of the tree.

This is exactly how the query executor in Chapter 10 will work. Plan nodes will be iterators that produce rows, and the executor will chain them together with iterator adaptors.

</details>

---

## DSA in Context: Tree-to-Tree Transformation

The query planner performs a **tree-to-tree transformation**: it takes an AST (one tree structure) and produces a plan (a different tree structure). This is a fundamental operation in computer science — compilers, transpilers, and query engines all do it.

### The transformation pattern

The AST for `SELECT name FROM users WHERE age > 18`:

```
Statement::Select
├── columns: [Named("name")]
├── from: "users"
└── where_clause:
    BinaryOp (>)
    ├── Column("age")
    └── Literal(18)
```

The plan tree for the same query:

```
Plan::Project [name]
└── Plan::Filter (age > 18)
    └── Plan::Scan users [id, name, age, email]
```

These are different tree shapes. The AST mirrors the SQL syntax (SELECT comes first, FROM comes second). The plan mirrors the execution order (Scan happens first, Project happens last). The planner maps one structure to the other.

### DFS for validation

When the planner validates an expression like `age > 18 AND name = 'Alice'`, it performs a DFS traversal of the expression tree:

```
        AND
       /   \
      >      =
     / \    / \
   age  18 name 'Alice'
```

The DFS visits nodes in this order: `AND` -> `>` -> `age` -> `18` -> `=` -> `name` -> `'Alice'`. At each leaf, it checks whether column references are valid. At each inner node, it checks type compatibility. The `?` operator provides early termination: if `age` does not exist, the traversal stops immediately.

This DFS validation is O(n) where n is the number of nodes in the expression tree. Each node is visited exactly once. There is no need for BFS here because validation does not depend on level ordering — we just need to visit every node.

### Recursive descent for transformation

The planner uses **recursive descent** to build the plan. For a SELECT statement:

1. Look up the table name in the schema (resolves the leaf).
2. Build a Scan node (the deepest node in the plan tree).
3. If there is a WHERE clause, wrap the Scan in a Filter.
4. If there are specific columns, wrap in a Project.

Each step takes the current subtree and wraps it in a new node. This is the same pattern as building a linked list by prepending: each new node becomes the root, and the old tree becomes a child.

---

## System Design Corner: Query Planning in Production

### Rule-based vs cost-based planning

Our planner uses **rule-based planning**: fixed rules determine the plan structure. SELECT always becomes Project -> Filter -> Scan. There is no choice involved.

Production databases use **cost-based planning**. The planner generates multiple candidate plans and estimates the cost (I/O, CPU, memory) of each one, then picks the cheapest. For example, `SELECT * FROM users WHERE id = 42` could be executed as:

- **Plan A:** Full table scan, then filter. Cost: read all N rows.
- **Plan B:** Index lookup on `id`, fetch one row. Cost: read ~1 row.

A cost-based planner would pick Plan B. Our rule-based planner always picks Plan A (full scan), because we have not built indexes yet.

The cost model typically considers:
- **Table statistics** — row count, column cardinality (number of distinct values), data distribution
- **Index availability** — which columns have indexes, index type (B-tree, hash)
- **Join ordering** — for multi-table queries, the order of joins dramatically affects cost
- **Memory budget** — can the intermediate result fit in memory, or do we need to spill to disk?

PostgreSQL's planner, for example, considers over a dozen different join strategies and uses dynamic programming to find the optimal join order for up to ~12 tables.

### Plan caching and prepared statements

Parsing and planning are expensive relative to executing simple queries. For queries that are executed repeatedly with different parameters, databases support **prepared statements**:

```sql
-- Parse and plan once
PREPARE get_user AS SELECT * FROM users WHERE id = $1;

-- Execute many times with different values
EXECUTE get_user(42);
EXECUTE get_user(99);
EXECUTE get_user(7);
```

The database parses and plans the query once, storing the plan. Subsequent executions reuse the cached plan, substituting the parameter values. This saves the cost of lexing, parsing, and planning on every call.

In our database, implementing prepared statements would mean:
1. Parse the SQL once, producing an AST with parameter placeholders.
2. Plan the AST once, producing a plan with parameter slots.
3. On execution, substitute parameter values into the plan and execute.

We will not build this, but understanding the concept explains why separating planning from execution matters — you cannot cache a plan if planning and execution are interleaved.

### Query plan visualization

PostgreSQL's `EXPLAIN` command shows the plan tree. `EXPLAIN ANALYZE` executes the query and shows actual timing alongside estimates:

```
EXPLAIN ANALYZE SELECT name FROM users WHERE age > 18;

Seq Scan on users  (cost=0.00..35.50 rows=1000 width=32) (actual time=0.01..0.15 rows=847 loops=1)
  Filter: (age > 18)
  Rows Removed by Filter: 153
Planning Time: 0.05 ms
Execution Time: 0.25 ms
```

Our `Display` implementation for `Plan` is a simplified version of this. In Chapter 9 (optimizer) and Chapter 10 (executor), we could extend it to show estimated and actual costs.

---

## Design Insight: Strategic Programming

John Ousterhout draws a distinction between **tactical** and **strategic** programming in *A Philosophy of Software Design*.

A tactical programmer would skip the planner entirely. "Why build a separate plan representation? Just walk the AST and execute directly." This works in the short term and produces fewer lines of code. The SQL executor would contain a big match statement that handles AST nodes and reads/writes data in the same function.

A strategic programmer builds the planner as a separate stage, even though it seems like unnecessary work. The payoff comes later:

1. **Optimization is separate from planning.** In Chapter 9, we will add an optimizer that transforms plans into better plans (e.g., pushing filters closer to scans, eliminating redundant projections). This works because the optimizer receives a `Plan` and returns a `Plan` — it never touches the AST. If planning and execution were combined, adding optimization would require rewriting the executor.

2. **Execution is separate from validation.** The executor in Chapter 10 can assume the plan is valid — tables exist, columns are correct, types match. It does not need error-handling code for schema validation. This makes the executor simpler and faster.

3. **Testing is separated.** We can test the planner without executing anything. We can test the executor with hand-crafted plans without parsing SQL. Each stage is independently testable.

4. **Plan display is free.** Because the plan is a data structure, we can print it (`EXPLAIN`), serialize it, cache it, or send it over the network. If the plan were interleaved with execution, you would need to add instrumentation to an executing system — much harder.

The pipeline is:

```
SQL String → [Lexer] → Tokens → [Parser] → AST → [Planner] → Plan → [Optimizer] → Plan → [Executor] → Results
```

Each arrow is a clean boundary. Each stage has a defined input type and output type. You can replace, test, or optimize each stage independently. This is strategic programming: invest in structure now, and the system becomes easier to extend later.

Ousterhout's principle: *complexity is anything related to the structure of a system that makes it hard to understand and modify.* The planner adds a stage, but it reduces complexity by isolating concerns. The alternative — a monolithic execute-from-AST function — is simpler to write but harder to extend.

---

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

**-> [Tree Transformations: AST to Plan](../ds-narratives/ch08-tree-transformations.md)**

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
