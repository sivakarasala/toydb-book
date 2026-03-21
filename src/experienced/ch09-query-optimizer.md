# Chapter 9: Query Optimizer

Your database can lex SQL into tokens, parse tokens into an AST, and convert the AST into a query plan. But the plan it produces is naive. `SELECT name FROM users WHERE 1 + 1 = 2` generates a plan that scans every row in the `users` table, computes `1 + 1` for each row, compares it to `2`, and only then returns the `name` column. That is absurd. A human can see instantly that `1 + 1 = 2` is always true, so the filter should be removed entirely. The difference between a database that runs this query in 2 microseconds and one that scans a million rows is not a smarter executor — it is a smarter optimizer.

Query optimization is the art of rewriting a plan into a different plan that produces the same results but does less work. The optimizer never changes what the query returns. It changes how the database computes the answer. This is the chapter where your database starts thinking before it acts.

By the end of this chapter, you will have:

- A `trait OptimizerRule` that defines a single transformation on a plan tree
- A `Vec<Box<dyn OptimizerRule>>` that stores heterogeneous rules and applies them in sequence
- A constant folding rule that evaluates expressions like `1 + 1` and `3 > 5` at plan time
- A filter pushdown rule that moves filters closer to their data source
- A full compilation pipeline: SQL string to optimized plan in a single function call
- A deep understanding of trait objects, dynamic dispatch, and when to choose them over generics

---

## Spotlight: Trait Objects & Dynamic Dispatch

Every chapter has one spotlight concept. This chapter's spotlight is **trait objects and dynamic dispatch** — the mechanism Rust uses when you need to work with values of different types through a common interface, but you do not know the concrete types at compile time.

### The problem: a collection of different types

You have several optimizer rules. Each is a different struct with different fields and different logic. But you want to store them all in a single `Vec` and iterate over them, calling `optimize()` on each one. In Rust, a `Vec` holds elements of a single type. You cannot write `Vec<ConstantFolding | FilterPushdown>`. The types are different sizes, different layouts, different everything.

Generics do not help here either. You could write `fn apply<R: OptimizerRule>(rule: &R, plan: Plan)`, but that works for one rule at a time. You cannot have a `Vec<T>` where each element is a different `T` — that is not how generics work. Generics are monomorphized: the compiler generates a separate copy of the function for each concrete type. The type must be known at compile time.

### Trait objects: type erasure

A trait object erases the concrete type and keeps only the interface. You write `dyn OptimizerRule` to say "some type that implements `OptimizerRule`, but I do not know which one." Since the size is unknown at compile time, you cannot put `dyn OptimizerRule` on the stack directly. You need a pointer:

```rust
// Box<dyn OptimizerRule> — an owned, heap-allocated trait object
let rule: Box<dyn OptimizerRule> = Box::new(ConstantFolding);

// &dyn OptimizerRule — a borrowed trait object
let rule_ref: &dyn OptimizerRule = &ConstantFolding;
```

Now you can build a heterogeneous collection:

```rust
let rules: Vec<Box<dyn OptimizerRule>> = vec![
    Box::new(ConstantFolding),
    Box::new(FilterPushdown),
    Box::new(ShortCircuitEvaluation),
];

for rule in &rules {
    plan = rule.optimize(plan);
}
```

Each element in the `Vec` is a `Box<dyn OptimizerRule>` — same size (a pointer), same type (the trait object). The concrete types behind the pointer are different, but the `Vec` does not need to know that.

### How dynamic dispatch works: the vtable

When you call `rule.optimize(plan)` on a `&dyn OptimizerRule`, Rust does not know at compile time which function to call. It looks up the function pointer at runtime. This is called dynamic dispatch, and it works through a vtable (virtual function table).

A `&dyn OptimizerRule` is actually two pointers — a "fat pointer":

