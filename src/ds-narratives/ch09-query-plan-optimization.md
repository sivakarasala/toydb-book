# Query Plan Optimization — "Rewriting the recipe before cooking"

A chef receives an order: grilled salmon with roasted vegetables and garlic sauce. The naive approach: chop ALL vegetables, roast ALL vegetables, make the sauce, grill the salmon, then plate everything. But the oven takes 30 minutes and the salmon takes 8. A smart chef starts the vegetables first, preps the sauce while they roast, and times the salmon to finish last. Same ingredients, same result, vastly different total time.

Query optimization is the same idea. Your database receives `SELECT name FROM users WHERE age > 30 AND active = true ORDER BY name`. The naive plan: scan every row, check both conditions, sort the results, then pick only the `name` column. But what if there is an index on `active`? What if we project away unnecessary columns before sorting, so the sort operates on smaller rows? What if we push the filter down closer to the data source?

A query optimizer rewrites the plan tree before execution, applying transformations that produce the same result faster. Let's build one.

---

## The Naive Way

Without optimization, the planner builds a plan exactly mirroring the SQL structure:

```rust
fn main() {
    // SQL: SELECT name FROM users WHERE age > 30 AND active = true
    //
    // Naive plan (bottom to top):
    //   1. Scan ALL rows from 'users' table
    //   2. Filter: age > 30 AND active = true
    //   3. Project: keep only 'name' column
    //
    // Problems:
    // - We read ALL columns from disk, then throw most away in projection
    // - We read ALL rows, then throw most away in filtering
    // - If there's an index on 'active', we completely ignore it

    println!("Naive plan reads 100% of the data, uses 10% of it.");
    println!("An optimized plan reads only what's needed.");
}
```

The naive plan works, but it does maximum work. Every optimization we apply reduces the amount of data flowing through the pipeline. Filter pushdown eliminates rows early. Projection pushdown eliminates columns early. Constant folding eliminates unnecessary computation entirely.

---

## The Insight

Think of a plan as a tree of operations. Data flows from the leaves (table scans) up through intermediate nodes (filters, joins, sorts) to the root (the final result). Each node transforms the data stream in some way.

Optimization is tree surgery. You rearrange nodes, remove redundant ones, and sometimes replace entire subtrees with more efficient alternatives. The key insight: **you can rearrange the tree as long as the final result is identical**. A filter that happens after a sort produces the same rows as a filter before the sort -- but filtering first means the sort handles fewer rows.

The three fundamental optimizations:

1. **Constant folding**: Replace expressions like `2 + 3` with `5` at plan time. Why compute it for every row?
2. **Filter pushdown**: Move filter nodes as close to the data source as possible. Fewer rows flow through the rest of the plan.
3. **Projection pushdown**: Only read the columns you actually need. Narrower rows mean less memory and faster operations.

---

## The Build

### Plan Nodes

First, define the plan tree. Each node represents an operation:

```rust
#[derive(Debug, Clone)]
enum PlanNode {
    Scan {
        table: String,
        columns: Vec<String>,  // empty = all columns
    },
    Filter {
        predicate: Expr,
        input: Box<PlanNode>,
    },
    Project {
        columns: Vec<String>,
        input: Box<PlanNode>,
    },
    Sort {
        order_by: Vec<String>,
        input: Box<PlanNode>,
    },
    Limit {
        count: usize,
        input: Box<PlanNode>,
    },
}
```

### Expressions

Expressions appear in filter predicates and projections:

```rust
#[derive(Debug, Clone)]
enum Expr {
    Column(String),
    Integer(i64),
    Str(String),
    Boolean(bool),
    BinaryOp {
        op: Op,
        left: Box<Expr>,
        right: Box<Expr>,
    },
}

#[derive(Debug, Clone, PartialEq)]
enum Op {
    Eq, NotEq, Lt, Gt, LtEq, GtEq,
    Add, Sub, Mul, Div,
    And, Or,
}
```

### Plan Pretty-Printing

Before we optimize, we need to visualize plans:

```rust
fn print_plan(node: &PlanNode, indent: usize) -> String {
    let pad = "  ".repeat(indent);
    match node {
        PlanNode::Scan { table, columns } => {
            if columns.is_empty() {
                format!("{}Scan({}.*)", pad, table)
            } else {
                format!("{}Scan({}: [{}])", pad, table, columns.join(", "))
            }
        }
        PlanNode::Filter { predicate, input } => {
            format!("{}Filter({:?})\n{}",
                pad, predicate, print_plan(input, indent + 1))
        }
        PlanNode::Project { columns, input } => {
            format!("{}Project([{}])\n{}",
                pad, columns.join(", "), print_plan(input, indent + 1))
        }
        PlanNode::Sort { order_by, input } => {
            format!("{}Sort([{}])\n{}",
                pad, order_by.join(", "), print_plan(input, indent + 1))
        }
        PlanNode::Limit { count, input } => {
            format!("{}Limit({})\n{}",
                pad, count, print_plan(input, indent + 1))
        }
    }
}
```

### Optimization 1: Constant Folding

Replace constant expressions with their computed values:

```rust
fn fold_constants(expr: Expr) -> Expr {
    match expr {
        Expr::BinaryOp { op, left, right } => {
            let left = fold_constants(*left);
            let right = fold_constants(*right);

            // Fold integer arithmetic
            if let (Expr::Integer(a), Expr::Integer(b)) = (&left, &right) {
                let result = match op {
                    Op::Add => Some(a + b),
                    Op::Sub => Some(a - b),
                    Op::Mul => Some(a * b),
                    Op::Div => if *b != 0 { Some(a / b) } else { None },
                    _ => None,
                };
                if let Some(val) = result {
                    return Expr::Integer(val);
                }

                // Fold comparisons
                let bool_result = match op {
                    Op::Eq => Some(a == b),
                    Op::NotEq => Some(a != b),
                    Op::Lt => Some(a < b),
                    Op::Gt => Some(a > b),
                    Op::LtEq => Some(a <= b),
                    Op::GtEq => Some(a >= b),
                    _ => None,
                };
                if let Some(val) = bool_result {
                    return Expr::Boolean(val);
                }
            }

            // Fold boolean logic
            if let (Expr::Boolean(a), Expr::Boolean(b)) = (&left, &right) {
                match op {
                    Op::And => return Expr::Boolean(*a && *b),
                    Op::Or => return Expr::Boolean(*a || *b),
                    _ => {}
                }
            }

            // Short-circuit: AND with false, OR with true
            match (&op, &left, &right) {
                (Op::And, Expr::Boolean(false), _) => return Expr::Boolean(false),
                (Op::And, _, Expr::Boolean(false)) => return Expr::Boolean(false),
                (Op::And, Expr::Boolean(true), other) => return other.clone(),
                (Op::And, other, Expr::Boolean(true)) => return other.clone(),
                (Op::Or, Expr::Boolean(true), _) => return Expr::Boolean(true),
                (Op::Or, _, Expr::Boolean(true)) => return Expr::Boolean(true),
                (Op::Or, Expr::Boolean(false), other) => return other.clone(),
                (Op::Or, other, Expr::Boolean(false)) => return other.clone(),
                _ => {}
            }

            Expr::BinaryOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
            }
        }
        other => other,
    }
}
```

The short-circuit cases are powerful. `WHERE true AND age > 30` simplifies to `WHERE age > 30`. `WHERE false AND anything` simplifies to `WHERE false`, and a later optimization pass can eliminate the entire filter and replace the scan with an empty result.

### Optimization 2: Filter Pushdown

Move filters below sorts and projections -- closer to the data source:

```rust
fn push_filters_down(node: PlanNode) -> PlanNode {
    match node {
        // Filter above Sort: push filter below sort
        PlanNode::Filter {
            predicate,
            input,
        } => {
            let input = *input;
            match input {
                PlanNode::Sort { order_by, input: sort_input } => {
                    // Push filter below sort
                    let new_filter = PlanNode::Filter {
                        predicate,
                        input: sort_input,
                    };
                    let optimized_filter = push_filters_down(new_filter);
                    PlanNode::Sort {
                        order_by,
                        input: Box::new(optimized_filter),
                    }
                }
                PlanNode::Project { columns, input: proj_input } => {
                    // Push filter below project (if filter only uses projected columns)
                    let filter_cols = collect_columns(&predicate);
                    let all_available = filter_cols.iter().all(|c| columns.contains(c));

                    if all_available {
                        let new_filter = PlanNode::Filter {
                            predicate,
                            input: proj_input,
                        };
                        let optimized = push_filters_down(new_filter);
                        PlanNode::Project {
                            columns,
                            input: Box::new(optimized),
                        }
                    } else {
                        // Cannot push down -- filter uses columns not in projection
                        PlanNode::Filter {
                            predicate,
                            input: Box::new(PlanNode::Project {
                                columns,
                                input: proj_input,
                            }),
                        }
                    }
                }
                other => {
                    PlanNode::Filter {
                        predicate,
                        input: Box::new(push_filters_down(other)),
                    }
                }
            }
        }

        // Recurse into other node types
        PlanNode::Sort { order_by, input } => {
            PlanNode::Sort {
                order_by,
                input: Box::new(push_filters_down(*input)),
            }
        }
        PlanNode::Project { columns, input } => {
            PlanNode::Project {
                columns,
                input: Box::new(push_filters_down(*input)),
            }
        }
        PlanNode::Limit { count, input } => {
            PlanNode::Limit {
                count,
                input: Box::new(push_filters_down(*input)),
            }
        }
        other => other,
    }
}

fn collect_columns(expr: &Expr) -> Vec<String> {
    match expr {
        Expr::Column(name) => vec![name.clone()],
        Expr::BinaryOp { left, right, .. } => {
            let mut cols = collect_columns(left);
            cols.extend(collect_columns(right));
            cols
        }
        _ => vec![],
    }
}
```

### Optimization 3: Projection Pushdown

Push column projections down into scans so we only read needed columns from disk:

```rust
fn push_projections_down(node: PlanNode) -> PlanNode {
    match node {
        PlanNode::Project { columns, input } => {
            let input = push_projections_down(*input);
            match input {
                PlanNode::Scan { table, columns: _ } => {
                    // Push projection into scan
                    PlanNode::Scan {
                        table,
                        columns: columns.clone(),
                    }
                }
                PlanNode::Filter { predicate, input: filter_input } => {
                    // Need columns from both projection and filter
                    let filter_cols = collect_columns(&predicate);
                    let mut needed: Vec<String> = columns.clone();
                    for c in &filter_cols {
                        if !needed.contains(c) {
                            needed.push(c.clone());
                        }
                    }

                    let optimized_input = push_projections_down(
                        PlanNode::Project {
                            columns: needed,
                            input: filter_input,
                        }
                    );

                    // If projection added extra columns for the filter,
                    // we need a final project to trim them
                    if filter_cols.iter().any(|c| !columns.contains(c)) {
                        PlanNode::Project {
                            columns,
                            input: Box::new(PlanNode::Filter {
                                predicate,
                                input: Box::new(optimized_input),
                            }),
                        }
                    } else {
                        PlanNode::Filter {
                            predicate,
                            input: Box::new(optimized_input),
                        }
                    }
                }
                other => PlanNode::Project {
                    columns,
                    input: Box::new(other),
                },
            }
        }
        PlanNode::Filter { predicate, input } => {
            PlanNode::Filter {
                predicate,
                input: Box::new(push_projections_down(*input)),
            }
        }
        PlanNode::Sort { order_by, input } => {
            PlanNode::Sort {
                order_by,
                input: Box::new(push_projections_down(*input)),
            }
        }
        other => other,
    }
}
```

### The Optimization Pipeline

Chain the optimizations together. Run each one on the output of the previous:

```rust
fn optimize(plan: PlanNode) -> PlanNode {
    let plan = fold_plan_constants(plan);
    let plan = push_filters_down(plan);
    let plan = push_projections_down(plan);
    plan
}

fn fold_plan_constants(node: PlanNode) -> PlanNode {
    match node {
        PlanNode::Filter { predicate, input } => {
            let folded = fold_constants(predicate);
            let input = fold_plan_constants(*input);
            // If predicate folded to true, eliminate the filter entirely
            match folded {
                Expr::Boolean(true) => input,
                _ => PlanNode::Filter {
                    predicate: folded,
                    input: Box::new(input),
                },
            }
        }
        PlanNode::Sort { order_by, input } => {
            PlanNode::Sort {
                order_by,
                input: Box::new(fold_plan_constants(*input)),
            }
        }
        PlanNode::Project { columns, input } => {
            PlanNode::Project {
                columns,
                input: Box::new(fold_plan_constants(*input)),
            }
        }
        PlanNode::Limit { count, input } => {
            PlanNode::Limit {
                count,
                input: Box::new(fold_plan_constants(*input)),
            }
        }
        other => other,
    }
}
```

---

## The Payoff

Here is the full, runnable optimizer:

```rust
#[derive(Debug, Clone)]
enum Op { Eq, NotEq, Lt, Gt, LtEq, GtEq, Add, Sub, Mul, Div, And, Or }

#[derive(Debug, Clone)]
enum Expr {
    Column(String), Integer(i64), Str(String), Boolean(bool),
    BinaryOp { op: Op, left: Box<Expr>, right: Box<Expr> },
}

#[derive(Debug, Clone)]
enum PlanNode {
    Scan { table: String, columns: Vec<String> },
    Filter { predicate: Expr, input: Box<PlanNode> },
    Project { columns: Vec<String>, input: Box<PlanNode> },
    Sort { order_by: Vec<String>, input: Box<PlanNode> },
    Limit { count: usize, input: Box<PlanNode> },
}

fn print_plan(n: &PlanNode, d: usize) -> String {
    let p = "  ".repeat(d);
    match n {
        PlanNode::Scan { table, columns } => {
            if columns.is_empty() { format!("{}Scan({}.*)", p, table) }
            else { format!("{}Scan({}: [{}])", p, table, columns.join(", ")) }
        }
        PlanNode::Filter { predicate, input } =>
            format!("{}Filter({:?})\n{}", p, predicate, print_plan(input, d+1)),
        PlanNode::Project { columns, input } =>
            format!("{}Project([{}])\n{}", p, columns.join(", "), print_plan(input, d+1)),
        PlanNode::Sort { order_by, input } =>
            format!("{}Sort([{}])\n{}", p, order_by.join(", "), print_plan(input, d+1)),
        PlanNode::Limit { count, input } =>
            format!("{}Limit({})\n{}", p, count, print_plan(input, d+1)),
    }
}

fn collect_cols(e: &Expr) -> Vec<String> {
    match e {
        Expr::Column(n) => vec![n.clone()],
        Expr::BinaryOp { left, right, .. } => {
            let mut c = collect_cols(left); c.extend(collect_cols(right)); c
        }
        _ => vec![],
    }
}

fn fold_expr(e: Expr) -> Expr {
    match e {
        Expr::BinaryOp { op, left, right } => {
            let l = fold_expr(*left); let r = fold_expr(*right);
            if let (Expr::Integer(a), Expr::Integer(b)) = (&l, &r) {
                match op {
                    Op::Add => return Expr::Integer(a + b),
                    Op::Sub => return Expr::Integer(a - b),
                    Op::Mul => return Expr::Integer(a * b),
                    Op::Eq => return Expr::Boolean(a == b),
                    Op::Gt => return Expr::Boolean(a > b),
                    Op::Lt => return Expr::Boolean(a < b),
                    _ => {}
                }
            }
            match (&op, &l, &r) {
                (Op::And, Expr::Boolean(false), _) | (Op::And, _, Expr::Boolean(false))
                    => return Expr::Boolean(false),
                (Op::And, Expr::Boolean(true), other) | (Op::And, other, Expr::Boolean(true))
                    => return other.clone(),
                (Op::Or, Expr::Boolean(true), _) | (Op::Or, _, Expr::Boolean(true))
                    => return Expr::Boolean(true),
                (Op::Or, Expr::Boolean(false), other) | (Op::Or, other, Expr::Boolean(false))
                    => return other.clone(),
                _ => {}
            }
            Expr::BinaryOp { op, left: Box::new(l), right: Box::new(r) }
        }
        other => other,
    }
}

fn fold_plan(n: PlanNode) -> PlanNode {
    match n {
        PlanNode::Filter { predicate, input } => {
            let f = fold_expr(predicate);
            let i = fold_plan(*input);
            match f {
                Expr::Boolean(true) => i,
                _ => PlanNode::Filter { predicate: f, input: Box::new(i) },
            }
        }
        PlanNode::Sort { order_by, input } =>
            PlanNode::Sort { order_by, input: Box::new(fold_plan(*input)) },
        PlanNode::Project { columns, input } =>
            PlanNode::Project { columns, input: Box::new(fold_plan(*input)) },
        PlanNode::Limit { count, input } =>
            PlanNode::Limit { count, input: Box::new(fold_plan(*input)) },
        other => other,
    }
}

fn push_filters(n: PlanNode) -> PlanNode {
    match n {
        PlanNode::Filter { predicate, input } => {
            match *input {
                PlanNode::Sort { order_by, input: si } => {
                    let f = push_filters(PlanNode::Filter { predicate, input: si });
                    PlanNode::Sort { order_by, input: Box::new(f) }
                }
                other => PlanNode::Filter {
                    predicate, input: Box::new(push_filters(other))
                },
            }
        }
        PlanNode::Sort { order_by, input } =>
            PlanNode::Sort { order_by, input: Box::new(push_filters(*input)) },
        PlanNode::Project { columns, input } =>
            PlanNode::Project { columns, input: Box::new(push_filters(*input)) },
        PlanNode::Limit { count, input } =>
            PlanNode::Limit { count, input: Box::new(push_filters(*input)) },
        other => other,
    }
}

fn push_projections(n: PlanNode) -> PlanNode {
    match n {
        PlanNode::Project { columns, input } => {
            match push_projections(*input) {
                PlanNode::Scan { table, .. } =>
                    PlanNode::Scan { table, columns },
                other => PlanNode::Project { columns, input: Box::new(other) },
            }
        }
        PlanNode::Filter { predicate, input } =>
            PlanNode::Filter { predicate, input: Box::new(push_projections(*input)) },
        PlanNode::Sort { order_by, input } =>
            PlanNode::Sort { order_by, input: Box::new(push_projections(*input)) },
        other => other,
    }
}

fn optimize(plan: PlanNode) -> PlanNode {
    let plan = fold_plan(plan);
    let plan = push_filters(plan);
    let plan = push_projections(plan);
    plan
}

fn main() {
    // Example 1: Filter pushdown past sort
    println!("=== Example 1: Filter Pushdown ===");
    let plan = PlanNode::Project {
        columns: vec!["name".into()],
        input: Box::new(PlanNode::Filter {
            predicate: Expr::BinaryOp {
                op: Op::Gt,
                left: Box::new(Expr::Column("age".into())),
                right: Box::new(Expr::Integer(30)),
            },
            input: Box::new(PlanNode::Sort {
                order_by: vec!["name".into()],
                input: Box::new(PlanNode::Scan {
                    table: "users".into(),
                    columns: vec![],
                }),
            }),
        }),
    };

    println!("BEFORE:\n{}", print_plan(&plan, 0));
    let optimized = optimize(plan);
    println!("AFTER:\n{}", print_plan(&optimized, 0));

    // Example 2: Constant folding eliminates filter
    println!("=== Example 2: Constant Folding ===");
    let plan2 = PlanNode::Filter {
        predicate: Expr::BinaryOp {
            op: Op::And,
            left: Box::new(Expr::Boolean(true)),
            right: Box::new(Expr::BinaryOp {
                op: Op::Gt,
                left: Box::new(Expr::Column("age".into())),
                right: Box::new(Expr::BinaryOp {
                    op: Op::Add,
                    left: Box::new(Expr::Integer(10)),
                    right: Box::new(Expr::Integer(20)),
                }),
            }),
        },
        input: Box::new(PlanNode::Scan {
            table: "users".into(),
            columns: vec![],
        }),
    };

    println!("BEFORE:\n{}", print_plan(&plan2, 0));
    let optimized2 = optimize(plan2);
    println!("AFTER:\n{}", print_plan(&optimized2, 0));
    // "true AND age > (10 + 20)" folds to "age > 30"

    // Example 3: Projection pushdown into scan
    println!("=== Example 3: Projection Pushdown ===");
    let plan3 = PlanNode::Project {
        columns: vec!["name".into(), "email".into()],
        input: Box::new(PlanNode::Scan {
            table: "users".into(),
            columns: vec![], // reads all columns
        }),
    };

    println!("BEFORE:\n{}", print_plan(&plan3, 0));
    let optimized3 = optimize(plan3);
    println!("AFTER:\n{}", print_plan(&optimized3, 0));
    // Project + Scan(*) becomes Scan(name, email)
}
```

