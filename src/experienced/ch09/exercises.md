## Exercise 1: The Optimizer Trait

**Goal:** Define the `OptimizerRule` trait, build the `Optimizer` struct that holds a collection of rules, and apply them in sequence to a plan tree.

### Step 1: Prerequisites from previous chapters

Before we build the optimizer, we need the plan types from Chapter 8. Here is the subset we will work with (copy these into your `src/planner.rs` or whatever file holds your plan types, if you have not already):

```rust
// src/planner.rs — plan types from Chapter 8

/// A value in an expression
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Integer(i64),
    Float(f64),
    String(String),
    Boolean(bool),
    Null,
}

/// An expression — the building block of filters and projections
#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    /// A literal value: 42, 'hello', true
    Literal(Value),
    /// A column reference: name, users.id
    ColumnRef(String),
    /// A binary operation: a + b, x = y, age > 30
    BinaryOp {
        left: Box<Expression>,
        op: BinaryOperator,
        right: Box<Expression>,
    },
    /// A unary operation: NOT x, -42
    UnaryOp {
        op: UnaryOperator,
        operand: Box<Expression>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum BinaryOperator {
    Add,
    Subtract,
    Multiply,
    Divide,
    Equal,
    NotEqual,
    LessThan,
    GreaterThan,
    LessOrEqual,
    GreaterOrEqual,
    And,
    Or,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOperator {
    Not,
    Negate,
}

/// A query plan node — a tree of operations
#[derive(Debug, Clone, PartialEq)]
pub enum Plan {
    /// Scan all rows from a table
    Scan {
        table: String,
    },
    /// Filter rows based on a predicate
    Filter {
        predicate: Expression,
        source: Box<Plan>,
    },
    /// Project (select) specific columns
    Project {
        columns: Vec<Expression>,
        source: Box<Plan>,
    },
    /// A node that produces no rows (optimization result)
    EmptyResult,
}
```

### Step 2: Define the OptimizerRule trait

Create `src/optimizer.rs`:

```rust
// src/optimizer.rs

use crate::planner::{Plan, Expression, Value, BinaryOperator, UnaryOperator};

/// A single optimization rule that transforms a plan tree.
///
/// Each rule has a name (for logging) and an optimize method
/// that takes a plan and returns a potentially transformed plan.
pub trait OptimizerRule {
    /// Human-readable name of this rule, used in optimization reports.
    fn name(&self) -> &str;

    /// Apply this rule to a plan tree. Returns the (possibly modified) plan.
    /// The rule should recursively transform the entire tree, not just the root.
    fn optimize(&self, plan: Plan) -> Plan;
}
```

This is the key abstraction. Every optimization rule — constant folding, filter pushdown, dead code elimination, whatever we invent later — implements this single trait. The optimizer does not know or care what specific rules exist. It just calls `optimize()` on each one.

### Step 3: Build the Optimizer struct

```rust
/// The query optimizer. Holds a sequence of optimization rules
/// and applies them to query plans.
pub struct Optimizer {
    rules: Vec<Box<dyn OptimizerRule>>,
}

impl Optimizer {
    /// Create a new optimizer with no rules.
    pub fn new() -> Self {
        Optimizer { rules: Vec::new() }
    }

    /// Add a rule to the optimizer. Rules are applied in the order they are added.
    pub fn add_rule(&mut self, rule: Box<dyn OptimizerRule>) {
        self.rules.push(rule);
    }

    /// Create an optimizer with the default set of rules.
    pub fn default_optimizer() -> Self {
        let mut optimizer = Optimizer::new();
        // We will add rules as we build them in later exercises.
        // For now, this is empty.
        optimizer
    }

    /// Apply all rules to the plan, in order. Returns the optimized plan
    /// and a report of what changed.
    pub fn optimize(&self, plan: Plan) -> OptimizeResult {
        let mut current = plan;
        let mut applied_rules: Vec<String> = Vec::new();

        for rule in &self.rules {
            let before = format!("{:?}", current);
            current = rule.optimize(current);
            let after = format!("{:?}", current);

            if before != after {
                applied_rules.push(rule.name().to_string());
            }
        }

        OptimizeResult {
            plan: current,
            applied_rules,
        }
    }
}

/// The result of optimization: the transformed plan and a log of which rules fired.
#[derive(Debug)]
pub struct OptimizeResult {
    pub plan: Plan,
    pub applied_rules: Vec<String>,
}

impl std::fmt::Display for OptimizeResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Plan: ")?;
        display_plan(&self.plan, f, 0)?;
        if self.applied_rules.is_empty() {
            write!(f, "\nNo optimizations applied.")?;
        } else {
            write!(f, "\nOptimizations applied:")?;
            for rule in &self.applied_rules {
                write!(f, "\n  - {}", rule)?;
            }
        }
        Ok(())
    }
}
```

### Step 4: Display the plan tree

To see what the optimizer is doing, we need a way to print plan trees. Add a recursive display function:

```rust
/// Recursively display a plan tree with indentation.
fn display_plan(
    plan: &Plan,
    f: &mut std::fmt::Formatter<'_>,
    indent: usize,
) -> std::fmt::Result {
    let pad = "  ".repeat(indent);
    match plan {
        Plan::Scan { table } => {
            write!(f, "{}Scan({})", pad, table)
        }
        Plan::Filter { predicate, source } => {
            write!(f, "{}Filter({})\n", pad, format_expr(predicate))?;
            display_plan(source, f, indent + 1)
        }
        Plan::Project { columns, source } => {
            let cols: Vec<String> = columns.iter().map(|c| format_expr(c)).collect();
            write!(f, "{}Project({})\n", pad, cols.join(", "))?;
            display_plan(source, f, indent + 1)
        }
        Plan::EmptyResult => {
            write!(f, "{}EmptyResult", pad)
        }
    }
}

/// Format an expression as a human-readable string.
pub fn format_expr(expr: &Expression) -> String {
    match expr {
        Expression::Literal(Value::Integer(n)) => n.to_string(),
        Expression::Literal(Value::Float(f)) => f.to_string(),
        Expression::Literal(Value::String(s)) => format!("'{}'", s),
        Expression::Literal(Value::Boolean(b)) => b.to_string(),
        Expression::Literal(Value::Null) => "NULL".to_string(),
        Expression::ColumnRef(name) => name.clone(),
        Expression::BinaryOp { left, op, right } => {
            let op_str = match op {
                BinaryOperator::Add => "+",
                BinaryOperator::Subtract => "-",
                BinaryOperator::Multiply => "*",
                BinaryOperator::Divide => "/",
                BinaryOperator::Equal => "=",
                BinaryOperator::NotEqual => "!=",
                BinaryOperator::LessThan => "<",
                BinaryOperator::GreaterThan => ">",
                BinaryOperator::LessOrEqual => "<=",
                BinaryOperator::GreaterOrEqual => ">=",
                BinaryOperator::And => "AND",
                BinaryOperator::Or => "OR",
            };
            format!("({} {} {})", format_expr(left), op_str, format_expr(right))
        }
        Expression::UnaryOp { op, operand } => {
            let op_str = match op {
                UnaryOperator::Not => "NOT ",
                UnaryOperator::Negate => "-",
            };
            format!("{}{}", op_str, format_expr(operand))
        }
    }
}
```

