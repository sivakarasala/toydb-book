# Tree Transformations — "From meaning to machinery"

You have a tree that says what the user wants. The parser built it: `SELECT name FROM users WHERE age > 30` became a `Statement::Select` with columns, a table name, and a filter expression. But the executor does not speak SQL. It speaks in operations: scan this table, filter with this predicate, project these columns. You need to translate one tree into another tree — same information, different shape.

This is a tree transformation. The input tree (AST) represents *what*. The output tree (Plan) represents *how*. The transformation walks every node in the input, decides what it means, and builds the corresponding output. It is the same pattern used in compilers (AST to IR), transpilers (one language to another), and code formatters (AST to pretty-printed text). Once you see the pattern, you see it everywhere.

Let's build tree transformations from scratch.

---

## The Naive Way

The simplest approach: a giant function with one branch per AST node type, building plan nodes directly with deeply nested constructors.

```rust
fn main() {
    // Imagine translating this by hand:
    // SELECT name FROM users WHERE age > 30
    //
    // You'd write something like:
    let plan = "Project(name, Filter(age > 30, Scan(users)))";
    //
    // But with 5 statement types, each with optional clauses,
    // this becomes a 200-line match statement with repeated logic.
    // Every new feature (LIMIT, ORDER BY, JOIN) adds more nesting.
    //
    // The real problem: validation is mixed with construction.
    // "Does this table exist?" lives next to "Build a Scan node."
    // Testing one means running both.

    println!("Naive: one giant function, no separation of concerns");
}
```

This works for a toy, but it tangles three distinct concerns: validating input, resolving names, and building output. When something goes wrong, you cannot tell which stage failed.

---

## The Insight

Think of a human translator working on a legal document. They do not translate word by word from left to right. They read a clause, understand its legal meaning, look up the equivalent legal term in the target language, and write the translated clause. Each clause is handled by the same process: read, understand, translate.

A tree transformation works identically. You walk the input tree, and at each node you ask: "What does this mean, and what output node corresponds to it?" The walk itself is mechanical — the interesting work happens at each node.

There are two fundamental walk orders:

```text
Input AST:          SELECT
                   /   |   \
               [name]  users  WHERE
                                |
                              age > 30

Pre-order  (top-down):  Visit parent FIRST, then children.
Post-order (bottom-up): Visit children FIRST, then parent.
```

**Top-down** is natural for our planner: you see `SELECT`, decide it needs a `Project` node, then recurse into the children to build the `Scan` and `Filter` underneath. The parent decides the shape, the children fill in the details.

**Bottom-up** is natural for optimizers: you transform the leaves first, then the intermediate nodes can inspect their already-transformed children. Constant folding works this way — fold `2 * 3` into `6` before the parent `+` node sees it.

---

## The Build

### Two Tree Types

The key to a clean transformation: the input and output are *different types*. The AST speaks in SQL concepts (SELECT, FROM, WHERE). The Plan speaks in execution concepts (Scan, Filter, Project). Keeping them separate means the planner is the only code that knows both vocabularies.

```rust
// --- Input tree: what the user said ---

#[derive(Debug, Clone)]
enum Expr {
    Column(String),
    Integer(i64),
    Gt(Box<Expr>, Box<Expr>),
    And(Box<Expr>, Box<Expr>),
}

#[derive(Debug, Clone)]
enum Statement {
    Select {
        columns: Vec<String>,
        table: String,
        where_clause: Option<Expr>,
    },
    Insert {
        table: String,
        values: Vec<Expr>,
    },
}

// --- Output tree: how to execute it ---

#[derive(Debug, Clone)]
enum Plan {
    Scan { table: String },
    Filter { predicate: Expr, source: Box<Plan> },
    Project { columns: Vec<String>, source: Box<Plan> },
    InsertPlan { table: String, values: Vec<Expr> },
}
```

### The Transformation Function

The planner walks each `Statement` variant, validates it, and builds the corresponding `Plan`. Notice how the plan is constructed inside-out: the innermost node (Scan) is built first, then wrapped in Filter, then wrapped in Project.