Each optimization pass transforms the plan tree. Filter pushdown moves the filter below the sort, so fewer rows get sorted. Constant folding simplifies `10 + 20` to `30` at plan time. Projection pushdown tells the scan to read only `name` and `email` instead of all columns. The final plan does the same work with less data.

---

## Complexity Table

| Operation | Time | Space | Notes |
|-----------|------|-------|-------|
| Constant folding (per expr) | O(n) | O(d) | n = expr nodes, d = depth |
| Filter pushdown (per plan) | O(p) | O(p) | p = plan nodes |
| Projection pushdown (per plan) | O(p) | O(p) | p = plan nodes |
| Full optimization pipeline | O(p * n) | O(p) | One pass per optimization rule |
| Column collection from expr | O(n) | O(c) | c = distinct columns |
| Plan printing | O(p) | O(p * d) | String building at each level |

The optimizer itself is fast -- it runs in time proportional to the plan size. The value comes from the execution speedup. Pushing a filter below a sort that processes 1 million rows might eliminate 990,000 of them. The optimizer takes microseconds; the execution savings are milliseconds or seconds.

---

## Where This Shows Up in Our Database

In Chapter 9, we build the query planner and optimizer:

```rust,ignore
// The optimization pipeline:
// 1. Parse SQL -> AST
// 2. Build naive plan from AST
// 3. Apply optimization rules
// 4. Execute optimized plan

pub fn plan_and_optimize(ast: Statement) -> PlanNode {
    let naive_plan = build_plan(ast);
    let optimized = optimize(naive_plan);
    optimized
}
```