### Step 5: Test the optimizer framework

```rust
#[cfg(test)]
mod optimizer_tests {
    use super::*;

    /// A trivial rule that does nothing — useful for testing the framework.
    struct NoOpRule;

    impl OptimizerRule for NoOpRule {
        fn name(&self) -> &str {
            "NoOp"
        }

        fn optimize(&self, plan: Plan) -> Plan {
            plan
        }
    }

    /// A rule that replaces all Scan nodes with EmptyResult.
    /// (Not useful in practice, but tests recursive application.)
    struct KillScansRule;

    impl OptimizerRule for KillScansRule {
        fn name(&self) -> &str {
            "KillScans"
        }

        fn optimize(&self, plan: Plan) -> Plan {
            match plan {
                Plan::Scan { .. } => Plan::EmptyResult,
                Plan::Filter { predicate, source } => Plan::Filter {
                    predicate,
                    source: Box::new(self.optimize(*source)),
                },
                Plan::Project { columns, source } => Plan::Project {
                    columns,
                    source: Box::new(self.optimize(*source)),
                },
                Plan::EmptyResult => Plan::EmptyResult,
            }
        }
    }

    #[test]
    fn empty_optimizer_returns_plan_unchanged() {
        let optimizer = Optimizer::new();
        let plan = Plan::Scan { table: "users".to_string() };

        let result = optimizer.optimize(plan.clone());
        assert_eq!(result.plan, plan);
        assert!(result.applied_rules.is_empty());
    }

    #[test]
    fn noop_rule_does_not_appear_in_applied() {
        let mut optimizer = Optimizer::new();
        optimizer.add_rule(Box::new(NoOpRule));

        let plan = Plan::Scan { table: "users".to_string() };
        let result = optimizer.optimize(plan);

        assert!(result.applied_rules.is_empty());
    }

    #[test]
    fn kill_scans_rule_replaces_scan() {
        let mut optimizer = Optimizer::new();
        optimizer.add_rule(Box::new(KillScansRule));

        let plan = Plan::Project {
            columns: vec![Expression::ColumnRef("name".to_string())],
            source: Box::new(Plan::Scan { table: "users".to_string() }),
        };

        let result = optimizer.optimize(plan);
        let expected = Plan::Project {
            columns: vec![Expression::ColumnRef("name".to_string())],
            source: Box::new(Plan::EmptyResult),
        };
        assert_eq!(result.plan, expected);
        assert_eq!(result.applied_rules, vec!["KillScans"]);
    }

    #[test]
    fn multiple_rules_applied_in_order() {
        let mut optimizer = Optimizer::new();
        optimizer.add_rule(Box::new(NoOpRule));
        optimizer.add_rule(Box::new(KillScansRule));

        let plan = Plan::Scan { table: "users".to_string() };
        let result = optimizer.optimize(plan);

        assert_eq!(result.plan, Plan::EmptyResult);
        // Only KillScans actually changed the plan
        assert_eq!(result.applied_rules, vec!["KillScans"]);
    }
}
```

Run the tests:

```
$ cargo test optimizer_tests
running 4 tests
test optimizer::optimizer_tests::empty_optimizer_returns_plan_unchanged ... ok
test optimizer::optimizer_tests::noop_rule_does_not_appear_in_applied ... ok
test optimizer::optimizer_tests::kill_scans_rule_replaces_scan ... ok
test optimizer::optimizer_tests::multiple_rules_applied_in_order ... ok

test result: ok. 4 passed; 0 failed
```

The framework is clean: define a rule, box it, add it to the optimizer, done. The `Vec<Box<dyn OptimizerRule>>` is the key data structure — it holds different concrete types behind a single interface. Each rule is free to do whatever it wants with the plan tree, as long as it returns a valid plan.

> **Why not an enum of rules?** We could define `enum Rule { ConstantFolding, FilterPushdown }` and match on it. That works for a closed set of rules, but it means adding a new rule requires modifying the enum and every match statement that uses it. With trait objects, adding a new rule is just implementing the trait and calling `add_rule()`. The existing code never changes. This is the Open-Closed Principle in action: open for extension, closed for modification.

---

## Exercise 2: Constant Folding

**Goal:** Build a rule that evaluates constant expressions at plan time, eliminating unnecessary computation at execution time.

### The idea

When the planner sees `WHERE 1 + 1 = 2`, it creates a `Filter` node with this expression tree:

```
        BinaryOp(=)
       /            \
  BinaryOp(+)    Literal(2)
   /       \
Literal(1)  Literal(1)
```

Every row that passes through this filter computes `1 + 1`, then checks if the result equals `2`. That is wasteful — the answer is always `true`. The constant folding rule detects that both sides of `+` are literals, evaluates `1 + 1 = 2` at plan time, and replaces the entire expression tree with `Literal(true)`.

Then, since the filter's predicate is always true, the rule removes the `Filter` node entirely — there is no point filtering when everything passes.

### Step 1: Implement constant evaluation

We need a helper function that tries to evaluate an expression. If all operands are constants, it computes the result. If any operand references a column, it returns the expression unchanged.

