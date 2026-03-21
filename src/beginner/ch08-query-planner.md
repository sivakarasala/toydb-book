# Chapter 8: Query Planner

In the last two chapters, we built a lexer and a parser. Given the SQL string `SELECT name FROM users WHERE age > 18`, we can now produce a beautiful tree -- an Abstract Syntax Tree -- that describes the structure of the query. But the AST describes WHAT the user asked for. It does not describe HOW to get it.

Think of the difference between a recipe and the act of cooking. The recipe says "make a cake" -- it lists ingredients and steps. But you still need to actually open the fridge, measure the flour, preheat the oven, and mix the batter. The recipe is the AST. The cooking instructions -- in the right order, with the right tools -- that is the **query plan**.

This chapter builds a query planner that transforms the AST into a plan -- a tree of operations that an executor can process step by step. The planner also validates your SQL: it checks that tables exist, columns are real, and types make sense. If you write `SELECT name FROM unicorns`, the planner will tell you there is no table called `unicorns`.

By the end of this chapter, you will have:

- A `Plan` enum representing execution plan nodes (Scan, Filter, Project, Insert, CreateTable)
- A `Schema` catalog that knows which tables and columns exist
- A `Planner` that transforms a parsed `Statement` into a validated `Plan`
- A `Display` implementation that prints plans as indented trees (like SQL's EXPLAIN)
- A deep understanding of Rust iterators and closures

---

## Spotlight: Iterators & Closures

Every chapter has one **spotlight concept** -- the Rust idea we dig into deeply. This chapter's spotlight is **iterators and closures**.

### What is an iterator?

An iterator is something that produces a sequence of values, one at a time. Think of a ticket dispenser at a deli counter. Each time you pull a ticket, you get the next number. The dispenser keeps track of where it is. When there are no more tickets, it stops.

In Rust, iterators work through a single trait with a single method:

```rust
trait Iterator {
    type Item;
    fn next(&mut self) -> Option<Self::Item>;
}
```

Let us break this down:

- **`type Item`** -- what kind of values does this iterator produce? Numbers? Strings? Database rows? This is an "associated type" -- a type that is defined by the implementation.
- **`fn next(&mut self)`** -- give me the next value. Returns `Some(value)` if there is one, or `None` when the sequence is finished.
- **`&mut self`** -- the iterator can change its internal state (like advancing a counter). That is why it needs a mutable reference.

### Your first encounter: for loops

Every `for` loop you have written in Rust uses an iterator behind the scenes:

```rust
let names = vec!["Alice", "Bob", "Carol"];

// This:
for name in &names {
    println!("{}", name);
}

// Is actually this:
let mut iter = names.iter();
loop {
    match iter.next() {
        Some(name) => println!("{}", name),
        None => break,
    }
}
```

`vec.iter()` creates an iterator over references to the items in the vector. Each call to `.next()` returns the next item. When we have gone through all items, `.next()` returns `None` and the loop ends.

> **What just happened?**
>
> A `for` loop is syntactic sugar -- a convenient shorthand -- for repeatedly calling `.next()` on an iterator. The compiler transforms your `for` loop into the `loop` / `match` version automatically. You do not pay any performance cost for using `for` -- it compiles to the same code.

### The power of iterator adapters

The magic of iterators is that you can chain transformations together. Each transformation is called an **adapter** -- it takes an iterator and produces a new iterator:

```rust
let numbers = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

// .filter() keeps only elements that match a condition
let evens: Vec<&i32> = numbers.iter()
    .filter(|n| **n % 2 == 0)
    .collect();
// [2, 4, 6, 8, 10]
```

Let us unpack this step by step:

1. `numbers.iter()` -- create an iterator over `&i32` references
2. `.filter(|n| **n % 2 == 0)` -- keep only even numbers
3. `.collect()` -- gather the results into a `Vec`

The thing inside `.filter()` -- that `|n| **n % 2 == 0` -- is called a **closure**. We will explain closures in detail shortly.

### More adapter examples

```rust
let numbers = vec![1, 2, 3, 4, 5];

// .map() transforms each element
let doubled: Vec<i32> = numbers.iter()
    .map(|n| n * 2)
    .collect();
// [2, 4, 6, 8, 10]

// Chain adapters together
let result: Vec<i32> = numbers.iter()
    .filter(|n| **n % 2 == 0)    // keep evens: [2, 4]
    .map(|n| n * n)               // square them: [4, 16]
    .collect();
// [4, 16]

// .enumerate() pairs each element with its index
let indexed: Vec<(usize, &i32)> = numbers.iter()
    .enumerate()
    .collect();
// [(0, 1), (1, 2), (2, 3), (3, 4), (4, 5)]

// .any() checks if ANY element matches
let has_even = numbers.iter().any(|n| n % 2 == 0);
// true

// .find() returns the first match
let first_even = numbers.iter().find(|n| **n % 2 == 0);
// Some(2)
```

### What are closures?

A closure is a small, anonymous function that you define right where you need it. Think of it as a sticky note with instructions that you hand to someone.

```rust
// A regular function
fn double(x: i32) -> i32 {
    x * 2
}

// The same thing as a closure
let double = |x: i32| -> i32 { x * 2 };

// Closures can be even shorter -- types are often inferred
let double = |x| x * 2;
```

The `|x|` part is the parameter list (between pipes instead of parentheses). The `x * 2` part is the body. If the body is a single expression, you do not need curly braces or a `return` statement.

The special thing about closures is that they can **capture** variables from their surroundings:

```rust
let threshold = 18;

// This closure "captures" the threshold variable
let is_adult = |age: i32| age >= threshold;

println!("{}", is_adult(21));  // true
println!("{}", is_adult(15));  // false
```

A regular function cannot access `threshold` -- it only sees its parameters. A closure can reach out and grab variables from the code around it. That is what makes closures so useful with iterator adapters: you can customize the behavior of `.filter()`, `.map()`, etc. with values from your surrounding code.

> **What just happened?**
>
> A closure is like a sticky note that says "do this" plus a snapshot of any variables it needs from the surrounding code. When you write `.filter(|n| *n > threshold)`, you are handing the iterator a sticky note that says "keep items greater than 18." The iterator reads the sticky note each time it has a new item.

### Closures with iterators: a real example

Here is a practical example from our codebase. Suppose we have a list of column names and want to check if a specific column exists:

```rust
let columns = vec!["id", "name", "age", "email"];

// Does the column "age" exist?
let exists = columns.iter().any(|col| *col == "age");
// true

// Find all columns that start with "e"
let e_columns: Vec<&&str> = columns.iter()
    .filter(|col| col.starts_with("e"))
    .collect();
// ["email"]

// Transform column names to uppercase
let upper: Vec<String> = columns.iter()
    .map(|col| col.to_uppercase())
    .collect();
// ["ID", "NAME", "AGE", "EMAIL"]
```

### .collect() and the turbofish

`.collect()` is special -- it can produce different collection types depending on what you ask for. The type annotation tells Rust what to collect into:

```rust
let numbers = vec![1, 2, 3, 4, 5];

// Collect into a Vec
let v: Vec<i32> = numbers.iter().copied().collect();

// Or use "turbofish" syntax (the ::<> after collect)
let v = numbers.iter().copied().collect::<Vec<i32>>();
```

The `::<Vec<i32>>` syntax is called "turbofish" because it looks like a fish: `::<>`. It provides a type hint to `.collect()` when Rust cannot infer the type from context.

### Collecting Results: stopping at the first error

One powerful pattern is collecting a sequence of `Result` values. If any element is an `Err`, the entire collection becomes an `Err`:

```rust
let strings = vec!["1", "2", "oops", "4"];
let parsed: Result<Vec<i32>, _> = strings.iter()
    .map(|s| s.parse::<i32>())
    .collect();
// Err(ParseIntError) -- stopped at "oops"
```

This is incredibly useful in the planner: when validating a list of columns, we want to stop at the first invalid column and return an error.

> **Common Mistakes**
>
> 1. **Forgetting `.collect()`**: Iterator adapters are lazy -- `.filter()` and `.map()` do not actually do anything until you consume the iterator (with `.collect()`, `for`, `.count()`, etc.). If you write `numbers.iter().filter(...)` without `.collect()`, nothing happens.
>
> 2. **Double references in `.filter()`**: When you call `.iter()` on a `Vec<i32>`, you get an iterator of `&i32`. Then `.filter()` gives you `&&i32` (a reference to a reference). Use `**n` to dereference twice, or `|&&n|` in the pattern to destructure.
>
> 3. **Using `.iter()` vs `.into_iter()`**: `.iter()` borrows the elements (you keep the original Vec). `.into_iter()` takes ownership (the original Vec is consumed). Use `.iter()` when you want to keep the original data.

---

## Exercise 1: Define Plan Nodes

**Goal:** Define the `Plan` enum -- a tree of nodes describing how to execute a query.

### Step 1: Create the plan module

Create `src/plan.rs` and register it:

```rust
// src/lib.rs
pub mod lexer;
pub mod parser;
pub mod plan;
```

### Step 2: Supporting types

Before defining the plan, we need metadata about columns:

```rust
// src/plan.rs
use crate::parser::{ColumnDef, DataType, Expression, Operator, Value};
use std::fmt;

/// Metadata about a column in a plan.
/// This is richer than the raw column name -- it includes
/// type information resolved from the schema.
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: DataType,
}
```

### Step 3: Define the Plan enum

Each plan node represents one step in query execution. Plans form a tree -- a Project node contains a Filter node, which contains a Scan node:

```rust
/// A plan node represents a single step in query execution.
///
/// Think of a plan as a recipe: each step transforms data from the
/// step before it. Data flows from the bottom of the tree upward.
///
/// Example for "SELECT name FROM users WHERE age > 18":
///
/// Project [name]          <-- pick the "name" column
///   Filter (age > 18)     <-- keep rows where age > 18
///     Scan users          <-- read all rows from the "users" table
///
#[derive(Debug, Clone, PartialEq)]
pub enum Plan {
    /// Read all rows from a table.
    /// This is always at the bottom (leaf) of the plan tree.
    Scan {
        table: String,
        columns: Vec<ColumnInfo>,
    },

    /// Keep only rows that match a condition.
    /// Wraps another plan (the source of rows to filter).
    Filter {
        source: Box<Plan>,
        predicate: Expression,
    },

    /// Select specific columns from each row.
    /// Wraps another plan (the source of rows to project).
    Project {
        source: Box<Plan>,
        columns: Vec<String>,
    },

    /// Insert rows into a table.
    Insert {
        table: String,
        columns: Vec<String>,
        values: Vec<Vec<Value>>,
    },

    /// Create a new table.
    CreateTable {
        name: String,
        columns: Vec<ColumnDef>,
    },
}
```

Notice that `Filter` and `Project` use `Box<Plan>` -- the same recursive pattern from Chapter 7. A plan can contain other plans, forming a tree. Without `Box`, the `Plan` type would have infinite size.

### Step 4: Read the plan tree

Let us understand how `SELECT name FROM users WHERE age > 18` becomes a plan tree:

```
         Plan tree              Data flow (during execution)

Project [name]                 ← rows with just "name" column
  └── Filter (age > 18)       ← rows where age > 18
        └── Scan users         ← ALL rows from the "users" table
```

The tree reads **bottom-up** for data flow: first scan all rows from `users`, then filter to keep rows where `age > 18`, then project to keep only the `name` column.

But the planner builds the tree **top-down** from the SQL: it sees SELECT (project), sees WHERE (filter), sees FROM (scan), and wraps each layer around the previous one.

### Step 5: Build a plan by hand and test it

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plan_construction() {
        // Build: Project [name] -> Filter (age > 18) -> Scan users

        // Start at the bottom: Scan
        let scan = Plan::Scan {
            table: "users".to_string(),
            columns: vec![
                ColumnInfo {
                    name: "id".to_string(),
                    data_type: DataType::Integer,
                },
                ColumnInfo {
                    name: "name".to_string(),
                    data_type: DataType::Text,
                },
                ColumnInfo {
                    name: "age".to_string(),
                    data_type: DataType::Integer,
                },
            ],
        };

        // Wrap it in a Filter
        let filter = Plan::Filter {
            source: Box::new(scan),
            predicate: Expression::BinaryOp {
                left: Box::new(Expression::Column("age".to_string())),
                op: Operator::Gt,
                right: Box::new(Expression::Literal(Value::Integer(18))),
            },
        };

        // Wrap it in a Project
        let project = Plan::Project {
            source: Box::new(filter),
            columns: vec!["name".to_string()],
        };

        // Verify the outermost node is Project
        match &project {
            Plan::Project { columns, source } => {
                assert_eq!(columns, &vec!["name".to_string()]);
                // The source should be a Filter
                match source.as_ref() {
                    Plan::Filter { source, .. } => {
                        // The filter's source should be a Scan
                        match source.as_ref() {
                            Plan::Scan { table, .. } => {
                                assert_eq!(table, "users");
                            }
                            _ => panic!("Expected Scan"),
                        }
                    }
                    _ => panic!("Expected Filter"),
                }
            }
            _ => panic!("Expected Project"),
        }
    }
}
```

`source.as_ref()` converts `&Box<Plan>` to `&Plan`, letting us match on the inner plan without moving it out of the Box.

> **What just happened?**
>
> We built a plan tree by nesting nodes inside each other using `Box::new()`. The Scan is the innermost node (a leaf), the Filter wraps the Scan, and the Project wraps the Filter. This nesting structure mirrors how data will flow during execution: data is produced by the innermost node and transformed by each outer node.

---

## Exercise 2: Build the Schema Catalog

**Goal:** Create a `Schema` that tracks which tables exist and what columns they have. The planner will use this to validate queries.

### Step 1: Define the schema types

```rust
// src/plan.rs (continued)
use std::collections::HashMap;