```rust
use std::collections::HashMap;

struct Schema {
    tables: HashMap<String, Vec<String>>, // table -> columns
}

#[derive(Debug)]
enum PlanError {
    TableNotFound(String),
    ColumnNotFound(String, String), // column, table
}

fn plan_statement(stmt: &Statement, schema: &Schema) -> Result<Plan, PlanError> {
    match stmt {
        Statement::Select { columns, table, where_clause } => {
            // Step 1: Validate the table exists
            let table_columns = schema.tables.get(table)
                .ok_or(PlanError::TableNotFound(table.clone()))?;

            // Step 2: Validate all selected columns exist
            for col in columns {
                if !table_columns.contains(col) {
                    return Err(PlanError::ColumnNotFound(
                        col.clone(), table.clone()
                    ));
                }
            }

            // Step 3: Build the plan inside-out
            //   Scan -> Filter (if WHERE exists) -> Project
            let mut plan = Plan::Scan { table: table.clone() };

            if let Some(predicate) = where_clause {
                plan = Plan::Filter {
                    predicate: predicate.clone(),
                    source: Box::new(plan),
                };
            }

            plan = Plan::Project {
                columns: columns.clone(),
                source: Box::new(plan),
            };

            Ok(plan)
        }

        Statement::Insert { table, values } => {
            if !schema.tables.contains_key(table) {
                return Err(PlanError::TableNotFound(table.clone()));
            }
            Ok(Plan::InsertPlan {
                table: table.clone(),
                values: values.clone(),
            })
        }
    }
}
```

### Displaying Plans as Trees

Production databases show plans with `EXPLAIN`. We can do the same with recursive `Display`:

```rust
fn display_plan(plan: &Plan, indent: usize) -> String {
    let pad = "  ".repeat(indent);
    match plan {
        Plan::Scan { table } => {
            format!("{}Scan: {}", pad, table)
        }
        Plan::Filter { predicate, source } => {
            format!(
                "{}Filter: {:?}\n{}",
                pad, predicate,
                display_plan(source, indent + 1)
            )
        }
        Plan::Project { columns, source } => {
            format!(
                "{}Project: [{}]\n{}",
                pad, columns.join(", "),
                display_plan(source, indent + 1)
            )
        }
        Plan::InsertPlan { table, values } => {
            format!("{}Insert into {} ({} values)", pad, table, values.len())
        }
    }
}
```

### A Generic Tree Mapper

The pattern of "walk a tree and transform each node" is so common that we can generalize it. Here is a function that transforms any `Plan` tree using a closure:

```rust
fn map_plan<F>(plan: Plan, f: &F) -> Plan
where
    F: Fn(Plan) -> Plan,
{
    // First, recursively transform children (bottom-up)
    let transformed = match plan {
        Plan::Filter { predicate, source } => Plan::Filter {
            predicate,
            source: Box::new(map_plan(*source, f)),
        },
        Plan::Project { columns, source } => Plan::Project {
            columns,
            source: Box::new(map_plan(*source, f)),
        },
        // Leaf nodes have no children
        other => other,
    };

    // Then apply the transformation to the current node
    f(transformed)
}
```

This is powerful. Any optimization rule becomes a closure:

```rust
fn remove_trivial_projects(plan: Plan) -> Plan {
    map_plan(plan, &|node| {
        match node {
            // A project that selects all columns is a no-op
            Plan::Project { columns, source } if columns.is_empty() => *source,
            other => other,
        }
    })
}
```

---

## The Payoff

Here is the full, runnable implementation:

```rust
use std::collections::HashMap;

#[derive(Debug, Clone)]
enum Expr {
    Column(String),
    Integer(i64),
    Gt(Box<Expr>, Box<Expr>),
    And(Box<Expr>, Box<Expr>),
}

#[derive(Debug, Clone)]
enum Statement {
    Select {
        columns: Vec<String>,
        table: String,
        where_clause: Option<Expr>,
    },
}

#[derive(Debug, Clone)]
enum Plan {
    Scan { table: String },
    Filter { predicate: Expr, source: Box<Plan> },
    Project { columns: Vec<String>, source: Box<Plan> },
}

#[derive(Debug)]
enum PlanError {
    TableNotFound(String),
    ColumnNotFound(String, String),
}

struct Schema {
    tables: HashMap<String, Vec<String>>,
}

fn plan_statement(stmt: &Statement, schema: &Schema) -> Result<Plan, PlanError> {
    match stmt {
        Statement::Select { columns, table, where_clause } => {
            let table_columns = schema.tables.get(table)
                .ok_or(PlanError::TableNotFound(table.clone()))?;

            for col in columns {
                if !table_columns.contains(col) {
                    return Err(PlanError::ColumnNotFound(col.clone(), table.clone()));
                }
            }

            let mut plan = Plan::Scan { table: table.clone() };

            if let Some(predicate) = where_clause {
                plan = Plan::Filter {
                    predicate: predicate.clone(),
                    source: Box::new(plan),
                };
            }

            plan = Plan::Project {
                columns: columns.clone(),
                source: Box::new(plan),
            };

            Ok(plan)
        }
    }
}

fn display_plan(plan: &Plan, indent: usize) -> String {
    let pad = "  ".repeat(indent);
    match plan {
        Plan::Scan { table } => format!("{}Scan: {}", pad, table),
        Plan::Filter { predicate, source } => format!(
            "{}Filter: {:?}\n{}", pad, predicate, display_plan(source, indent + 1)
        ),
        Plan::Project { columns, source } => format!(
            "{}Project: [{}]\n{}", pad, columns.join(", "), display_plan(source, indent + 1)
        ),
    }
}

fn map_plan<F>(plan: Plan, f: &F) -> Plan
where F: Fn(Plan) -> Plan {
    let transformed = match plan {
        Plan::Filter { predicate, source } => Plan::Filter {
            predicate,
            source: Box::new(map_plan(*source, f)),
        },
        Plan::Project { columns, source } => Plan::Project {
            columns,
            source: Box::new(map_plan(*source, f)),
        },
        other => other,
    };
    f(transformed)
}

// Optimization: push filters below projections
fn push_filters_down(plan: Plan) -> Plan {
    map_plan(plan, &|node| {
        match node {
            // Project above Filter — swap them so filter runs first
            Plan::Project { columns, source } => {
                match *source {
                    Plan::Filter { predicate, source: inner } => {
                        Plan::Filter {
                            predicate,
                            source: Box::new(Plan::Project {
                                columns,
                                source: inner,
                            }),
                        }
                    }
                    other => Plan::Project { columns, source: Box::new(other) },
                }
            }
            other => other,
        }
    })
}

fn main() {
    let mut tables = HashMap::new();
    tables.insert("users".to_string(), vec![
        "name".to_string(), "age".to_string(), "email".to_string(),
    ]);
    let schema = Schema { tables };

    // SELECT name FROM users WHERE age > 30
    let stmt = Statement::Select {
        columns: vec!["name".to_string()],
        table: "users".to_string(),
        where_clause: Some(Expr::Gt(
            Box::new(Expr::Column("age".to_string())),
            Box::new(Expr::Integer(30)),
        )),
    };

    let plan = plan_statement(&stmt, &schema).unwrap();
    println!("=== Original Plan ===");
    println!("{}\n", display_plan(&plan, 0));
    // Project: [name]
    //   Filter: Gt(Column("age"), Integer(30))
    //     Scan: users

    let optimized = push_filters_down(plan);
    println!("=== After Filter Pushdown ===");
    println!("{}\n", display_plan(&optimized, 0));
    // Filter: Gt(Column("age"), Integer(30))
    //   Project: [name]
    //     Scan: users

    // Error case: table not found
    let bad_stmt = Statement::Select {
        columns: vec!["id".to_string()],
        table: "orders".to_string(),
        where_clause: None,
    };
    match plan_statement(&bad_stmt, &schema) {
        Err(e) => println!("Expected error: {:?}", e),
        Ok(_) => println!("Should have failed!"),
    }
}
```

The planner transforms an AST tree into a Plan tree. The generic `map_plan` function lets you write optimizations as simple closures. The display function makes plans inspectable. This is the architecture of every query planner from SQLite to Snowflake.

---

## Complexity Table

| Operation | Time | Space | Notes |
|-----------|------|-------|-------|
| AST to Plan (no JOIN) | O(n) | O(n) | One pass over the AST, one Plan node per AST node |
| Schema validation | O(c * t) | O(1) | c = columns checked, t = columns in table |
| Display plan | O(n) | O(d) stack | d = tree depth |
| `map_plan` (single rule) | O(n) | O(d) stack | Visits every node once |
| k optimization rules | O(k * n) | O(d) stack | Each rule is a full tree walk |
| Filter pushdown | O(n) | O(d) stack | Single bottom-up pass |

The key insight: tree transformations are always O(n) per pass — you visit each node once. The cost multiplier is the number of passes (optimization rules). Production databases run 10-20 rules, which is why optimization is fast relative to actual query execution.

---

## Where This Shows Up in Our Database

In Chapter 8, the planner transforms parsed AST `Statement`s into `Plan` trees:

```text
SQL string
  → Lexer  → tokens
  → Parser → AST (Statement)
  → Planner → Plan tree        ← you are here
  → Optimizer → optimized Plan
  → Executor → results
```