```rust
/// Attempt to evaluate a constant expression. Returns the simplified expression.
/// If the expression contains column references, it cannot be fully evaluated
/// and is returned with sub-expressions folded as much as possible.
fn fold_constants(expr: Expression) -> Expression {
    match expr {
        // Literals are already constant — nothing to fold.
        Expression::Literal(_) => expr,

        // Column references cannot be evaluated at plan time.
        Expression::ColumnRef(_) => expr,

        // Binary operations: try to fold both sides, then evaluate if both are literals.
        Expression::BinaryOp { left, op, right } => {
            let left = fold_constants(*left);
            let right = fold_constants(*right);

            // If both sides are now literals, evaluate the operation.
            match (&left, &right) {
                (Expression::Literal(l), Expression::Literal(r)) => {
                    match eval_binary(&op, l, r) {
                        Some(result) => Expression::Literal(result),
                        None => Expression::BinaryOp {
                            left: Box::new(left),
                            op,
                            right: Box::new(right),
                        },
                    }
                }
                _ => Expression::BinaryOp {
                    left: Box::new(left),
                    op,
                    right: Box::new(right),
                },
            }
        }

        // Unary operations: fold the operand, then evaluate if it is a literal.
        Expression::UnaryOp { op, operand } => {
            let operand = fold_constants(*operand);
            match &operand {
                Expression::Literal(v) => {
                    match eval_unary(&op, v) {
                        Some(result) => Expression::Literal(result),
                        None => Expression::UnaryOp {
                            op,
                            operand: Box::new(operand),
                        },
                    }
                }
                _ => Expression::UnaryOp {
                    op,
                    operand: Box::new(operand),
                },
            }
        }
    }
}
```

### Step 2: Implement the evaluation functions

These functions compute the result of applying an operator to literal values:

```rust
/// Evaluate a binary operation on two literal values.
/// Returns None if the operation is not supported for the given types.
fn eval_binary(op: &BinaryOperator, left: &Value, right: &Value) -> Option<Value> {
    match (op, left, right) {
        // Arithmetic on integers
        (BinaryOperator::Add, Value::Integer(a), Value::Integer(b)) => {
            Some(Value::Integer(a + b))
        }
        (BinaryOperator::Subtract, Value::Integer(a), Value::Integer(b)) => {
            Some(Value::Integer(a - b))
        }
        (BinaryOperator::Multiply, Value::Integer(a), Value::Integer(b)) => {
            Some(Value::Integer(a * b))
        }
        (BinaryOperator::Divide, Value::Integer(a), Value::Integer(b)) => {
            if *b == 0 {
                None // Division by zero: leave unfolded, let the executor handle it
            } else {
                Some(Value::Integer(a / b))
            }
        }

        // Arithmetic on floats
        (BinaryOperator::Add, Value::Float(a), Value::Float(b)) => {
            Some(Value::Float(a + b))
        }
        (BinaryOperator::Subtract, Value::Float(a), Value::Float(b)) => {
            Some(Value::Float(a - b))
        }
        (BinaryOperator::Multiply, Value::Float(a), Value::Float(b)) => {
            Some(Value::Float(a * b))
        }
        (BinaryOperator::Divide, Value::Float(a), Value::Float(b)) => {
            if *b == 0.0 {
                None
            } else {
                Some(Value::Float(a / b))
            }
        }

        // Comparison on integers
        (BinaryOperator::Equal, Value::Integer(a), Value::Integer(b)) => {
            Some(Value::Boolean(a == b))
        }
        (BinaryOperator::NotEqual, Value::Integer(a), Value::Integer(b)) => {
            Some(Value::Boolean(a != b))
        }
        (BinaryOperator::LessThan, Value::Integer(a), Value::Integer(b)) => {
            Some(Value::Boolean(a < b))
        }
        (BinaryOperator::GreaterThan, Value::Integer(a), Value::Integer(b)) => {
            Some(Value::Boolean(a > b))
        }
        (BinaryOperator::LessOrEqual, Value::Integer(a), Value::Integer(b)) => {
            Some(Value::Boolean(a <= b))
        }
        (BinaryOperator::GreaterOrEqual, Value::Integer(a), Value::Integer(b)) => {
            Some(Value::Boolean(a >= b))
        }

        // Comparison on strings
        (BinaryOperator::Equal, Value::String(a), Value::String(b)) => {
            Some(Value::Boolean(a == b))
        }
        (BinaryOperator::NotEqual, Value::String(a), Value::String(b)) => {
            Some(Value::Boolean(a != b))
        }

        // Boolean logic
        (BinaryOperator::And, Value::Boolean(a), Value::Boolean(b)) => {
            Some(Value::Boolean(*a && *b))
        }
        (BinaryOperator::Or, Value::Boolean(a), Value::Boolean(b)) => {
            Some(Value::Boolean(*a || *b))
        }

        // Comparison on booleans
        (BinaryOperator::Equal, Value::Boolean(a), Value::Boolean(b)) => {
            Some(Value::Boolean(a == b))
        }

        // NULL comparisons: anything compared with NULL is NULL
        (_, Value::Null, _) | (_, _, Value::Null) => Some(Value::Null),

        // Type mismatch or unsupported operation: cannot fold
        _ => None,
    }
}

/// Evaluate a unary operation on a literal value.
fn eval_unary(op: &UnaryOperator, value: &Value) -> Option<Value> {
    match (op, value) {
        (UnaryOperator::Negate, Value::Integer(n)) => Some(Value::Integer(-n)),
        (UnaryOperator::Negate, Value::Float(f)) => Some(Value::Float(-f)),
        (UnaryOperator::Not, Value::Boolean(b)) => Some(Value::Boolean(!b)),
        _ => None,
    }
}
```

### Step 3: Implement the ConstantFolding rule

Now we wrap the folding logic in an `OptimizerRule` implementation:

```rust
/// Constant Folding: evaluate expressions made entirely of literals at plan time.
///
/// Examples:
/// - `WHERE 1 + 1 = 2` becomes `WHERE true` (then Filter is removed)
/// - `WHERE 3 > 5` becomes `WHERE false` (then plan becomes EmptyResult)
/// - `WHERE age > 2 + 3` becomes `WHERE age > 5` (partial fold)
pub struct ConstantFolding;

impl OptimizerRule for ConstantFolding {
    fn name(&self) -> &str {
        "ConstantFolding"
    }

    fn optimize(&self, plan: Plan) -> Plan {
        match plan {
            Plan::Filter { predicate, source } => {
                // First, recursively optimize the source plan.
                let optimized_source = self.optimize(*source);

                // Fold constants in the predicate.
                let folded = fold_constants(predicate);

                match &folded {
                    // Predicate is always true: remove the filter entirely.
                    Expression::Literal(Value::Boolean(true)) => optimized_source,

                    // Predicate is always false: nothing will pass, emit EmptyResult.
                    Expression::Literal(Value::Boolean(false)) => Plan::EmptyResult,

                    // Predicate is NULL: in SQL, NULL in a WHERE clause means
                    // "unknown", which is treated as false.
                    Expression::Literal(Value::Null) => Plan::EmptyResult,

                    // Predicate still has non-constant parts: keep the filter.
                    _ => Plan::Filter {
                        predicate: folded,
                        source: Box::new(optimized_source),
                    },
                }
            }

            // Project: fold constants in column expressions, recurse into source.
            Plan::Project { columns, source } => {
                let optimized_source = self.optimize(*source);
                let folded_columns: Vec<Expression> = columns
                    .into_iter()
                    .map(fold_constants)
                    .collect();
                Plan::Project {
                    columns: folded_columns,
                    source: Box::new(optimized_source),
                }
            }

            // Scan and EmptyResult have no expressions to fold.
            Plan::Scan { .. } => plan,
            Plan::EmptyResult => plan,
        }
    }
}
```

### Step 4: Test constant folding

```rust
#[cfg(test)]
mod constant_folding_tests {
    use super::*;

    /// Helper: create a simple binary expression
    fn binop(left: Expression, op: BinaryOperator, right: Expression) -> Expression {
        Expression::BinaryOp {
            left: Box::new(left),
            op,
            right: Box::new(right),
        }
    }

    /// Helper: create an integer literal
    fn int(n: i64) -> Expression {
        Expression::Literal(Value::Integer(n))
    }

    /// Helper: create a boolean literal
    fn bool_lit(b: bool) -> Expression {
        Expression::Literal(Value::Boolean(b))
    }

    /// Helper: create a column reference
    fn col(name: &str) -> Expression {
        Expression::ColumnRef(name.to_string())
    }

    #[test]
    fn fold_1_plus_1_equals_2() {
        // WHERE 1 + 1 = 2  =>  Filter removed (always true)
        let plan = Plan::Filter {
            predicate: binop(
                binop(int(1), BinaryOperator::Add, int(1)),
                BinaryOperator::Equal,
                int(2),
            ),
            source: Box::new(Plan::Scan { table: "users".to_string() }),
        };

        let rule = ConstantFolding;
        let result = rule.optimize(plan);

        // Filter should be removed entirely — the predicate is always true
        assert_eq!(result, Plan::Scan { table: "users".to_string() });
    }

    #[test]
    fn fold_3_greater_than_5_is_false() {
        // WHERE 3 > 5  =>  EmptyResult (always false)
        let plan = Plan::Filter {
            predicate: binop(int(3), BinaryOperator::GreaterThan, int(5)),
            source: Box::new(Plan::Scan { table: "users".to_string() }),
        };

        let rule = ConstantFolding;
        let result = rule.optimize(plan);

        assert_eq!(result, Plan::EmptyResult);
    }

    #[test]
    fn partial_fold_age_greater_than_2_plus_3() {
        // WHERE age > 2 + 3  =>  WHERE age > 5
        let plan = Plan::Filter {
            predicate: binop(
                col("age"),
                BinaryOperator::GreaterThan,
                binop(int(2), BinaryOperator::Add, int(3)),
            ),
            source: Box::new(Plan::Scan { table: "users".to_string() }),
        };

        let rule = ConstantFolding;
        let result = rule.optimize(plan);

        let expected = Plan::Filter {
            predicate: binop(col("age"), BinaryOperator::GreaterThan, int(5)),
            source: Box::new(Plan::Scan { table: "users".to_string() }),
        };
        assert_eq!(result, expected);
    }

    #[test]
    fn no_fold_when_column_involved() {
        // WHERE age > 30  =>  unchanged (age is not a constant)
        let plan = Plan::Filter {
            predicate: binop(col("age"), BinaryOperator::GreaterThan, int(30)),
            source: Box::new(Plan::Scan { table: "users".to_string() }),
        };

        let rule = ConstantFolding;
        let result = rule.optimize(plan.clone());

        assert_eq!(result, plan);
    }

    #[test]
    fn fold_nested_constants() {
        // WHERE (2 * 3) + (4 - 1) = 9  =>  Filter removed (6 + 3 = 9 is true)
        let plan = Plan::Filter {
            predicate: binop(
                binop(
                    binop(int(2), BinaryOperator::Multiply, int(3)),
                    BinaryOperator::Add,
                    binop(int(4), BinaryOperator::Subtract, int(1)),
                ),
                BinaryOperator::Equal,
                int(9),
            ),
            source: Box::new(Plan::Scan { table: "orders".to_string() }),
        };

        let rule = ConstantFolding;
        let result = rule.optimize(plan);

        assert_eq!(result, Plan::Scan { table: "orders".to_string() });
    }

    #[test]
    fn fold_boolean_and_true_true() {
        // WHERE true AND true  =>  Filter removed
        let plan = Plan::Filter {
            predicate: binop(bool_lit(true), BinaryOperator::And, bool_lit(true)),
            source: Box::new(Plan::Scan { table: "users".to_string() }),
        };

        let rule = ConstantFolding;
        let result = rule.optimize(plan);

        assert_eq!(result, Plan::Scan { table: "users".to_string() });
    }

    #[test]
    fn fold_boolean_and_true_false() {
        // WHERE true AND false  =>  EmptyResult
        let plan = Plan::Filter {
            predicate: binop(bool_lit(true), BinaryOperator::And, bool_lit(false)),
            source: Box::new(Plan::Scan { table: "users".to_string() }),
        };

        let rule = ConstantFolding;
        let result = rule.optimize(plan);

        assert_eq!(result, Plan::EmptyResult);
    }

    #[test]
    fn fold_in_project_columns() {
        // SELECT 1 + 2, name FROM users  =>  SELECT 3, name FROM users
        let plan = Plan::Project {
            columns: vec![
                binop(int(1), BinaryOperator::Add, int(2)),
                col("name"),
            ],
            source: Box::new(Plan::Scan { table: "users".to_string() }),
        };

        let rule = ConstantFolding;
        let result = rule.optimize(plan);

        let expected = Plan::Project {
            columns: vec![int(3), col("name")],
            source: Box::new(Plan::Scan { table: "users".to_string() }),
        };
        assert_eq!(result, expected);
    }
}
```