/// Metadata about a single column in a table.
#[derive(Debug, Clone)]
pub struct SchemaColumn {
    pub name: String,
    pub data_type: DataType,
}

/// Metadata about a table.
#[derive(Debug, Clone)]
pub struct SchemaTable {
    pub name: String,
    pub columns: Vec<SchemaColumn>,
}

impl SchemaTable {
    /// Check if a column exists in this table.
    pub fn has_column(&self, name: &str) -> bool {
        self.columns.iter().any(|c| c.name == name)
    }

    /// Get a column by name.
    pub fn get_column(&self, name: &str) -> Option<&SchemaColumn> {
        self.columns.iter().find(|c| c.name == name)
    }

    /// Return all column names.
    pub fn column_names(&self) -> Vec<String> {
        self.columns.iter().map(|c| c.name.clone()).collect()
    }
}
```

Here we see iterators in action:

- `.any(|c| c.name == name)` -- returns `true` if any column's name matches. The closure captures `name` from the surrounding function.
- `.find(|c| c.name == name)` -- returns `Some(&column)` for the first column that matches, or `None` if none do.
- `.map(|c| c.name.clone()).collect()` -- transforms each column into its name and collects into a Vec.

### Step 2: The Schema struct

```rust
/// The schema catalog -- knows all tables and their columns.
///
/// Think of this as the "dictionary" of the database. Before you
/// can use a word (table/column) in a sentence (query), the dictionary
/// must contain it.
#[derive(Debug, Clone)]
pub struct Schema {
    tables: HashMap<String, SchemaTable>,
}