Query optimization is a deep field in database research:
- **PostgreSQL** uses a cost-based optimizer that estimates the number of rows at each plan node and chooses the cheapest plan
- **MySQL** uses a rule-based optimizer for simple queries and cost-based for complex ones
- **SQLite** uses a "next generation query planner" (NGQP) that explores alternative join orders
- **CockroachDB** uses the Cascades framework for cost-based optimization with memo tables

Our rule-based optimizer is the simplest form. Production databases add cost estimation, statistics about table sizes and value distributions, and alternative plan enumeration. But the foundation is the same: rewrite the plan tree before execution.

---

## Try It Yourself

### Exercise 1: Filter Splitting

When a filter predicate is an AND of two conditions (`age > 30 AND active = true`), split it into two separate filter nodes. This enables each condition to be pushed down independently -- one might go below a join while the other cannot.

<details>
<summary>Solution</summary>

```rust
#[derive(Debug, Clone)]
enum Op { Gt, Eq, And }

#[derive(Debug, Clone)]
enum Expr {
    Column(String), Integer(i64), Boolean(bool),
    BinaryOp { op: Op, left: Box<Expr>, right: Box<Expr> },
}

#[derive(Debug, Clone)]
enum Plan {
    Scan { table: String },
    Filter { pred: Expr, input: Box<Plan> },
}

fn split_and(expr: &Expr) -> Vec<Expr> {
    match expr {
        Expr::BinaryOp { op: Op::And, left, right } => {
            let mut parts = split_and(left);
            parts.extend(split_and(right));
            parts
        }
        other => vec![other.clone()],
    }
}

fn split_filters(plan: Plan) -> Plan {
    match plan {
        Plan::Filter { pred, input } => {
            let input = split_filters(*input);
            let conditions = split_and(&pred);

            // Stack individual filters, innermost first
            let mut result = input;
            for cond in conditions {
                result = Plan::Filter {
                    pred: cond,
                    input: Box::new(result),
                };
            }
            result
        }
        Plan::Scan { table } => Plan::Scan { table },
    }
}

fn print_plan(p: &Plan, d: usize) -> String {
    let pad = "  ".repeat(d);
    match p {
        Plan::Scan { table } => format!("{}Scan({})", pad, table),
        Plan::Filter { pred, input } =>
            format!("{}Filter({:?})\n{}", pad, pred, print_plan(input, d+1)),
    }
}

fn main() {
    // WHERE age > 30 AND active = true AND role = 'admin'
    let pred = Expr::BinaryOp {
        op: Op::And,
        left: Box::new(Expr::BinaryOp {
            op: Op::And,
            left: Box::new(Expr::BinaryOp {
                op: Op::Gt,
                left: Box::new(Expr::Column("age".into())),
                right: Box::new(Expr::Integer(30)),
            }),
            right: Box::new(Expr::BinaryOp {
                op: Op::Eq,
                left: Box::new(Expr::Column("active".into())),
                right: Box::new(Expr::Boolean(true)),
            }),
        }),
        right: Box::new(Expr::BinaryOp {
            op: Op::Eq,
            left: Box::new(Expr::Column("role".into())),
            right: Box::new(Expr::Column("admin".into())),
        }),
    };

    let plan = Plan::Filter {
        pred,
        input: Box::new(Plan::Scan { table: "users".into() }),
    };

    println!("BEFORE:\n{}\n", print_plan(&plan, 0));

    let split = split_filters(plan);
    println!("AFTER:\n{}", print_plan(&split, 0));
    // Three separate Filter nodes, one per condition
}
```