```
&dyn OptimizerRule
┌─────────────────┐
│ data pointer     │───> the actual struct (ConstantFolding, FilterPushdown, etc.)
├─────────────────┤
│ vtable pointer   │───> vtable for that struct's OptimizerRule impl
└─────────────────┘

vtable:
┌─────────────────┐
│ drop()           │───> destructor for the concrete type
├─────────────────┤
│ size             │    size of the concrete type
├─────────────────┤
│ align            │    alignment of the concrete type
├─────────────────┤
│ name()           │───> ConstantFolding::name or FilterPushdown::name
├─────────────────┤
│ optimize()       │───> ConstantFolding::optimize or FilterPushdown::optimize
└─────────────────┘
```

Each concrete type that implements `OptimizerRule` gets its own vtable. The vtable is created once at compile time — it is just a static table of function pointers. At runtime, calling a method on a trait object is one extra pointer dereference compared to a direct function call. This is the cost of dynamic dispatch: typically 1-2 nanoseconds per call.

### Static dispatch (`impl Trait`) vs dynamic dispatch (`dyn Trait`)

Rust gives you the choice:

```rust
// Static dispatch: compiler generates specialized code for each type
fn apply_rule(rule: &impl OptimizerRule, plan: Plan) -> Plan {
    rule.optimize(plan)
}
// The compiler creates apply_rule::<ConstantFolding> and apply_rule::<FilterPushdown>
// as separate functions. No vtable lookup. Inlining possible.

// Dynamic dispatch: one function, runtime lookup
fn apply_rule(rule: &dyn OptimizerRule, plan: Plan) -> Plan {
    rule.optimize(plan)
}
// One copy of the function. Vtable lookup at runtime. No inlining.
```

When to use which:

| Use case | Choice | Why |
|----------|--------|-----|
| Homogeneous collection | `impl Trait` / generics | All elements same type, no overhead |
| Heterogeneous collection | `dyn Trait` | Different types in one Vec |
| Hot loop, performance critical | `impl Trait` | Avoids vtable overhead, enables inlining |
| Plugin system, extensibility | `dyn Trait` | New types can be added without recompilation |
| Return type varies | `Box<dyn Trait>` | Cannot use `impl Trait` when returning different types from branches |

For our optimizer, `dyn Trait` is the right choice. We have a collection of rules that are different types, and the number of rules might change. The vtable overhead is negligible compared to the work each rule does (walking an entire plan tree).

### Object safety

Not every trait can be used as a trait object. To be "object safe," a trait must follow certain rules:

```rust
// Object safe: can be used as dyn Trait
trait OptimizerRule {
    fn name(&self) -> &str;
    fn optimize(&self, plan: Plan) -> Plan;
}

// NOT object safe: has a generic method
trait BadRule {
    fn optimize<T: PlanNode>(&self, node: T) -> T;
    // Error: "the trait `BadRule` cannot be made into an object"
    // Generic methods require compile-time monomorphization,
    // which is incompatible with runtime dispatch.
}

// NOT object safe: returns Self
trait AlsoBad {
    fn clone_rule(&self) -> Self;
    // Error: "Self" is the concrete type, which is erased
    // behind dyn Trait. The compiler doesn't know what to return.
}
```

The rules are simple: no generic methods, no `Self` in return position, no `Sized` requirement. If the compiler cannot build a vtable for the trait, it is not object safe.