impl Schema {
    /// Create an empty schema.
    pub fn new() -> Self {
        Schema {
            tables: HashMap::new(),
        }
    }

    /// Add a table to the schema.
    pub fn add_table(&mut self, table: SchemaTable) {
        self.tables.insert(table.name.clone(), table);
    }

    /// Look up a table by name.
    pub fn get_table(&self, name: &str) -> Option<&SchemaTable> {
        self.tables.get(name)
    }

    /// Check if a table exists.
    pub fn has_table(&self, name: &str) -> bool {
        self.tables.contains_key(name)
    }
}
```

`HashMap<String, SchemaTable>` stores our tables indexed by name, giving us O(1) lookup. When the planner encounters `FROM users`, it calls `schema.get_table("users")` to verify the table exists and get its column information.

### Step 3: Test the schema

```rust
#[cfg(test)]
mod schema_tests {
    use super::*;

    fn test_schema() -> Schema {
        let mut schema = Schema::new();
        schema.add_table(SchemaTable {
            name: "users".to_string(),
            columns: vec![
                SchemaColumn {
                    name: "id".to_string(),
                    data_type: DataType::Integer,
                },
                SchemaColumn {
                    name: "name".to_string(),
                    data_type: DataType::Text,
                },
                SchemaColumn {
                    name: "age".to_string(),
                    data_type: DataType::Integer,
                },
            ],
        });
        schema
    }