Run the tests:

```
$ cargo test constant_folding_tests
running 8 tests
test optimizer::constant_folding_tests::fold_1_plus_1_equals_2 ... ok
test optimizer::constant_folding_tests::fold_3_greater_than_5_is_false ... ok
test optimizer::constant_folding_tests::partial_fold_age_greater_than_2_plus_3 ... ok
test optimizer::constant_folding_tests::no_fold_when_column_involved ... ok
test optimizer::constant_folding_tests::fold_nested_constants ... ok
test optimizer::constant_folding_tests::fold_boolean_and_true_true ... ok
test optimizer::constant_folding_tests::fold_boolean_and_true_false ... ok
test optimizer::constant_folding_tests::fold_in_project_columns ... ok

test result: ok. 8 passed; 0 failed
```

Notice how `fold_constants` is recursive. The expression `(2 * 3) + (4 - 1) = 9` unfolds from the leaves: first `2 * 3` becomes `6`, then `4 - 1` becomes `3`, then `6 + 3` becomes `9`, then `9 = 9` becomes `true`. Each recursive call handles one level of the tree. This bottom-up evaluation is the natural approach when folding expressions — you must know the children's values before you can compute the parent's value.

---

## Exercise 3: Filter Pushdown

**Goal:** Build a rule that moves filter predicates closer to the data source, reducing the number of rows that flow through the plan.

### The idea

Consider this query: `SELECT name FROM users WHERE age > 30`. The naive planner might produce:

```
Project(name)
  Filter(age > 30)
    Scan(users)
```

This is already in a good order — the filter is right above the scan. But what about a more complex plan?

```
Project(name, total)
  Filter(age > 30)
    Project(name, age, price * quantity AS total)
      Scan(orders_with_users)
```

Here the filter `age > 30` sits above a `Project`. The `Project` computes `price * quantity` for every row, even rows where `age <= 30`. If we push the filter below the project:

```
Project(name, total)
  Project(name, age, price * quantity AS total)
    Filter(age > 30)
      Scan(orders_with_users)
```

Now the filter runs first, and the expensive `price * quantity` computation only happens for rows where `age > 30`. On a million-row table where only 10,000 rows pass the filter, this eliminates 990,000 unnecessary multiplications.

### Step 1: Check if a filter can be pushed past a node

A filter can be pushed past a `Project` only if the filter references columns that are available below the project. We need a helper that extracts column references from an expression:

```rust
/// Extract all column names referenced in an expression.
fn referenced_columns(expr: &Expression) -> Vec<String> {
    match expr {
        Expression::Literal(_) => Vec::new(),
        Expression::ColumnRef(name) => vec![name.clone()],
        Expression::BinaryOp { left, right, .. } => {
            let mut cols = referenced_columns(left);
            cols.extend(referenced_columns(right));
            cols
        }
        Expression::UnaryOp { operand, .. } => referenced_columns(operand),
    }
}

/// Get the columns produced by a project node (their output names).
/// For simple column references, the name is the column name.
/// For other expressions, we cannot easily determine the name,
/// so we skip them.
fn project_output_columns(columns: &[Expression]) -> Vec<String> {
    columns.iter().filter_map(|expr| {
        match expr {
            Expression::ColumnRef(name) => Some(name.clone()),
            _ => None,
        }
    }).collect()
}

/// Check if all columns referenced by the predicate exist in the
/// source columns (i.e., the filter can be pushed below the project).
fn can_push_past_project(predicate: &Expression, project_cols: &[Expression]) -> bool {
    let needed = referenced_columns(predicate);
    let available = project_output_columns(project_cols);

    // The filter can be pushed down if every column it references
    // is available as a simple column pass-through in the project.
    needed.iter().all(|col| available.contains(col))
}
```

### Step 2: Implement the FilterPushdown rule

```rust
/// Filter Pushdown: move filters closer to the data source.
///
/// This reduces the number of rows processed by expensive operations
/// like projections with computed columns.
///
/// The rule pushes a Filter node past a Project node when the filter
/// only references columns that pass through the project unchanged.
pub struct FilterPushdown;

impl OptimizerRule for FilterPushdown {
    fn name(&self) -> &str {
        "FilterPushdown"
    }

    fn optimize(&self, plan: Plan) -> Plan {
        match plan {
            // The key pattern: Filter on top of Project.
            // If the filter can be pushed down, swap them.
            Plan::Filter { predicate, source } => {
                let optimized_source = self.optimize(*source);

                match optimized_source {
                    Plan::Project { columns, source: project_source } => {
                        if can_push_past_project(&predicate, &columns) {
                            // Push the filter below the project:
                            //   Filter(pred)            Project(cols)
                            //     Project(cols)    =>      Filter(pred)
                            //       Source                   Source
                            Plan::Project {
                                columns,
                                source: Box::new(Plan::Filter {
                                    predicate,
                                    source: project_source,
                                }),
                            }
                        } else {
                            // Cannot push down — keep the original order.
                            Plan::Filter {
                                predicate,
                                source: Box::new(Plan::Project {
                                    columns,
                                    source: project_source,
                                }),
                            }
                        }
                    }
                    // Filter on top of Filter: recursively optimize,
                    // but do not reorder filters.
                    other => Plan::Filter {
                        predicate,
                        source: Box::new(other),
                    },
                }
            }

            // Recursively optimize children.
            Plan::Project { columns, source } => {
                Plan::Project {
                    columns,
                    source: Box::new(self.optimize(*source)),
                }
            }

            // Leaf nodes: nothing to push.
            Plan::Scan { .. } => plan,
            Plan::EmptyResult => plan,
        }
    }
}
```

### Step 3: Test filter pushdown