</details>

### Exercise 2: Redundant Filter Elimination

If a filter predicate is a constant `true`, remove the filter node entirely. If it is `false`, replace the entire subtree with an `Empty` node (a scan that returns zero rows). Implement `eliminate_redundant_filters`.

<details>
<summary>Solution</summary>

```rust
#[derive(Debug, Clone)]
enum Expr { Column(String), Integer(i64), Boolean(bool) }

#[derive(Debug, Clone)]
enum Plan {
    Scan { table: String },
    Filter { pred: Expr, input: Box<Plan> },
    Empty,  // produces no rows
}

fn eliminate_redundant(plan: Plan) -> Plan {
    match plan {
        Plan::Filter { pred, input } => {
            let input = eliminate_redundant(*input);
            match pred {
                Expr::Boolean(true) => input,     // filter always passes -> remove
                Expr::Boolean(false) => Plan::Empty, // filter never passes -> empty
                _ => Plan::Filter { pred, input: Box::new(input) },
            }
        }
        other => other,
    }
}

fn print_plan(p: &Plan, d: usize) -> String {
    let pad = "  ".repeat(d);
    match p {
        Plan::Scan { table } => format!("{}Scan({})", pad, table),
        Plan::Filter { pred, input } =>
            format!("{}Filter({:?})\n{}", pad, pred, print_plan(input, d+1)),
        Plan::Empty => format!("{}Empty", pad),
    }
}

fn main() {
    // Filter(true) -> eliminated
    let plan1 = Plan::Filter {
        pred: Expr::Boolean(true),
        input: Box::new(Plan::Scan { table: "users".into() }),
    };
    println!("BEFORE: {}", print_plan(&plan1, 0));
    println!("AFTER:  {}\n", print_plan(&eliminate_redundant(plan1), 0));

    // Filter(false) -> Empty
    let plan2 = Plan::Filter {
        pred: Expr::Boolean(false),
        input: Box::new(Plan::Scan { table: "users".into() }),
    };
    println!("BEFORE: {}", print_plan(&plan2, 0));
    println!("AFTER:  {}\n", print_plan(&eliminate_redundant(plan2), 0));

    // Normal filter -> kept
    let plan3 = Plan::Filter {
        pred: Expr::Column("age".into()),
        input: Box::new(Plan::Scan { table: "users".into() }),
    };
    println!("BEFORE: {}", print_plan(&plan3, 0));
    println!("AFTER:  {}", print_plan(&eliminate_redundant(plan3), 0));
}
```