    #[test]
    fn schema_table_exists() {
        let schema = test_schema();
        assert!(schema.has_table("users"));
        assert!(!schema.has_table("unicorns"));
    }

    #[test]
    fn schema_column_lookup() {
        let schema = test_schema();
        let users = schema.get_table("users").unwrap();
        assert!(users.has_column("name"));
        assert!(!users.has_column("email"));
    }

    #[test]
    fn schema_column_names() {
        let schema = test_schema();
        let users = schema.get_table("users").unwrap();
        let names = users.column_names();
        assert_eq!(names, vec!["id", "name", "age"]);
    }
}
```

---

## Exercise 3: The Planner

**Goal:** Build a `Planner` that transforms a parsed `Statement` (AST) into a validated `Plan`.

### Step 1: Define the Planner struct

```rust
/// The query planner. Transforms an AST into a validated execution plan.
///
/// The planner does three things:
/// 1. Validates that tables and columns mentioned in the SQL actually exist
/// 2. Resolves column types from the schema
/// 3. Builds a tree of Plan nodes that describe how to execute the query
pub struct Planner<'a> {
    schema: &'a Schema,
}
```

The `<'a>` is a **lifetime parameter**. It tells Rust: "the Planner borrows a Schema, and the Schema must live at least as long as the Planner does." Think of it as a library card -- the Planner has a card that lets it read the Schema, and the Schema (the library) must stay open as long as the Planner has the card.

Why borrow instead of own? Because multiple planners might need to read the same schema, and we do not want to copy the entire schema each time. Borrowing lets us share access without copying.

> **What just happened?**
>
> Lifetimes are Rust's way of tracking how long references are valid. The `'a` in `&'a Schema` says "this reference is valid for some lifetime `'a`." The same `'a` on `Planner<'a>` says "this Planner cannot outlive the Schema it references." The compiler checks this at compile time -- if you try to use a Planner after the Schema is dropped, you get a compile error, not a runtime crash.

### Step 2: Plan a SELECT statement

```rust
impl<'a> Planner<'a> {
    /// Create a new planner with access to the schema catalog.
    pub fn new(schema: &'a Schema) -> Self {
        Planner { schema }
    }