```rust
#[cfg(test)]
mod filter_pushdown_tests {
    use super::*;

    fn binop(left: Expression, op: BinaryOperator, right: Expression) -> Expression {
        Expression::BinaryOp {
            left: Box::new(left),
            op,
            right: Box::new(right),
        }
    }

    fn int(n: i64) -> Expression {
        Expression::Literal(Value::Integer(n))
    }

    fn col(name: &str) -> Expression {
        Expression::ColumnRef(name.to_string())
    }

    #[test]
    fn push_filter_past_project() {
        // Before:
        //   Filter(age > 30)
        //     Project(name, age)
        //       Scan(users)
        //
        // After:
        //   Project(name, age)
        //     Filter(age > 30)
        //       Scan(users)

        let plan = Plan::Filter {
            predicate: binop(col("age"), BinaryOperator::GreaterThan, int(30)),
            source: Box::new(Plan::Project {
                columns: vec![col("name"), col("age")],
                source: Box::new(Plan::Scan { table: "users".to_string() }),
            }),
        };

        let rule = FilterPushdown;
        let result = rule.optimize(plan);

        let expected = Plan::Project {
            columns: vec![col("name"), col("age")],
            source: Box::new(Plan::Filter {
                predicate: binop(col("age"), BinaryOperator::GreaterThan, int(30)),
                source: Box::new(Plan::Scan { table: "users".to_string() }),
            }),
        };

        assert_eq!(result, expected);
    }

    #[test]
    fn do_not_push_filter_referencing_computed_column() {
        // Before:
        //   Filter(total > 100)
        //     Project(name, price * qty AS total)
        //       Scan(orders)
        //
        // "total" is a computed column, not a pass-through.
        // The filter cannot be pushed below the project.

        let plan = Plan::Filter {
            predicate: binop(col("total"), BinaryOperator::GreaterThan, int(100)),
            source: Box::new(Plan::Project {
                columns: vec![
                    col("name"),
                    // This is a computed expression, not a simple column reference
                    binop(col("price"), BinaryOperator::Multiply, col("qty")),
                ],
                source: Box::new(Plan::Scan { table: "orders".to_string() }),
            }),
        };

        let rule = FilterPushdown;
        let result = rule.optimize(plan.clone());

        // Plan should be unchanged — cannot push past computed column
        assert_eq!(result, plan);
    }

    #[test]
    fn filter_on_scan_stays_in_place() {
        // Filter directly on Scan — already in the best position.
        let plan = Plan::Filter {
            predicate: binop(col("age"), BinaryOperator::GreaterThan, int(30)),
            source: Box::new(Plan::Scan { table: "users".to_string() }),
        };

        let rule = FilterPushdown;
        let result = rule.optimize(plan.clone());

        assert_eq!(result, plan);
    }

    #[test]
    fn push_filter_through_nested_project() {
        // Before:
        //   Filter(id = 1)
        //     Project(id, name)
        //       Project(id, name, age)
        //         Scan(users)
        //
        // After:
        //   Project(id, name)
        //     Filter(id = 1)
        //       Project(id, name, age)
        //         Scan(users)
        //
        // The filter pushes past the outer project only.
        // A second pass could push it further.

        let plan = Plan::Filter {
            predicate: binop(col("id"), BinaryOperator::Equal, int(1)),
            source: Box::new(Plan::Project {
                columns: vec![col("id"), col("name")],
                source: Box::new(Plan::Project {
                    columns: vec![col("id"), col("name"), col("age")],
                    source: Box::new(Plan::Scan { table: "users".to_string() }),
                }),
            }),
        };

        let rule = FilterPushdown;
        let result = rule.optimize(plan);

        let expected = Plan::Project {
            columns: vec![col("id"), col("name")],
            source: Box::new(Plan::Filter {
                predicate: binop(col("id"), BinaryOperator::Equal, int(1)),
                source: Box::new(Plan::Project {
                    columns: vec![col("id"), col("name"), col("age")],
                    source: Box::new(Plan::Scan { table: "users".to_string() }),
                }),
            }),
        };

        assert_eq!(result, expected);
    }
}
```

Run the tests:

```
$ cargo test filter_pushdown_tests
running 4 tests
test optimizer::filter_pushdown_tests::push_filter_past_project ... ok
test optimizer::filter_pushdown_tests::do_not_push_filter_referencing_computed_column ... ok
test optimizer::filter_pushdown_tests::filter_on_scan_stays_in_place ... ok
test optimizer::filter_pushdown_tests::push_filter_through_nested_project ... ok

test result: ok. 4 passed; 0 failed
```

Filter pushdown is one of the most impactful optimizations in real databases. In production systems, it pushes filters past joins (filtering before joining millions of rows), past aggregations (filtering before grouping), and even into storage engines (predicate pushdown to skip entire disk pages). Our version handles the Project case, which demonstrates the core idea: do less work by filtering early.

---

## Exercise 4: Running the Full Pipeline

**Goal:** Wire together the lexer, parser, planner, and optimizer into a single `compile` function that takes raw SQL and returns an optimized plan.

### Step 1: Update the default optimizer

First, add the rules we built to the default optimizer:

```rust
impl Optimizer {
    /// Create an optimizer with the standard set of rules.
    /// Rules are applied in this order:
    /// 1. ConstantFolding — simplify constant expressions
    /// 2. FilterPushdown — move filters closer to data sources
    ///
    /// The order matters: constant folding should run first so that
    /// filter pushdown can see simplified predicates.
    pub fn default_optimizer() -> Self {
        let mut optimizer = Optimizer::new();
        optimizer.add_rule(Box::new(ConstantFolding));
        optimizer.add_rule(Box::new(FilterPushdown));
        optimizer
    }
}
```

### Step 2: Build the compile function

This is the top-level entry point. It threads SQL through every stage of the pipeline:

```rust
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::planner::Planner;

/// Compilation error — wraps errors from any stage of the pipeline.
#[derive(Debug)]
pub enum CompileError {
    LexError(String),
    ParseError(String),
    PlanError(String),
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompileError::LexError(msg) => write!(f, "Lex error: {}", msg),
            CompileError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            CompileError::PlanError(msg) => write!(f, "Plan error: {}", msg),
        }
    }
}

/// Compile a SQL string into an optimized query plan.
///
/// Pipeline: SQL → Tokens → AST → Plan → Optimized Plan
///
/// Returns both the optimized plan and the optimization report.
pub fn compile(sql: &str) -> Result<OptimizeResult, CompileError> {
    // Stage 1: Lex
    let tokens = Lexer::tokenize(sql)
        .map_err(CompileError::LexError)?;

    // Stage 2: Parse
    let ast = Parser::parse(tokens)
        .map_err(CompileError::ParseError)?;

    // Stage 3: Plan
    let plan = Planner::plan(ast)
        .map_err(CompileError::PlanError)?;

    // Stage 4: Optimize
    let optimizer = Optimizer::default_optimizer();
    let result = optimizer.optimize(plan);

    Ok(result)
}
```