</details>

### Exercise 3: Limit Pushdown

When a `Limit` sits above a `Project`, push it below. Projecting 10 rows from 1 million is much faster than projecting 1 million rows and then keeping 10. Implement `push_limit_down` that swaps Limit and Project nodes.

<details>
<summary>Solution</summary>

```rust
#[derive(Debug, Clone)]
enum Plan {
    Scan { table: String },
    Filter { pred: String, input: Box<Plan> },
    Project { columns: Vec<String>, input: Box<Plan> },
    Sort { by: Vec<String>, input: Box<Plan> },
    Limit { count: usize, input: Box<Plan> },
}

fn push_limit_down(plan: Plan) -> Plan {
    match plan {
        Plan::Limit { count, input } => {
            match *input {
                // Limit above Project -> push Limit below Project
                Plan::Project { columns, input: proj_input } => {
                    let pushed = Plan::Limit {
                        count,
                        input: proj_input,
                    };
                    let optimized = push_limit_down(pushed);
                    Plan::Project {
                        columns,
                        input: Box::new(optimized),
                    }
                }
                // Do NOT push Limit below Sort -- that would change results!
                // Sort needs all rows before it can determine the top N
                Plan::Sort { by, input: sort_input } => {
                    Plan::Limit {
                        count,
                        input: Box::new(Plan::Sort {
                            by,
                            input: Box::new(push_limit_down(*sort_input)),
                        }),
                    }
                }
                other => Plan::Limit {
                    count,
                    input: Box::new(push_limit_down(other)),
                },
            }
        }
        Plan::Project { columns, input } =>
            Plan::Project { columns, input: Box::new(push_limit_down(*input)) },
        Plan::Filter { pred, input } =>
            Plan::Filter { pred, input: Box::new(push_limit_down(*input)) },
        Plan::Sort { by, input } =>
            Plan::Sort { by, input: Box::new(push_limit_down(*input)) },
        other => other,
    }
}

fn print_plan(p: &Plan, d: usize) -> String {
    let pad = "  ".repeat(d);
    match p {
        Plan::Scan { table } => format!("{}Scan({})", pad, table),
        Plan::Filter { pred, input } =>
            format!("{}Filter({})\n{}", pad, pred, print_plan(input, d+1)),
        Plan::Project { columns, input } =>
            format!("{}Project([{}])\n{}", pad, columns.join(", "), print_plan(input, d+1)),
        Plan::Sort { by, input } =>
            format!("{}Sort([{}])\n{}", pad, by.join(", "), print_plan(input, d+1)),
        Plan::Limit { count, input } =>
            format!("{}Limit({})\n{}", pad, count, print_plan(input, d+1)),
    }
}

fn main() {
    // Limit(10) above Project -> push Limit below
    let plan = Plan::Limit {
        count: 10,
        input: Box::new(Plan::Project {
            columns: vec!["name".into(), "age".into()],
            input: Box::new(Plan::Scan { table: "users".into() }),
        }),
    };

    println!("BEFORE:\n{}\n", print_plan(&plan, 0));
    let optimized = push_limit_down(plan);
    println!("AFTER:\n{}\n", print_plan(&optimized, 0));

    // Limit above Sort -> do NOT push down
    let plan2 = Plan::Limit {
        count: 10,
        input: Box::new(Plan::Sort {
            by: vec!["name".into()],
            input: Box::new(Plan::Scan { table: "users".into() }),
        }),
    };

    println!("BEFORE:\n{}\n", print_plan(&plan2, 0));
    let optimized2 = push_limit_down(plan2);
    println!("AFTER (unchanged):\n{}", print_plan(&optimized2, 0));
}
```

</details>

---

## Recap

Query optimization rewrites the plan tree to do less work while producing the same result. Constant folding eliminates compile-time-computable expressions. Filter pushdown moves predicates closer to the data source, eliminating rows early. Projection pushdown reduces the width of data flowing through the plan. Each optimization is a simple tree transformation, but together they can reduce execution time by orders of magnitude. The optimizer does not change what your query computes -- only how efficiently it computes it.