    /// Transform a Statement (AST) into a Plan.
    pub fn plan(&self, statement: &Statement) -> Result<Plan, String> {
        use crate::parser::Statement;

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

            Statement::CreateTable { name, columns } => {
                self.plan_create_table(name, columns)
            }
        }
    }

    /// Plan a SELECT statement.
    fn plan_select(
        &self,
        columns: &[Expression],
        from: &str,
        where_clause: &Option<Expression>,
    ) -> Result<Plan, String> {
        // Step 1: Validate the table exists
        let table = self.schema.get_table(from).ok_or_else(|| {
            format!("Table '{}' does not exist", from)
        })?;

        // Step 2: Build the Scan node (bottom of the tree)
        let column_infos: Vec<ColumnInfo> = table
            .columns
            .iter()
            .map(|c| ColumnInfo {
                name: c.name.clone(),
                data_type: c.data_type.clone(),
            })
            .collect();

        let mut plan = Plan::Scan {
            table: from.to_string(),
            columns: column_infos,
        };

        // Step 3: If there is a WHERE clause, wrap in a Filter
        if let Some(predicate) = where_clause {
            // Validate columns in the predicate
            self.validate_expression(predicate, table)?;

            plan = Plan::Filter {
                source: Box::new(plan),
                predicate: predicate.clone(),
            };
        }

        // Step 4: Handle the column list
        // Check for SELECT * (all columns)
        let is_star = columns.len() == 1
            && matches!(&columns[0], Expression::Column(n) if n == "*");

        if !is_star {
            // Validate and extract column names
            let col_names: Result<Vec<String>, String> = columns
                .iter()
                .map(|expr| match expr {
                    Expression::Column(name) => {
                        if table.has_column(name) {
                            Ok(name.clone())
                        } else {
                            Err(format!(
                                "Column '{}' does not exist in table '{}'",
                                name, from
                            ))
                        }
                    }
                    _ => Ok(expr.to_string()),
                })
                .collect();

            plan = Plan::Project {
                source: Box::new(plan),
                columns: col_names?,
            };
        }

        Ok(plan)
    }
```

There is a lot happening here. Let us walk through it step by step.

**Step 1: Validate the table.** `.ok_or_else()` converts `Option` to `Result` -- if the table does not exist (None), it becomes `Err("Table 'unicorns' does not exist")`. The `?` then returns that error immediately.

**Step 2: Build the Scan.** We use an iterator chain to convert `SchemaColumn` objects into `ColumnInfo` objects. The `.map()` closure transforms each column, and `.collect()` gathers them into a Vec.

**Step 3: Optional Filter.** `if let Some(predicate) = where_clause` is Rust's way of saying "if this Option contains a value, bind it to `predicate` and run this block." If the WHERE clause is None, we skip the block entirely.

**Step 4: Column projection.** We use `.map().collect()` again, but this time we collect into a `Result<Vec<String>, String>`. If any column is invalid, the entire result becomes an `Err`, and the `?` returns the error.

> **What just happened?**
>
> The planner builds the plan tree from the inside out: first Scan (the leaf), then Filter (if there is a WHERE clause), then Project (if the user selected specific columns). Each wrapper node uses `Box::new()` to wrap the previous plan. The result is a nested tree: `Project(Filter(Scan))`.

### Step 3: Validate expressions

The planner needs to check that column names in WHERE clauses actually exist:

```rust
    /// Validate that all column references in an expression exist
    /// in the given table.
    fn validate_expression(
        &self,
        expr: &Expression,
        table: &SchemaTable,
    ) -> Result<(), String> {
        match expr {
            Expression::Column(name) => {
                if table.has_column(name) {
                    Ok(())
                } else {
                    Err(format!(
                        "Column '{}' does not exist in table '{}'",
                        name, table.name
                    ))
                }
            }
            Expression::Literal(_) => Ok(()), // Literals are always valid
            Expression::BinaryOp { left, right, .. } => {
                // Validate both sides
                self.validate_expression(left, table)?;
                self.validate_expression(right, table)?;
                Ok(())
            }
            Expression::UnaryOp { expr, .. } => {
                self.validate_expression(expr, table)
            }
        }
    }