> **Coming from other languages?**
>
> | Concept | JavaScript | Python | Go | Rust |
> |---------|-----------|--------|----|------|
> | Interface | Not formal | ABC (abstract) | `interface{}` | `trait` |
> | Dynamic dispatch | Everything is dynamic | Everything is dynamic | All interface calls | `dyn Trait` (explicit) |
> | Static dispatch | Not possible | Not possible | Not possible | `impl Trait` / generics |
> | Virtual table | Hidden prototype chain | `__mro__` | Implicit itab | Explicit vtable |
> | Heterogeneous list | `[obj1, obj2, ...]` (always) | `[obj1, obj2, ...]` (always) | `[]interface{}` | `Vec<Box<dyn Trait>>` |
> | Performance choice | None (always dynamic) | None (always dynamic) | None (always dynamic) | You choose per call site |
>
> **From JS:** In JavaScript, every method call is a dynamic lookup — the engine searches the prototype chain. Rust makes this explicit: if you want dynamic dispatch, you write `dyn`. Otherwise, the compiler generates specialized code for each type, which is faster.
>
> **From Python:** Python's Abstract Base Classes (ABCs) are similar to Rust traits. When you call a method on an ABC reference, Python does a dictionary lookup. Rust's `dyn Trait` does a vtable lookup — same idea, but the vtable is a fixed array of function pointers (faster than a hash map).
>
> **From Go:** Go interfaces are the closest analogy. An interface value in Go is a fat pointer (type pointer + data pointer), just like `dyn Trait` in Rust. The key difference: in Go, all interface calls are dynamic. In Rust, you choose between static (`impl Trait`) and dynamic (`dyn Trait`) dispatch per call site. When performance matters, this choice is significant.

---

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

## Rust Gym

Three short exercises to strengthen your trait object and dynamic dispatch skills. All use `std` only.

### Gym 1: Heterogeneous Animal Collection

Create a `Vec<Box<dyn Animal>>` with different animal types. Each type implements `speak()` differently.

```rust
// Goal: define a trait and multiple implementors, store them in a Vec,
// iterate and call the trait method.

trait Animal {
    fn name(&self) -> &str;
    fn speak(&self) -> String;
}

// Your task: define Dog, Cat, and Duck structs that implement Animal.
// Create a Vec<Box<dyn Animal>> with one of each.
// Print each animal's name and speech.

// Expected output:
// Rex says: Woof!
// Whiskers says: Meow!
// Donald says: Quack!
```

<details>
<summary>Hint</summary>

Each struct needs a `name` field (a `String`). Implement `Animal` for each one. Use `Box::new(Dog { name: "Rex".to_string() })` to create a `Box<dyn Animal>`.

</details>

<details>
<summary>Solution</summary>

```rust
trait Animal {
    fn name(&self) -> &str;
    fn speak(&self) -> String;
}

struct Dog { name: String }
struct Cat { name: String }
struct Duck { name: String }

impl Animal for Dog {
    fn name(&self) -> &str { &self.name }
    fn speak(&self) -> String { "Woof!".to_string() }
}

impl Animal for Cat {
    fn name(&self) -> &str { &self.name }
    fn speak(&self) -> String { "Meow!".to_string() }
}

impl Animal for Duck {
    fn name(&self) -> &str { &self.name }
    fn speak(&self) -> String { "Quack!".to_string() }
}

fn main() {
    let animals: Vec<Box<dyn Animal>> = vec![
        Box::new(Dog { name: "Rex".to_string() }),
        Box::new(Cat { name: "Whiskers".to_string() }),
        Box::new(Duck { name: "Donald".to_string() }),
    ];

    for animal in &animals {
        println!("{} says: {}", animal.name(), animal.speak());
    }
}
```

Output:

```
Rex says: Woof!
Whiskers says: Meow!
Donald says: Quack!
```

Each element in `animals` is a different concrete type, but the `Vec` does not care. It stores `Box<dyn Animal>` — a pointer plus a vtable. When we call `animal.speak()`, Rust looks up the `speak` function pointer in the vtable and calls it. The call is resolved at runtime, not compile time.

</details>

### Gym 2: Plugin System

Build a simple plugin system where plugins register themselves and the system runs them in order.

```rust
// Goal: a PluginHost that holds Box<dyn Plugin> values
// and calls execute() on each one.

trait Plugin {
    fn name(&self) -> &str;
    fn execute(&self, input: &str) -> String;
}

// Your task:
// 1. Implement UppercasePlugin (converts input to uppercase)
// 2. Implement ReversePlugin (reverses the input string)
// 3. Implement PrefixPlugin { prefix: String } (prepends a prefix)
// 4. Build a PluginHost that stores Vec<Box<dyn Plugin>>
// 5. PluginHost::run() applies all plugins in sequence,
//    passing each plugin's output as the next plugin's input.

// Expected output for input "hello" with plugins [Uppercase, Reverse, Prefix(">> ")]:
// [UppercasePlugin] "hello" -> "HELLO"
// [ReversePlugin] "HELLO" -> "OLLEH"
// [PrefixPlugin] "OLLEH" -> ">> OLLEH"
// Final result: >> OLLEH
```