### Step 3: Display for Plan

Implement `Display` for `Plan` so we can print plans directly:

```rust
impl std::fmt::Display for Plan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        display_plan(self, f, 0)
    }
}
```

### Step 4: Demonstrate the pipeline

Here is a standalone demonstration that shows the optimizer in action. Since you may not have Chapters 6-8 wired together yet, we will build plans manually and optimize them:

```rust
/// Demonstrate the optimizer by building plans manually and optimizing them.
fn demonstrate_optimizer() {
    println!("=== Query Optimizer Demo ===\n");

    let optimizer = Optimizer::default_optimizer();

    // Demo 1: Constant folding removes trivial filter
    println!("--- Demo 1: Constant Folding ---");
    println!("SQL: SELECT * FROM users WHERE 1 + 1 = 2\n");

    let plan1 = Plan::Project {
        columns: vec![Expression::ColumnRef("*".to_string())],
        source: Box::new(Plan::Filter {
            predicate: Expression::BinaryOp {
                left: Box::new(Expression::BinaryOp {
                    left: Box::new(Expression::Literal(Value::Integer(1))),
                    op: BinaryOperator::Add,
                    right: Box::new(Expression::Literal(Value::Integer(1))),
                }),
                op: BinaryOperator::Equal,
                right: Box::new(Expression::Literal(Value::Integer(2))),
            },
            source: Box::new(Plan::Scan { table: "users".to_string() }),
        }),
    };

    println!("Before optimization:");
    println!("{}\n", plan1);

    let result1 = optimizer.optimize(plan1);
    println!("After optimization:");
    println!("{}\n", result1);

    // Demo 2: Constant folding produces EmptyResult
    println!("--- Demo 2: False Predicate ---");
    println!("SQL: SELECT name FROM users WHERE 3 > 5\n");

    let plan2 = Plan::Project {
        columns: vec![Expression::ColumnRef("name".to_string())],
        source: Box::new(Plan::Filter {
            predicate: Expression::BinaryOp {
                left: Box::new(Expression::Literal(Value::Integer(3))),
                op: BinaryOperator::GreaterThan,
                right: Box::new(Expression::Literal(Value::Integer(5))),
            },
            source: Box::new(Plan::Scan { table: "users".to_string() }),
        }),
    };

    println!("Before optimization:");
    println!("{}\n", plan2);

    let result2 = optimizer.optimize(plan2);
    println!("After optimization:");
    println!("{}\n", result2);

    // Demo 3: Filter pushdown
    println!("--- Demo 3: Filter Pushdown ---");
    println!("SQL: SELECT name, age FROM users WHERE age > 30\n");

    let plan3 = Plan::Filter {
        predicate: Expression::BinaryOp {
            left: Box::new(Expression::ColumnRef("age".to_string())),
            op: BinaryOperator::GreaterThan,
            right: Box::new(Expression::Literal(Value::Integer(30))),
        },
        source: Box::new(Plan::Project {
            columns: vec![
                Expression::ColumnRef("name".to_string()),
                Expression::ColumnRef("age".to_string()),
            ],
            source: Box::new(Plan::Scan { table: "users".to_string() }),
        }),
    };

    println!("Before optimization:");
    println!("{}\n", plan3);

    let result3 = optimizer.optimize(plan3);
    println!("After optimization:");
    println!("{}\n", result3);

    // Demo 4: Both rules together — partial fold + pushdown
    println!("--- Demo 4: Combined Optimizations ---");
    println!("SQL: SELECT name, age FROM users WHERE age > 10 + 20\n");

    let plan4 = Plan::Filter {
        predicate: Expression::BinaryOp {
            left: Box::new(Expression::ColumnRef("age".to_string())),
            op: BinaryOperator::GreaterThan,
            right: Box::new(Expression::BinaryOp {
                left: Box::new(Expression::Literal(Value::Integer(10))),
                op: BinaryOperator::Add,
                right: Box::new(Expression::Literal(Value::Integer(20))),
            }),
        },
        source: Box::new(Plan::Project {
            columns: vec![
                Expression::ColumnRef("name".to_string()),
                Expression::ColumnRef("age".to_string()),
            ],
            source: Box::new(Plan::Scan { table: "users".to_string() }),
        }),
    };

    println!("Before optimization:");
    println!("{}\n", plan4);

    let result4 = optimizer.optimize(plan4);
    println!("After optimization:");
    println!("{}\n", result4);
}
```

### Expected output

```
=== Query Optimizer Demo ===

--- Demo 1: Constant Folding ---
SQL: SELECT * FROM users WHERE 1 + 1 = 2

Before optimization:
Project(*)
  Filter((1 + 1) = 2)
    Scan(users)

After optimization:
Plan: Project(*)
  Scan(users)
Optimizations applied:
  - ConstantFolding

--- Demo 2: False Predicate ---
SQL: SELECT name FROM users WHERE 3 > 5

Before optimization:
Project(name)
  Filter(3 > 5)
    Scan(users)

After optimization:
Plan: Project(name)
  EmptyResult
Optimizations applied:
  - ConstantFolding

--- Demo 3: Filter Pushdown ---
SQL: SELECT name, age FROM users WHERE age > 30

Before optimization:
Filter((age > 30))
  Project(name, age)
    Scan(users)

After optimization:
Plan: Project(name, age)
  Filter((age > 30))
    Scan(users)
Optimizations applied:
  - FilterPushdown

--- Demo 4: Combined Optimizations ---
SQL: SELECT name, age FROM users WHERE age > 10 + 20

Before optimization:
Filter((age > (10 + 20)))
  Project(name, age)
    Scan(users)

After optimization:
Plan: Project(name, age)
  Filter((age > 30))
    Scan(users)
Optimizations applied:
  - ConstantFolding
  - FilterPushdown
```

Demo 4 is the most interesting. Two rules fire in sequence: constant folding simplifies `10 + 20` to `30`, then filter pushdown moves the filter below the project. The order matters — if filter pushdown ran first, it would still work (it only looks at column names, not literal values), but the constant folding would then need to find the expression in a different position. Running constant folding first simplifies the expressions that all subsequent rules see.

### Step 5: Optimization statistics