```

This function uses **recursion** -- it calls itself on child expressions. For a `BinaryOp`, it validates the left side, then the right side. If either fails, the `?` propagates the error upward. For a `Column`, it checks the schema. For a `Literal`, there is nothing to validate.

### Step 4: Plan INSERT and CREATE TABLE

```rust
    /// Plan an INSERT statement.
    fn plan_insert(
        &self,
        table_name: &str,
        columns: &[String],
        values: &[Expression],
    ) -> Result<Plan, String> {
        // Validate table exists
        let table = self.schema.get_table(table_name).ok_or_else(|| {
            format!("Table '{}' does not exist", table_name)
        })?;

        // Validate all columns exist
        for col in columns {
            if !table.has_column(col) {
                return Err(format!(
                    "Column '{}' does not exist in table '{}'",
                    col, table_name
                ));
            }
        }

        // Evaluate literal values
        let row_values: Result<Vec<Value>, String> = values
            .iter()
            .map(|expr| match expr {
                Expression::Literal(v) => Ok(v.clone()),
                other => Err(format!(
                    "INSERT values must be literals, got: {}",
                    other
                )),
            })
            .collect();

        Ok(Plan::Insert {
            table: table_name.to_string(),
            columns: columns.to_vec(),
            values: vec![row_values?],
        })
    }

    /// Plan a CREATE TABLE statement.
    fn plan_create_table(
        &self,
        name: &str,
        columns: &[ColumnDef],
    ) -> Result<Plan, String> {
        // Check that the table does not already exist
        if self.schema.has_table(name) {
            return Err(format!("Table '{}' already exists", name));
        }

        // Check for duplicate column names
        let mut seen = std::collections::HashSet::new();
        for col in columns {
            if !seen.insert(&col.name) {
                return Err(format!(
                    "Duplicate column name: '{}'",
                    col.name
                ));
            }
        }

        Ok(Plan::CreateTable {
            name: name.to_string(),
            columns: columns.to_vec(),
        })
    }
}
```

The INSERT planner uses the same `.map().collect()` pattern to convert expressions to values, stopping at the first non-literal expression with an error.

The CREATE TABLE planner introduces `HashSet` -- a collection that stores unique values. `seen.insert()` returns `true` if the value was new, `false` if it already existed. This gives us a clean way to detect duplicate column names.

### Step 5: Test the planner

```rust
#[cfg(test)]
mod planner_tests {
    use super::*;
    use crate::parser::Parser;

    fn test_schema() -> Schema {
        let mut schema = Schema::new();
        schema.add_table(SchemaTable {
            name: "users".to_string(),
            columns: vec![
                SchemaColumn {
                    name: "id".to_string(),
                    data_type: DataType::Integer,
                },
                SchemaColumn {
                    name: "name".to_string(),
                    data_type: DataType::Text,
                },
                SchemaColumn {
                    name: "age".to_string(),
                    data_type: DataType::Integer,
                },
            ],
        });
        schema
    }

    #[test]
    fn plan_simple_select() {
        let schema = test_schema();
        let planner = Planner::new(&schema);
        let stmt = Parser::parse("SELECT name FROM users").unwrap();
        let plan = planner.plan(&stmt).unwrap();

        // Should be Project -> Scan (no WHERE means no Filter)
        match &plan {
            Plan::Project { columns, source } => {
                assert_eq!(columns, &vec!["name".to_string()]);
                match source.as_ref() {
                    Plan::Scan { table, .. } => {
                        assert_eq!(table, "users");
                    }
                    _ => panic!("Expected Scan"),
                }
            }
            _ => panic!("Expected Project"),
        }
    }

    #[test]
    fn plan_select_with_filter() {
        let schema = test_schema();
        let planner = Planner::new(&schema);
        let stmt = Parser::parse(
            "SELECT name FROM users WHERE age > 18"
        ).unwrap();
        let plan = planner.plan(&stmt).unwrap();

        // Should be Project -> Filter -> Scan
        match &plan {
            Plan::Project { source, .. } => {
                match source.as_ref() {
                    Plan::Filter { source, .. } => {
                        match source.as_ref() {
                            Plan::Scan { table, .. } => {
                                assert_eq!(table, "users");
                            }
                            _ => panic!("Expected Scan"),
                        }
                    }
                    _ => panic!("Expected Filter"),
                }
            }
            _ => panic!("Expected Project"),
        }
    }

    #[test]
    fn plan_select_star() {
        let schema = test_schema();
        let planner = Planner::new(&schema);
        let stmt = Parser::parse("SELECT * FROM users").unwrap();
        let plan = planner.plan(&stmt).unwrap();

        // SELECT * should produce just a Scan (no Project needed)
        match &plan {
            Plan::Scan { table, columns } => {
                assert_eq!(table, "users");
                assert_eq!(columns.len(), 3);
            }
            _ => panic!("Expected Scan for SELECT *"),
        }
    }

    #[test]
    fn plan_error_missing_table() {
        let schema = test_schema();
        let planner = Planner::new(&schema);
        let stmt = Parser::parse("SELECT * FROM unicorns").unwrap();
        let result = planner.plan(&stmt);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("does not exist"));
    }

    #[test]
    fn plan_error_missing_column() {
        let schema = test_schema();
        let planner = Planner::new(&schema);
        let stmt = Parser::parse(
            "SELECT email FROM users"
        ).unwrap();
        let result = planner.plan(&stmt);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("email"));
    }

    #[test]
    fn plan_create_table() {
        let schema = test_schema();
        let planner = Planner::new(&schema);
        let stmt = Parser::parse(
            "CREATE TABLE products (name TEXT, price INTEGER)"
        ).unwrap();
        let plan = planner.plan(&stmt).unwrap();

        match &plan {
            Plan::CreateTable { name, columns } => {
                assert_eq!(name, "products");
                assert_eq!(columns.len(), 2);
            }
            _ => panic!("Expected CreateTable"),
        }
    }

    #[test]
    fn plan_error_duplicate_table() {
        let schema = test_schema();
        let planner = Planner::new(&schema);
        let stmt = Parser::parse(
            "CREATE TABLE users (name TEXT)"
        ).unwrap();
        let result = planner.plan(&stmt);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("already exists"));
    }
}
```

Run the tests:

```
$ cargo test planner_tests
running 7 tests
test plan::planner_tests::plan_simple_select ... ok
test plan::planner_tests::plan_select_with_filter ... ok
test plan::planner_tests::plan_select_star ... ok
test plan::planner_tests::plan_error_missing_table ... ok
test plan::planner_tests::plan_error_missing_column ... ok
test plan::planner_tests::plan_create_table ... ok
test plan::planner_tests::plan_error_duplicate_table ... ok