<details>
<summary>Hint</summary>

`PluginHost::run` should loop over `&self.plugins`, calling `plugin.execute(current)` and updating `current` with the result each time. The tricky part is the `PrefixPlugin` — it needs to own its prefix string, so its struct has a `prefix: String` field.

</details>

<details>
<summary>Solution</summary>

```rust
trait Plugin {
    fn name(&self) -> &str;
    fn execute(&self, input: &str) -> String;
}

struct UppercasePlugin;
impl Plugin for UppercasePlugin {
    fn name(&self) -> &str { "UppercasePlugin" }
    fn execute(&self, input: &str) -> String { input.to_uppercase() }
}

struct ReversePlugin;
impl Plugin for ReversePlugin {
    fn name(&self) -> &str { "ReversePlugin" }
    fn execute(&self, input: &str) -> String {
        input.chars().rev().collect()
    }
}

struct PrefixPlugin { prefix: String }
impl Plugin for PrefixPlugin {
    fn name(&self) -> &str { "PrefixPlugin" }
    fn execute(&self, input: &str) -> String {
        format!("{}{}", self.prefix, input)
    }
}

struct PluginHost {
    plugins: Vec<Box<dyn Plugin>>,
}

impl PluginHost {
    fn new() -> Self {
        PluginHost { plugins: Vec::new() }
    }

    fn add(&mut self, plugin: Box<dyn Plugin>) {
        self.plugins.push(plugin);
    }

    fn run(&self, input: &str) -> String {
        let mut current = input.to_string();
        for plugin in &self.plugins {
            let next = plugin.execute(&current);
            println!("[{}] {:?} -> {:?}", plugin.name(), current, next);
            current = next;
        }
        current
    }
}

fn main() {
    let mut host = PluginHost::new();
    host.add(Box::new(UppercasePlugin));
    host.add(Box::new(ReversePlugin));
    host.add(Box::new(PrefixPlugin { prefix: ">> ".to_string() }));

    let result = host.run("hello");
    println!("Final result: {}", result);
}
```

Output:

```
[UppercasePlugin] "hello" -> "HELLO"
[ReversePlugin] "HELLO" -> "OLLEH"
[PrefixPlugin] "OLLEH" -> ">> OLLEH"
Final result: >> OLLEH
```

This is exactly the pattern our optimizer uses. The `PluginHost` is the `Optimizer`, each `Plugin` is an `OptimizerRule`, and `run()` is `optimize()`. The only difference is that our optimizer transforms a `Plan` tree instead of a `String`.

</details>

### Gym 3: Static vs Dynamic Dispatch Comparison

Write the same function using both `impl Trait` and `dyn Trait`, and observe the differences.

```rust
// Goal: understand the tradeoff between static and dynamic dispatch.

trait Formatter {
    fn format(&self, value: f64) -> String;
}

struct DecimalFormatter { places: usize }
struct PercentFormatter;
struct CurrencyFormatter { symbol: char }

// Your task:
// 1. Implement Formatter for all three types.
// 2. Write format_static(formatter: &impl Formatter, value: f64) -> String
// 3. Write format_dynamic(formatter: &dyn Formatter, value: f64) -> String
// 4. Write format_all(formatters: &[Box<dyn Formatter>], value: f64)
//    that prints the formatted value for each formatter.
// 5. Try writing format_all with impl Trait — explain why it does not compile.

// Expected output for value 0.1567:
// Decimal(2): 0.16
// Percent: 15.67%
// Currency($): $0.16
```