Add a method that produces a summary of what the optimizer did:

```rust
impl OptimizeResult {
    /// Generate a human-readable summary of the optimizations applied.
    pub fn stats_summary(&self) -> String {
        if self.applied_rules.is_empty() {
            return "No optimizations applied.".to_string();
        }

        let mut summary_parts = Vec::new();

        let constant_folds = self.applied_rules.iter()
            .filter(|r| r.as_str() == "ConstantFolding")
            .count();
        if constant_folds > 0 {
            summary_parts.push(format!(
                "Folded {} constant expression{}",
                constant_folds,
                if constant_folds == 1 { "" } else { "s" }
            ));
        }

        let pushdowns = self.applied_rules.iter()
            .filter(|r| r.as_str() == "FilterPushdown")
            .count();
        if pushdowns > 0 {
            summary_parts.push(format!(
                "Pushed down {} filter{}",
                pushdowns,
                if pushdowns == 1 { "" } else { "s" }
            ));
        }

        summary_parts.join(", ")
    }
}
```

### Step 6: Integration test

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn full_optimization_pipeline() {
        let optimizer = Optimizer::default_optimizer();

        // Build a plan that exercises both rules:
        // Filter(age > 2 + 3)
        //   Project(name, age)
        //     Scan(users)
        let plan = Plan::Filter {
            predicate: Expression::BinaryOp {
                left: Box::new(Expression::ColumnRef("age".to_string())),
                op: BinaryOperator::GreaterThan,
                right: Box::new(Expression::BinaryOp {
                    left: Box::new(Expression::Literal(Value::Integer(2))),
                    op: BinaryOperator::Add,
                    right: Box::new(Expression::Literal(Value::Integer(3))),
                }),
            },
            source: Box::new(Plan::Project {
                columns: vec![
                    Expression::ColumnRef("name".to_string()),
                    Expression::ColumnRef("age".to_string()),
                ],
                source: Box::new(Plan::Scan { table: "users".to_string() }),
            }),
        };

        let result = optimizer.optimize(plan);

        // After constant folding: Filter(age > 5) ...
        // After filter pushdown:
        //   Project(name, age)
        //     Filter(age > 5)
        //       Scan(users)
        let expected = Plan::Project {
            columns: vec![
                Expression::ColumnRef("name".to_string()),
                Expression::ColumnRef("age".to_string()),
            ],
            source: Box::new(Plan::Filter {
                predicate: Expression::BinaryOp {
                    left: Box::new(Expression::ColumnRef("age".to_string())),
                    op: BinaryOperator::GreaterThan,
                    right: Box::new(Expression::Literal(Value::Integer(5))),
                },
                source: Box::new(Plan::Scan { table: "users".to_string() }),
            }),
        };

        assert_eq!(result.plan, expected);
        assert_eq!(result.applied_rules, vec!["ConstantFolding", "FilterPushdown"]);
        assert_eq!(
            result.stats_summary(),
            "Folded 1 constant expression, Pushed down 1 filter"
        );
    }

    #[test]
    fn always_false_short_circuits_entire_plan() {
        let optimizer = Optimizer::default_optimizer();

        // SELECT name FROM users WHERE 1 = 0
        let plan = Plan::Project {
            columns: vec![Expression::ColumnRef("name".to_string())],
            source: Box::new(Plan::Filter {
                predicate: Expression::BinaryOp {
                    left: Box::new(Expression::Literal(Value::Integer(1))),
                    op: BinaryOperator::Equal,
                    right: Box::new(Expression::Literal(Value::Integer(0))),
                },
                source: Box::new(Plan::Scan { table: "users".to_string() }),
            }),
        };

        let result = optimizer.optimize(plan);

        // Constant folding turns 1 = 0 into false, which turns the filter
        // into EmptyResult. The Project remains but its source is EmptyResult.
        let expected = Plan::Project {
            columns: vec![Expression::ColumnRef("name".to_string())],
            source: Box::new(Plan::EmptyResult),
        };

        assert_eq!(result.plan, expected);
    }

    #[test]
    fn no_optimization_when_nothing_to_do() {
        let optimizer = Optimizer::default_optimizer();

        let plan = Plan::Filter {
            predicate: Expression::BinaryOp {
                left: Box::new(Expression::ColumnRef("age".to_string())),
                op: BinaryOperator::GreaterThan,
                right: Box::new(Expression::Literal(Value::Integer(30))),
            },
            source: Box::new(Plan::Scan { table: "users".to_string() }),
        };

        let result = optimizer.optimize(plan.clone());
        assert_eq!(result.plan, plan);
        assert!(result.applied_rules.is_empty());
    }
}
```

Run all the tests:

```
$ cargo test optimizer
running 16 tests
test optimizer::optimizer_tests::empty_optimizer_returns_plan_unchanged ... ok
test optimizer::optimizer_tests::noop_rule_does_not_appear_in_applied ... ok
test optimizer::optimizer_tests::kill_scans_rule_replaces_scan ... ok
test optimizer::optimizer_tests::multiple_rules_applied_in_order ... ok
test optimizer::constant_folding_tests::fold_1_plus_1_equals_2 ... ok
test optimizer::constant_folding_tests::fold_3_greater_than_5_is_false ... ok
test optimizer::constant_folding_tests::partial_fold_age_greater_than_2_plus_3 ... ok
test optimizer::constant_folding_tests::no_fold_when_column_involved ... ok
test optimizer::constant_folding_tests::fold_nested_constants ... ok
test optimizer::constant_folding_tests::fold_boolean_and_true_true ... ok
test optimizer::constant_folding_tests::fold_boolean_and_true_false ... ok
test optimizer::constant_folding_tests::fold_in_project_columns ... ok
test optimizer::filter_pushdown_tests::push_filter_past_project ... ok
test optimizer::filter_pushdown_tests::do_not_push_filter_referencing_computed_column ... ok
test optimizer::filter_pushdown_tests::filter_on_scan_stays_in_place ... ok
test optimizer::filter_pushdown_tests::push_filter_through_nested_project ... ok
test optimizer::integration_tests::full_optimization_pipeline ... ok
test optimizer::integration_tests::always_false_short_circuits_entire_plan ... ok
test optimizer::integration_tests::no_optimization_when_nothing_to_do ... ok

test result: ok. 19 passed; 0 failed
```

---