test result: ok. 7 passed; 0 failed; 0 ignored
```

---

## Exercise 4: Display Plans (EXPLAIN)

**Goal:** Implement `Display` for `Plan` so we can print plans as indented trees, similar to SQL's EXPLAIN command.

### Step 1: The display implementation

When you run `EXPLAIN SELECT name FROM users WHERE age > 18` in a real database, you see the plan as an indented tree. Let us build the same thing:

```rust
impl Plan {
    /// Format the plan as an indented tree.
    /// depth controls the indentation level.
    fn format_tree(&self, f: &mut fmt::Formatter<'_>, depth: usize)
        -> fmt::Result
    {
        let indent = "  ".repeat(depth);

        match self {
            Plan::Scan { table, columns } => {
                let col_names: Vec<&str> = columns
                    .iter()
                    .map(|c| c.name.as_str())
                    .collect();
                writeln!(
                    f,
                    "{}Scan: {} [{}]",
                    indent,
                    table,
                    col_names.join(", ")
                )
            }
            Plan::Filter { source, predicate } => {
                writeln!(f, "{}Filter: {}", indent, predicate)?;
                source.format_tree(f, depth + 1)
            }
            Plan::Project { source, columns } => {
                writeln!(
                    f,
                    "{}Project: [{}]",
                    indent,
                    columns.join(", ")
                )?;
                source.format_tree(f, depth + 1)
            }
            Plan::Insert { table, columns, .. } => {
                writeln!(
                    f,
                    "{}Insert: {} [{}]",
                    indent,
                    table,
                    columns.join(", ")
                )
            }
            Plan::CreateTable { name, columns } => {
                let col_defs: Vec<String> = columns
                    .iter()
                    .map(|c| format!("{} {}", c.name, c.data_type))
                    .collect();
                writeln!(
                    f,
                    "{}CreateTable: {} ({})",
                    indent,
                    name,
                    col_defs.join(", ")
                )
            }
        }
    }
}

impl fmt::Display for Plan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.format_tree(f, 0)
    }
}
```

`"  ".repeat(depth)` creates a string of spaces for indentation. At depth 0, there are no spaces. At depth 1, two spaces. At depth 2, four spaces. This creates the visual nesting:

```
Project: [name]
  Filter: (age > 18)
    Scan: users [id, name, age]
```

The `format_tree` method uses recursion: when it encounters a `Filter` or `Project`, it prints itself, then calls `format_tree` on its source with `depth + 1` to increase the indentation.

### Step 2: Test the display

```rust
#[cfg(test)]
mod display_tests {
    use super::*;
    use crate::parser::Parser;

    fn test_schema() -> Schema {
        let mut schema = Schema::new();
        schema.add_table(SchemaTable {
            name: "users".to_string(),
            columns: vec![
                SchemaColumn {
                    name: "id".to_string(),
                    data_type: DataType::Integer,
                },
                SchemaColumn {
                    name: "name".to_string(),
                    data_type: DataType::Text,
                },
                SchemaColumn {
                    name: "age".to_string(),
                    data_type: DataType::Integer,
                },
            ],
        });
        schema
    }

    #[test]
    fn display_plan() {
        let schema = test_schema();
        let planner = Planner::new(&schema);
        let stmt = Parser::parse(
            "SELECT name FROM users WHERE age > 18"
        ).unwrap();
        let plan = planner.plan(&stmt).unwrap();
        let output = format!("{}", plan);
        println!("{}", output);

        // Verify the tree structure in the output
        assert!(output.contains("Project"));
        assert!(output.contains("Filter"));
        assert!(output.contains("Scan"));
    }
}
```

Run with `cargo test display_tests -- --nocapture` to see the printed plan.

---

## Exercise 5: Full Pipeline

**Goal:** Test the complete pipeline from SQL string to plan.

### Step 1: Integration test

```rust
#[cfg(test)]
mod pipeline_tests {
    use super::*;
    use crate::parser::Parser;