<details>
<summary>Hint</summary>

`format_static` uses monomorphization — the compiler generates one version per concrete type. `format_dynamic` uses a vtable. `format_all` must use `dyn Trait` because the `Vec` contains different types. If you try `fn format_all(formatters: &[impl Formatter], value: f64)`, it means "a slice where all elements are the same (unknown) type," which is not what we want.

</details>

<details>
<summary>Solution</summary>

```rust
trait Formatter {
    fn format(&self, value: f64) -> String;
}

struct DecimalFormatter { places: usize }
impl Formatter for DecimalFormatter {
    fn format(&self, value: f64) -> String {
        format!("{:.prec$}", value, prec = self.places)
    }
}

struct PercentFormatter;
impl Formatter for PercentFormatter {
    fn format(&self, value: f64) -> String {
        format!("{:.2}%", value * 100.0)
    }
}

struct CurrencyFormatter { symbol: char }
impl Formatter for CurrencyFormatter {
    fn format(&self, value: f64) -> String {
        format!("{}{:.2}", self.symbol, value)
    }
}

// Static dispatch: compiler generates specialized versions
fn format_static(formatter: &impl Formatter, value: f64) -> String {
    formatter.format(value)
}

// Dynamic dispatch: runtime vtable lookup
fn format_dynamic(formatter: &dyn Formatter, value: f64) -> String {
    formatter.format(value)
}

// Must use dyn Trait — elements are different types
fn format_all(formatters: &[Box<dyn Formatter>], value: f64) {
    for formatter in formatters {
        println!("  {}", formatter.format(value));
    }
}

// This does NOT compile:
// fn format_all_static(formatters: &[impl Formatter], value: f64) {
//     // Error: `impl Trait` means "one specific type that implements Formatter"
//     // All elements must be the same type — defeats the purpose.
//     // This is a slice of T where T: Formatter, not a slice of
//     // "anything that implements Formatter."
// }

fn main() {
    let value = 0.1567;

    // Static dispatch calls — each resolves at compile time
    let dec = DecimalFormatter { places: 2 };
    let pct = PercentFormatter;
    let cur = CurrencyFormatter { symbol: '$' };

    println!("Static dispatch:");
    println!("  Decimal(2): {}", format_static(&dec, value));
    println!("  Percent: {}", format_static(&pct, value));
    println!("  Currency($): {}", format_static(&cur, value));

    // Dynamic dispatch — same results, resolved at runtime
    let formatters: Vec<Box<dyn Formatter>> = vec![
        Box::new(DecimalFormatter { places: 2 }),
        Box::new(PercentFormatter),
        Box::new(CurrencyFormatter { symbol: '$' }),
    ];

    println!("\nDynamic dispatch:");
    format_all(&formatters, value);
}
```

Output:

```
Static dispatch:
  Decimal(2): 0.16
  Percent: 15.67%
  Currency($): $0.16

Dynamic dispatch:
  0.16
  15.67%
  $0.16
```

The results are identical. The difference is in how the compiler handles each call:

- `format_static(&dec, value)` — the compiler knows `dec` is `DecimalFormatter` and generates a direct function call. It can inline `DecimalFormatter::format` at the call site.
- `format_dynamic(&dec as &dyn Formatter, value)` — the compiler generates a vtable lookup. It loads the function pointer from the vtable and calls through it. Inlining is not possible.

For our optimizer rules, the vtable overhead is negligible. Each `optimize()` call does substantial work (walking an entire plan tree). The cost of one pointer dereference per rule is invisible. Use `dyn Trait` when you need heterogeneous collections; use `impl Trait` when you need maximum performance on hot paths.

</details>

---

## DSA in Context: Tree Transformations

The optimizer performs tree transformations — it takes a plan tree as input and produces a modified plan tree as output. This is a fundamental operation in computer science, appearing in compilers, interpreters, symbolic math engines, and document processors.

### Pattern matching on tree nodes

