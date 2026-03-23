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