    fn test_schema() -> Schema {
        let mut schema = Schema::new();
        schema.add_table(SchemaTable {
            name: "users".to_string(),
            columns: vec![
                SchemaColumn {
                    name: "id".to_string(),
                    data_type: DataType::Integer,
                },
                SchemaColumn {
                    name: "name".to_string(),
                    data_type: DataType::Text,
                },
                SchemaColumn {
                    name: "age".to_string(),
                    data_type: DataType::Integer,
                },
            ],
        });
        schema.add_table(SchemaTable {
            name: "products".to_string(),
            columns: vec![
                SchemaColumn {
                    name: "id".to_string(),
                    data_type: DataType::Integer,
                },
                SchemaColumn {
                    name: "name".to_string(),
                    data_type: DataType::Text,
                },
                SchemaColumn {
                    name: "price".to_string(),
                    data_type: DataType::Integer,
                },
            ],
        });
        schema
    }

    /// Helper: parse SQL and plan it
    fn plan_sql(sql: &str, schema: &Schema) -> Result<Plan, String> {
        let stmt = Parser::parse(sql)?;
        let planner = Planner::new(schema);
        planner.plan(&stmt)
    }

    #[test]
    fn pipeline_select_all() {
        let schema = test_schema();
        let plan = plan_sql("SELECT * FROM users", &schema).unwrap();
        println!("{}", plan);
    }

    #[test]
    fn pipeline_select_columns() {
        let schema = test_schema();
        let plan = plan_sql(
            "SELECT name, age FROM users",
            &schema,
        ).unwrap();
        println!("{}", plan);
    }

    #[test]
    fn pipeline_select_where() {
        let schema = test_schema();
        let plan = plan_sql(
            "SELECT name FROM users WHERE age > 18",
            &schema,
        ).unwrap();
        println!("{}", plan);
    }

    #[test]
    fn pipeline_insert() {
        let schema = test_schema();
        let plan = plan_sql(
            "INSERT INTO users (name, age) VALUES ('Alice', 30)",
            &schema,
        ).unwrap();
        println!("{}", plan);
    }

    #[test]
    fn pipeline_create_table() {
        let schema = test_schema();
        let plan = plan_sql(
            "CREATE TABLE orders (id INTEGER, item TEXT)",
            &schema,
        ).unwrap();
        println!("{}", plan);
    }

    #[test]
    fn pipeline_error_nonexistent_table() {
        let schema = test_schema();
        let result = plan_sql("SELECT * FROM ghosts", &schema);
        assert!(result.is_err());
    }

    #[test]
    fn pipeline_error_nonexistent_column() {
        let schema = test_schema();
        let result = plan_sql(
            "SELECT email FROM users",
            &schema,
        );
        assert!(result.is_err());
    }
}
```

Run all tests:

```
$ cargo test
running 20+ tests
...
test result: ok. all passed
```

---

## What We Built

This chapter transformed ASTs into validated execution plans. Here is what you learned:

| Concept | What it does | Why it matters |
|---------|-------------|----------------|
| **Iterators** | Produce a sequence of values one at a time | Process collections without manual loops |
| **Iterator adapters** | Transform iterators: `.map()`, `.filter()`, `.collect()` | Chain transformations cleanly |
| **Closures** | Anonymous functions that capture surrounding variables | Customize iterator behavior |
| **Lifetimes (`'a`)** | Track how long references are valid | Share data safely without copying |
| **Option/Result with iterators** | `.collect::<Result<Vec<_>, _>>()` | Stop at first error in a list |

The plan tree is the bridge between "what the user asked" (AST) and "how to get it" (execution). In the next chapter, we will build an optimizer that rewrites plans to be more efficient.

---

## Exercises for Practice

1. **Add UPDATE planning**: Add a `plan_update()` method that handles `UPDATE users SET name = 'Bob' WHERE id = 1`. It should validate the table, columns, and WHERE clause expression.

   *Hint: The plan tree for UPDATE is `Update { source: Filter(Scan), assignments }` -- the Filter/Scan combination finds the rows to update.*

2. **Add DELETE planning**: Add a `plan_delete()` method for `DELETE FROM users WHERE age < 18`. Similar to SELECT but without the Project step.

   *Hint: DELETE needs a Filter(Scan) to find rows, just like UPDATE.*

3. **Wildcard expansion**: When you see `SELECT * FROM users`, the current planner returns a Scan without a Project. Modify it to expand `*` into the actual column names from the schema: `SELECT id, name, age FROM users`.

   *Hint: Use `table.column_names()` to get the list and replace the `*` column with the expanded list.*

4. **Column type validation in WHERE**: Currently we only check that columns exist. Add validation that comparisons make type sense -- `age > 'hello'` should produce an error because you cannot compare an integer to a string.

   *Hint: Add a `type_of()` method to Expression that returns the expected DataType by looking up columns in the schema.*

5. **Multiple tables in schema**: Write a test that creates a schema with two tables (`users` and `orders`) and verifies that `SELECT name FROM orders` correctly validates against the `orders` table (not `users`).

   *Hint: This should already work with the current implementation. The test confirms that the planner uses the correct table from the FROM clause.*