The same tree-to-tree pattern appears throughout:
- **Compilers**: AST → HIR (high-level IR) → MIR (mid-level IR) → machine code. Each step is a tree transformation. Rust's compiler has at least four IR stages.
- **Transpilers**: TypeScript AST → JavaScript AST. Babel plugins are tree transformations.
- **Linters**: Walk the AST, flag nodes that match a bad pattern. ESLint rules are tree visitors.
- **Code formatters**: Walk the AST, emit formatted text. `rustfmt` and Prettier work this way.

The separation of AST and Plan is an instance of a deeper principle: **separate the "what" from the "how."** The AST is a faithful representation of what the user wrote. The Plan is a faithful representation of what the machine will do. The transformation between them is where all the intelligence lives — validation, optimization, and strategy.

---

## Try It Yourself

### Exercise 1: Plan Node Counter

Write a function `count_nodes(plan: &Plan) -> usize` that counts the total number of nodes in a plan tree. Test with the plan from `SELECT name FROM users WHERE age > 30` (should be 3: Project, Filter, Scan).

<details>
<summary>Solution</summary>

```rust
fn count_nodes(plan: &Plan) -> usize {
    match plan {
        Plan::Scan { .. } => 1,
        Plan::Filter { source, .. } => 1 + count_nodes(source),
        Plan::Project { source, .. } => 1 + count_nodes(source),
    }
}

fn main() {
    // Build: Project -> Filter -> Scan
    let plan = Plan::Project {
        columns: vec!["name".to_string()],
        source: Box::new(Plan::Filter {
            predicate: Expr::Gt(
                Box::new(Expr::Column("age".to_string())),
                Box::new(Expr::Integer(30)),
            ),
            source: Box::new(Plan::Scan { table: "users".to_string() }),
        }),
    };

    assert_eq!(count_nodes(&plan), 3);
    println!("Node count: {} (expected 3)", count_nodes(&plan));
}
```

</details>

### Exercise 2: Collect Table Names

Write a function `collect_tables(plan: &Plan) -> Vec<String>` that extracts all table names from Scan nodes in a plan tree. This is useful for determining which tables a query touches (for locking or caching). Test with a plan that joins two tables.

<details>
<summary>Solution</summary>

```rust
fn collect_tables(plan: &Plan) -> Vec<String> {
    let mut tables = Vec::new();
    collect_tables_inner(plan, &mut tables);
    tables
}

fn collect_tables_inner(plan: &Plan, out: &mut Vec<String>) {
    match plan {
        Plan::Scan { table } => {
            if !out.contains(table) {
                out.push(table.clone());
            }
        }
        Plan::Filter { source, .. } | Plan::Project { source, .. } => {
            collect_tables_inner(source, out);
        }
    }
}

fn main() {
    let plan = Plan::Project {
        columns: vec!["name".to_string()],
        source: Box::new(Plan::Filter {
            predicate: Expr::Column("active".to_string()),
            source: Box::new(Plan::Scan { table: "users".to_string() }),
        }),
    };

    let tables = collect_tables(&plan);
    assert_eq!(tables, vec!["users"]);
    println!("Tables: {:?}", tables);
}
```

</details>

### Exercise 3: Remove Redundant Filters

Write a transformation that removes `Filter` nodes whose predicate is a literal `true` (always passes). Use `map_plan` to implement it. For example, `Filter(true, Scan(users))` should become just `Scan(users)`.

<details>
<summary>Solution</summary>

```rust
fn remove_true_filters(plan: Plan) -> Plan {
    map_plan(plan, &|node| {
        match node {
            Plan::Filter { predicate, source } => {
                // If the predicate is literally "true", skip the filter
                match &predicate {
                    Expr::Integer(1) => *source, // treating 1 as true
                    _ => Plan::Filter { predicate, source },
                }
            }
            other => other,
        }
    })
}

fn main() {
    // Filter(always-true, Scan(users)) → Scan(users)
    let plan = Plan::Filter {
        predicate: Expr::Integer(1),
        source: Box::new(Plan::Scan { table: "users".to_string() }),
    };

    let optimized = remove_true_filters(plan);
    println!("{}", display_plan(&optimized, 0));
    // Should print just: Scan: users
}
```

</details>

---

## Recap

A tree transformation converts one tree type into another. The planner transforms an AST (what the user said) into a Plan (what the machine will do). The walk is mechanical — visit every node, build the corresponding output. Top-down walks are natural for construction (parent decides the shape). Bottom-up walks are natural for optimization (children are transformed first). A generic `map_plan` function lets you write optimization rules as closures. This same pattern — tree in, tree out — is the backbone of every compiler, transpiler, linter, and query planner ever built.