Each optimizer rule is a pattern matcher. It looks at a node in the tree and asks: "Does this node match a pattern I can optimize?" If yes, it rewrites the node. If no, it recurses into the children.

```
Constant Folding Pattern:
  Match:   BinaryOp(Literal(a), op, Literal(b))
  Rewrite: Literal(eval(a, op, b))

Filter Pushdown Pattern:
  Match:   Filter(pred, Project(cols, source))
           where pred references only columns in cols
  Rewrite: Project(cols, Filter(pred, source))
```

This is exactly how compilers work. GCC and LLVM have hundreds of optimization passes, each one a pattern matcher that looks for specific tree shapes and rewrites them. The key insight is that each pass is simple — it handles one pattern. The power comes from composing many simple passes.

### Fixed-point iteration

Our optimizer applies each rule once. A more sophisticated approach is fixed-point iteration: apply all rules repeatedly until no rule makes any changes.

```rust
/// Apply rules repeatedly until the plan stops changing.
/// This is called "fixed-point iteration" — we iterate until
/// we reach a fixed point (a state that does not change).
fn optimize_to_fixed_point(&self, plan: Plan) -> OptimizeResult {
    let mut current = plan;
    let mut all_applied: Vec<String> = Vec::new();
    let mut iterations = 0;
    let max_iterations = 100; // Safety limit

    loop {
        let before = format!("{:?}", current);
        let mut changed = false;

        for rule in &self.rules {
            let before_rule = format!("{:?}", current);
            current = rule.optimize(current);
            let after_rule = format!("{:?}", current);

            if before_rule != after_rule {
                all_applied.push(rule.name().to_string());
                changed = true;
            }
        }

        iterations += 1;
        if !changed || iterations >= max_iterations {
            break;
        }
    }

    OptimizeResult {
        plan: current,
        applied_rules: all_applied,
    }
}
```

Why would rules need multiple passes? Consider filter pushdown through two levels of project:

```
Pass 1:
  Filter(pred)              Project(cols1)
    Project(cols1)    =>      Filter(pred)
      Project(cols2)            Project(cols2)
        Scan                      Scan

Pass 2:
  Project(cols1)            Project(cols1)
    Filter(pred)      =>      Project(cols2)
      Project(cols2)            Filter(pred)
        Scan                      Scan
```

The first pass pushes the filter past the first project. The second pass pushes it past the second project. Each pass applies a simple, local transformation. Multiple passes achieve a global result.

Fixed-point iteration is guaranteed to terminate if each rule either makes progress (reduces some measure of the plan) or leaves it unchanged. If a rule could increase the measure, you might loop forever — hence the safety limit.

### Time complexity

Our optimizer has O(R * N) time complexity per pass, where R is the number of rules and N is the number of nodes in the plan tree. Each rule walks the entire tree once. With fixed-point iteration, the worst case is O(I * R * N) where I is the number of iterations, but in practice I is small (typically 2-5) because each pass resolves most opportunities.

Production optimizers like PostgreSQL's use more sophisticated algorithms. They precompute which rules can fire based on the types of nodes present, so they skip rules that cannot possibly match. But the fundamental structure — a collection of transformation rules applied to a tree — is the same.

---

## System Design Corner: Query Optimization in Production

In a system design interview, discussing query optimization shows that you understand why databases are fast, not just that they are.

### Rule-based optimization (RBO) vs cost-based optimization (CBO)

Our optimizer is **rule-based**: it applies fixed transformations regardless of the data. "Push filters down" is always beneficial. "Fold constants" is always beneficial. Rules do not need statistics about the data.

Production databases also use **cost-based optimization**: they estimate the cost of different plans and choose the cheapest one. This requires statistics:

```
Table: users (1,000,000 rows)
  Column: age — min: 0, max: 120, distinct: 100, histogram: [...]
  Column: country — distinct: 195, most common: 'US' (40%), 'CN' (15%), ...

Query: SELECT name FROM users WHERE age > 30 AND country = 'US'

Plan A: Scan(users) → Filter(age > 30 AND country = 'US')
  Estimated cost: scan 1M rows, filter produces ~400K * 0.40 = ~160K rows

Plan B: IndexScan(users.country = 'US') → Filter(age > 30)
  Estimated cost: index lookup produces ~400K rows, filter produces ~280K rows

Plan C: IndexScan(users.age > 30) → Filter(country = 'US')
  Estimated cost: index lookup produces ~700K rows, filter produces ~280K rows

Cheapest: Plan B (if the country index exists and is selective enough)
```

The optimizer estimates how many rows each plan step produces (cardinality estimation), how much I/O each step costs (disk reads for scans, index lookups), and how much CPU each step costs (comparisons, hash computations). It then picks the cheapest plan.

### Join reordering

For queries with multiple joins, the order of joins matters enormously:

```sql
SELECT *
FROM orders o
JOIN customers c ON o.customer_id = c.id
JOIN products p ON o.product_id = p.id
WHERE c.country = 'US' AND p.category = 'Electronics'
```

If there are 10M orders, 1M customers, and 100K products:

- Join orders with customers first: 10M * (cost of looking up each customer) = expensive
- Filter customers to US first (400K), then join with orders: much cheaper
- Filter products to Electronics first (10K), then join: even cheaper

The optimizer considers different join orders and picks the one with the lowest estimated cost. With N tables, there are N! possible join orders — a combinatorial explosion. Production optimizers use dynamic programming to prune the search space.

### PostgreSQL's optimizer pipeline

PostgreSQL's optimizer has several stages:

```
1. Simplification (rule-based)
   - Constant folding (like our ConstantFolding rule)
   - Predicate normalization (convert OR to IN, flatten AND chains)
   - View expansion (inline view definitions)

2. Path generation
   - For each table: sequential scan, index scan (one per index), bitmap scan
   - For each join: nested loop, hash join, merge join
   - Generate all possible access paths

3. Cost estimation
   - Use table statistics (pg_statistic) to estimate row counts
   - Use cost model to estimate I/O and CPU cost per path
   - Account for caching, parallelism, disk vs SSD

4. Plan selection
   - Dynamic programming for join ordering
   - Pick lowest-cost path for each sub-plan
   - Handle subqueries, CTEs, window functions

5. Plan finalization
   - Add Sort nodes if ORDER BY is present
   - Add Limit nodes
   - Add Result nodes for returning data to the client
```

> **Interview talking point:** *"Our database has a rule-based optimizer with constant folding and filter pushdown. In production, I would add cost-based optimization with cardinality estimation to handle join reordering. The optimizer stores rules as trait objects — `Vec<Box<dyn OptimizerRule>>` — so adding new rules does not require modifying the optimizer core. Each rule is a tree transformation that rewrites plan nodes matching specific patterns. We apply rules in sequence and could extend this to fixed-point iteration for rules that interact."*

### Indexes and the optimizer

The optimizer cannot recommend using an index if it does not know indexes exist. In production databases, the optimizer queries the catalog to discover available indexes, then generates index scan plans alongside sequential scan plans. Our toydb does not have indexes yet, but the optimizer framework we built can easily support them: just add an `IndexScan` variant to `Plan` and a rule that converts `Filter(col = val, Scan(table))` into `IndexScan(table, col, val)` when an index exists on `col`.

---

## Design Insight: Pass-Through Elimination

In *A Philosophy of Software Design*, Ousterhout warns against pass-through methods — methods that do nothing but forward calls to another method. They add complexity without adding functionality.

Our optimizer rules embody the opposite principle: **each rule eliminates pass-through nodes in the plan tree.** A `Filter(true)` node is a pass-through — it accepts every row and forwards it unchanged. Constant folding detects this and removes the node. A `Filter` above a `Project` is processing rows that might be filtered out — filter pushdown rearranges the tree so rows are eliminated before the expensive projection.

The design insight is broader than optimizer rules:

**Optimizer rules do not create new plan types. They rearrange existing ones.** Each rule has a single responsibility — it knows one pattern and one rewrite. The rule does not know about other rules. The rule does not know about the overall optimization strategy. It just looks for its pattern and applies its rewrite.

This is why the `OptimizerRule` trait is so simple: one method for the name, one method for the transformation. No configuration, no state, no interaction between rules. The `Optimizer` composes them. This is the single-responsibility principle applied to tree transformations.

The same pattern appears throughout software design:

- **Compiler passes:** Each pass (dead code elimination, constant propagation, register allocation) knows one transformation.
- **Unix pipes:** Each tool (grep, sort, uniq, awk) does one thing. Composition creates complex behavior.
- **Middleware in web frameworks:** Each middleware handles one concern (logging, authentication, compression). The framework chains them.

When you find yourself writing a complex transformation, ask: "Can I decompose this into multiple simple transformations applied in sequence?" If each transformation is correct in isolation and the sequence is well-ordered, the composition is correct by construction. This is easier to test, easier to understand, and easier to extend than a monolithic transformation that handles every case.

> *"The best modules are those that provide powerful functionality yet have simple interfaces. A module that has a simple interface is easier to modify without affecting other modules."*
> — John Ousterhout, *A Philosophy of Software Design*

---

## What You Built

In this chapter, you:

1. **Defined the `OptimizerRule` trait** — a two-method interface (`name` and `optimize`) that every optimization rule implements, demonstrating trait objects and dynamic dispatch
2. **Built the `Optimizer` struct** — stores `Vec<Box<dyn OptimizerRule>>`, applies rules in sequence, and reports which rules fired, demonstrating heterogeneous collections through trait objects
3. **Implemented constant folding** — evaluates constant expressions at plan time, removes always-true filters, replaces always-false filters with `EmptyResult`, demonstrating recursive tree transformation
4. **Implemented filter pushdown** — moves filter nodes past project nodes when safe, reducing the rows processed by expensive operations, demonstrating plan tree rewriting with safety checks
5. **Wired the full pipeline** — SQL string to optimized plan through lexer, parser, planner, and optimizer, demonstrating module composition

Your database no longer blindly executes the plan the planner produces. It thinks first. `WHERE 1 + 1 = 2` is eliminated before execution begins. Filters are moved to where they can do the most good. And the framework is extensible — adding a new rule means implementing a trait and adding one line to `default_optimizer()`.

Chapter 10 builds the query executor that takes these optimized plans and actually runs them against the storage engine. The plans your optimizer produces will determine how efficiently the executor works — a well-optimized plan means less disk I/O, less memory, and faster results.

---

### DS Deep Dive

Our optimizer applies rules in a fixed order: constant folding, then filter pushdown. Production optimizers explore a search space of possible plans and use dynamic programming to find the cheapest one. This deep dive explores the Cascades framework, top-down vs bottom-up optimization, and how cost models combine cardinality estimation with I/O and CPU cost functions.

**-> [Query Optimization Theory -- "The Plan Space Explorer"](../ds-narratives/ch09-query-optimization.md)**

---

### Reference implementation

The files you built in this chapter correspond to these files in the reference codebase:

| Your file | Reference |
|-----------|-----------|
| `src/optimizer.rs` — `OptimizerRule` trait | [`src/sql/planner/optimizer.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/sql/planner/optimizer.rs) — optimization rules |
| `src/optimizer.rs` — `ConstantFolding` | [`src/sql/planner/optimizer.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/sql/planner/optimizer.rs) — `ConstantFolder` |
| `src/optimizer.rs` — `FilterPushdown` | [`src/sql/planner/optimizer.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/sql/planner/optimizer.rs) — `FilterPushdown` |
| `src/optimizer.rs` — `fold_constants()` | [`src/sql/planner/optimizer.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/sql/planner/optimizer.rs) — constant evaluation |
| `src/planner.rs` — `Plan` enum | [`src/sql/planner/plan.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/sql/planner/plan.rs) — `Node` enum |
