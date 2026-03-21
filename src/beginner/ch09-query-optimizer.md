# Chapter 9: Query Optimizer

In the last chapter, we built a query planner. Given `SELECT name FROM users WHERE age > 18`, the planner can produce a plan tree: Project(Filter(Scan)). But the plans it produces are naive. Consider this query:

```sql
SELECT name FROM users WHERE 1 + 1 = 2
```

The planner creates a filter that, for every single row in the `users` table, computes `1 + 1`, compares the result to `2`, and only then decides whether to keep the row. If your table has a million rows, that is a million additions and a million comparisons -- all for an expression that is always true. A human can see instantly that `1 + 1 = 2` is always true, so the filter should be removed entirely.

This is what an optimizer does. It rewrites a plan into a different plan that produces the exact same results but does less work. Think of it like a GPS rerouting you around traffic -- you end up at the same destination, but you get there faster because you took a smarter route.

This chapter builds a query optimizer. Along the way, you will learn one of Rust's most powerful features: **trait objects and dynamic dispatch**. This is how Rust lets you store different types in the same collection and call methods on them without knowing the concrete type at compile time.

By the end of this chapter, you will have:

- A `trait OptimizerRule` that defines a single transformation on a plan tree
- A `Vec<Box<dyn OptimizerRule>>` that stores different rules and applies them in sequence
- A constant folding rule that evaluates expressions like `1 + 1` at plan time
- A filter pushdown rule that moves filters closer to their data source
- A deep understanding of trait objects, `Box<dyn Trait>`, static vs dynamic dispatch, and vtables

---

## Spotlight: Trait Objects & Dynamic Dispatch

Every chapter has one **spotlight concept** -- the Rust idea we dig into deeply. This chapter's spotlight is **trait objects and dynamic dispatch**.

### Quick recap: what is a trait?

A trait is a contract. It says "any type that implements me must provide these methods." We have seen traits before:

```rust
trait Greeter {
    fn greet(&self) -> String;
}

struct EnglishGreeter;
struct SpanishGreeter;

impl Greeter for EnglishGreeter {
    fn greet(&self) -> String {
        "Hello!".to_string()
    }
}

impl Greeter for SpanishGreeter {
    fn greet(&self) -> String {
        "Hola!".to_string()
    }
}
```

Both `EnglishGreeter` and `SpanishGreeter` implement `Greeter`. They both have a `greet()` method. But they are different types -- different structs with potentially different sizes and internal data.

### The problem: different types in one collection

Here is the situation that leads to trait objects. You want to store multiple greeters in a single `Vec`:

```rust,ignore
// THIS DOES NOT COMPILE
let greeters = vec![
    EnglishGreeter,
    SpanishGreeter,  // Error: expected EnglishGreeter, found SpanishGreeter
];
```

A `Vec` in Rust holds elements of exactly one type. If you put an `EnglishGreeter` first, Rust infers the type as `Vec<EnglishGreeter>`. Then it refuses the `SpanishGreeter` because it is a different type.

This is frustrating because both types can `greet()`. You want to say "give me a Vec of anything that can greet." That is exactly what trait objects let you do.

### What is a trait object?

A trait object is a way to erase the concrete type and keep only the interface. You write `dyn Greeter` to mean "some type that implements Greeter, but I do not know which one."

The word `dyn` is short for "dynamic" -- it tells Rust "we will figure out which actual type this is at runtime, not at compile time."

```rust,ignore
// dyn Greeter means "some unknown type that implements Greeter"
let greeter: dyn Greeter = ???;
```

But there is a problem. Rust needs to know the size of every value at compile time so it can allocate the right amount of stack space. An `EnglishGreeter` might be 0 bytes (it has no fields). A future `ConfigurableGreeter` might be 200 bytes (with lots of configuration data). The compiler cannot allocate space for `dyn Greeter` on the stack because it does not know how big the concrete type is.

This is a fundamental difference from languages like Python or JavaScript, where all values are pointers to heap-allocated objects. In Rust, values live on the stack by default, and the compiler must know their size.

### Why we need Box: unknown sizes on the heap

The solution is `Box`. A `Box<T>` is a pointer to a value on the heap. The `Box` itself is always the same size -- 8 bytes on a 64-bit system (just a pointer). The actual data lives on the heap, where sizes do not need to be known at compile time.

Think of `Box` like a shipping label on a box at a warehouse. The label (the `Box` pointer) is always the same size. The actual package inside could be a paperback book or a refrigerator -- the label does not care.

```rust,ignore
// Box<dyn Greeter> -- always 8 bytes (a pointer)
// Points to the actual struct on the heap
let greeter: Box<dyn Greeter> = Box::new(EnglishGreeter);
```

### Putting it together: a Vec of trait objects

Now we can store different types in the same Vec:

```rust
trait Greeter {
    fn greet(&self) -> String;
}

struct EnglishGreeter;
struct SpanishGreeter;

impl Greeter for EnglishGreeter {
    fn greet(&self) -> String {
        "Hello!".to_string()
    }
}

impl Greeter for SpanishGreeter {
    fn greet(&self) -> String {
        "Hola!".to_string()
    }
}

fn main() {
    // Each element is a Box<dyn Greeter> -- same size (a pointer)
    let greeters: Vec<Box<dyn Greeter>> = vec![
        Box::new(EnglishGreeter),
        Box::new(SpanishGreeter),
    ];

    for greeter in &greeters {
        println!("{}", greeter.greet());
    }
}
```

Let us trace through what happens step by step:

1. `Box::new(EnglishGreeter)` allocates an `EnglishGreeter` on the heap and returns a `Box<EnglishGreeter>`.
2. Rust coerces `Box<EnglishGreeter>` into `Box<dyn Greeter>` -- it "erases" the concrete type but remembers the `Greeter` interface.
3. `Box::new(SpanishGreeter)` does the same for `SpanishGreeter`.
4. The `Vec` holds two `Box<dyn Greeter>` values. Both are the same size (a pointer), even though the concrete types behind them are different.
5. When we call `greeter.greet()`, Rust figures out at runtime which `greet()` function to call.

> **What just happened?**
>
> We put different types into the same `Vec` by erasing their concrete type and keeping only the trait interface. `Box<dyn Trait>` is the pattern: `Box` handles the unknown size (by putting the value on the heap), and `dyn Trait` handles the unknown type (by using runtime lookup to call the right method). Think of it like a numbered ticket at a help desk -- you do not know which agent is behind the counter, but you know they can all help you because they all work at the help desk (implement the trait).

### How does Rust call the right method? The vtable

When you call `greeter.greet()` on a `Box<dyn Greeter>`, Rust does not know at compile time whether this is an `EnglishGreeter` or a `SpanishGreeter`. So how does it call the right function?

The answer is a **vtable** (virtual function table). A vtable is a small table of function pointers that Rust creates at compile time for each concrete type that implements a trait.

When Rust creates a `Box<dyn Greeter>`, it actually stores two pointers side by side. This is called a "fat pointer":

```
Box<dyn Greeter> — a "fat pointer" (two pointers side by side)
┌─────────────────┐
│ data pointer     │───> the actual struct on the heap
├─────────────────┤
│ vtable pointer   │───> vtable for that struct's Greeter impl
└─────────────────┘

vtable for EnglishGreeter:
┌─────────────────┐
│ drop()           │───> how to clean up EnglishGreeter
├─────────────────┤
│ size             │    0 bytes (no fields)
├─────────────────┤
│ greet()          │───> EnglishGreeter::greet
└─────────────────┘

vtable for SpanishGreeter:
┌─────────────────┐
│ drop()           │───> how to clean up SpanishGreeter
├─────────────────┤
│ size             │    0 bytes (no fields)
├─────────────────┤
│ greet()          │───> SpanishGreeter::greet
└─────────────────┘
```

When you call `greeter.greet()`:
1. Rust follows the vtable pointer to find the vtable
2. It looks up the `greet()` entry in the vtable
3. It calls the function pointer stored there, passing the data pointer as `&self`

This lookup happens at runtime. That is why it is called **dynamic dispatch** -- the decision of which function to call is made dynamically (at runtime) rather than statically (at compile time).

The cost is one extra pointer dereference per method call -- typically 1-2 nanoseconds. For our optimizer (which runs once per query, not once per row), this cost is completely irrelevant.

### Static dispatch vs dynamic dispatch

Rust gives you a choice between two kinds of dispatch. This is unusual -- most languages only give you one.

**Static dispatch** with `impl Trait`:

```rust,ignore
// Static dispatch: the compiler generates a separate copy
// of this function for each concrete type
fn print_greeting(greeter: &impl Greeter) {
    println!("{}", greeter.greet());
}

// When you call print_greeting(&EnglishGreeter),
// the compiler creates print_greeting_for_EnglishGreeter.
// When you call print_greeting(&SpanishGreeter),
// it creates print_greeting_for_SpanishGreeter.
// Both are direct function calls -- no vtable lookup.
```

**Dynamic dispatch** with `dyn Trait`:

```rust,ignore
// Dynamic dispatch: one copy of this function,
// vtable lookup at runtime
fn print_greeting_dyn(greeter: &dyn Greeter) {
    println!("{}", greeter.greet());
}

// One function handles all types.
// It uses the vtable to find the right greet() at runtime.
```

Think of it this way:
- **Static dispatch** is like calling someone by name -- "Hey Alice, greet the customer!" You know exactly who to call. No lookup needed.
- **Dynamic dispatch** is like calling "whoever is on duty, greet the customer!" You check the schedule (vtable) to find out who is on duty.

When to use which?

| Situation | Use | Why |
|-----------|-----|-----|
| All items same type | `impl Trait` / generics | No overhead, compiler optimizes |
| Different types in one collection | `Box<dyn Trait>` | Only way to mix types in a Vec |
| Performance-critical inner loop | `impl Trait` / generics | Avoids vtable lookup, allows inlining |
| Plugin system, extensible rules | `Box<dyn Trait>` | New types without changing existing code |

For our optimizer, `Box<dyn Trait>` is the right choice. We have different rule types (constant folding, filter pushdown) and we want to store them all in a single `Vec`. Each rule runs once per query, so the vtable overhead is negligible.

### Object safety: not every trait can become dyn

There are a few rules about which traits can be used with `dyn`. A trait must be "object safe" to be used as a trait object:

```rust,ignore
// Object safe -- CAN be used as dyn Trait
trait OptimizerRule {
    fn name(&self) -> &str;
    fn optimize(&self, plan: Plan) -> Plan;
}

// NOT object safe -- has a generic method
trait BadRule {
    fn optimize<T>(&self, node: T) -> T;
    // Error! Generic methods need to know T at compile time,
    // but dyn dispatch happens at runtime. Incompatible.
}

// NOT object safe -- returns Self
trait AlsoBad {
    fn clone_rule(&self) -> Self;
    // Error! Behind dyn, we do not know what "Self" is.
    // The concrete type has been erased.
}
```

The rules are:
- No generic methods (they need compile-time type information)
- No `Self` in return position (the concrete type is erased)

If the compiler cannot build a vtable for the trait, it is not object safe.

> **What just happened?**
>
> A vtable is a fixed, finite table of function pointers. If a method is generic, the compiler would need a vtable entry for every possible type `T` -- that is infinite, which is impossible. If a method returns `Self`, the compiler does not know how big the return value is, because `Self` could be any concrete type. Both cases prevent the compiler from building a vtable, so both are forbidden.

---

## Exercise 1: The Optimizer Trait and Framework

**Goal:** Define the `OptimizerRule` trait, build the `Optimizer` struct that holds a collection of rules, and apply them in sequence to a plan tree.

### Step 1: Prerequisites from previous chapters

Before we build the optimizer, we need the plan types from Chapter 8. Here is the subset we will work with. If you already have these in your codebase, you can skip this step. Otherwise, make sure these types exist:

```rust
/// A value in an expression
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Integer(i64),
    Float(f64),
    String(String),
    Boolean(bool),
    Null,
}

/// An expression -- the building block of filters and projections
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
    Add, Subtract, Multiply, Divide,
    Equal, NotEqual, LessThan, GreaterThan, LessOrEqual, GreaterOrEqual,
    And, Or,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOperator {
    Not,
    Negate,
}

/// A query plan node -- a tree of operations
#[derive(Debug, Clone, PartialEq)]
pub enum Plan {
    /// Scan all rows from a table
    Scan { table: String },
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

### Step 2: Create the optimizer module

Create `src/optimizer.rs` and register it in your `src/lib.rs`:

```rust
// src/lib.rs -- add this line
pub mod optimizer;
```

### Step 3: Define the OptimizerRule trait

Now define the trait that all optimization rules will implement:

```rust
// src/optimizer.rs

use crate::planner::{Plan, Expression, Value, BinaryOperator, UnaryOperator};

/// A single optimization rule that transforms a plan tree.
///
/// Think of each rule as a specialist. One specialist knows how
/// to simplify math (constant folding). Another knows how to
/// rearrange the plan tree for efficiency (filter pushdown).
/// The optimizer asks each specialist in turn to look at the plan.
pub trait OptimizerRule {
    /// Human-readable name of this rule (for logging).
    fn name(&self) -> &str;

    /// Apply this rule to a plan tree. Returns the (possibly modified) plan.
    fn optimize(&self, plan: Plan) -> Plan;
}
```

This trait is simple on purpose. Two methods: `name()` for identification, `optimize()` for the actual work. Every optimization rule -- no matter how complex -- implements this same interface.

Why is this a good design? Because the optimizer does not need to know what rules exist. It just has a list of things that implement `OptimizerRule` and calls `optimize()` on each one. If you invent a brilliant new optimization next week, you just implement the trait and add it to the list. No existing code changes.

> **What just happened?**
>
> We defined a trait with two methods. Any struct that implements `OptimizerRule` must provide both `name()` and `optimize()`. The trait does not care what the struct looks like internally -- it only cares about the interface. This is what makes it possible to store different rule types in the same `Vec` using `Box<dyn OptimizerRule>`.

### Step 4: Build the Optimizer struct

The `Optimizer` holds a list of rules and applies them one by one:

```rust
// src/optimizer.rs (continued)

/// The query optimizer. Holds a sequence of optimization rules
/// and applies them to query plans.
///
/// Think of this as an assembly line. The plan enters at one end,
/// and each rule gets a chance to improve it as it passes through.
pub struct Optimizer {
    rules: Vec<Box<dyn OptimizerRule>>,
}
```

Let us pause and make sure `Vec<Box<dyn OptimizerRule>>` makes sense:

- `Vec<...>` -- a growable list
- `Box<...>` -- each element is a pointer to a heap-allocated value (because we do not know how big each rule struct is)
- `dyn OptimizerRule` -- each value implements `OptimizerRule`, but we do not know the concrete type

This is the "heterogeneous collection" pattern. Without `Box<dyn Trait>`, you cannot put different types in the same `Vec`.

Now implement the methods:

```rust
impl Optimizer {
    /// Create a new optimizer with no rules.
    pub fn new() -> Self {
        Optimizer { rules: Vec::new() }
    }

    /// Add a rule to the optimizer.
    ///
    /// Rules are applied in the order they are added. This matters!
    /// Constant folding should run before filter pushdown, because
    /// filter pushdown can do more if constants are already simplified.
    pub fn add_rule(&mut self, rule: Box<dyn OptimizerRule>) {
        self.rules.push(rule);
    }

    /// Apply all rules to the plan, in order.
    /// Returns the optimized plan and a report of which rules fired.
    pub fn optimize(&self, plan: Plan) -> OptimizeResult {
        let mut current = plan;
        let mut applied_rules: Vec<String> = Vec::new();

        for rule in &self.rules {
            // Take a snapshot of the plan before this rule runs
            let before = format!("{:?}", current);

            // Apply the rule -- this is where dynamic dispatch happens!
            // Rust uses the vtable to find the right optimize() method
            current = rule.optimize(current);

            // Check if the plan changed
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
```

Let us trace through what happens when `optimize()` is called:

1. We start with the original plan in `current`
2. We loop through each rule in `self.rules`
3. For each rule, `rule.optimize(current)` uses dynamic dispatch -- Rust looks at the vtable to find which `optimize()` function to call. If the rule is a `ConstantFolding`, it calls `ConstantFolding::optimize`. If it is a `FilterPushdown`, it calls `FilterPushdown::optimize`.
4. We compare the plan before and after. If it changed, we record the rule's name.
5. After all rules have run, we return the final plan and the list of changes.

### Step 5: The OptimizeResult struct

```rust
/// The result of optimization: the transformed plan
/// and a log of which rules actually made changes.
#[derive(Debug)]
pub struct OptimizeResult {
    pub plan: Plan,
    pub applied_rules: Vec<String>,
}
```

This is a simple data holder with two fields. We use it so the caller can see both the final plan and what the optimizer did.

### Step 6: Display helpers for plans and expressions

To see what the optimizer is doing, we need to print plan trees in a readable format:

```rust
/// Format a plan tree as a human-readable string with indentation.
///
/// Example output:
///   Project(name)
///     Filter((age > 18))
///       Scan(users)
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
            let cols: Vec<String> = columns.iter()
                .map(|c| format_expr(c))
                .collect();
            write!(f, "{}Project({})\n", pad, cols.join(", "))?;
            display_plan(source, f, indent + 1)
        }
        Plan::EmptyResult => {
            write!(f, "{}EmptyResult", pad)
        }
    }
}
```

This function is recursive -- it prints the current node, then calls itself for child nodes with more indentation. This mirrors the tree structure of the plan. The `write!` macro writes to a formatter (the same system that `println!` uses).

```rust
/// Format an expression as a human-readable string.
///
/// Examples:
///   Literal(42)         => "42"
///   ColumnRef("name")   => "name"
///   BinaryOp(age > 18)  => "(age > 18)"
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

### Step 7: Test the optimizer framework

Now let us verify the framework works before building any real rules. We will create two test rules: one that does nothing, and one that replaces all Scan nodes with EmptyResult.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// A rule that does nothing -- useful for testing the framework.
    struct NoOpRule;

    impl OptimizerRule for NoOpRule {
        fn name(&self) -> &str {
            "NoOp"
        }

        fn optimize(&self, plan: Plan) -> Plan {
            // Return the plan unchanged
            plan
        }
    }

    /// A rule that replaces all Scan nodes with EmptyResult.
    /// Not useful in practice, but great for testing!
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
        let plan = Plan::Scan {
            table: "users".to_string(),
        };

        let result = optimizer.optimize(plan.clone());
        assert_eq!(result.plan, plan);
        assert!(result.applied_rules.is_empty());
    }

    #[test]
    fn noop_rule_does_not_appear_in_applied() {
        let mut optimizer = Optimizer::new();
        optimizer.add_rule(Box::new(NoOpRule));

        let plan = Plan::Scan {
            table: "users".to_string(),
        };
        let result = optimizer.optimize(plan);

        // NoOp did not change the plan, so it should not appear
        assert!(result.applied_rules.is_empty());
    }

    #[test]
    fn kill_scans_rule_replaces_scan() {
        let mut optimizer = Optimizer::new();
        optimizer.add_rule(Box::new(KillScansRule));

        let plan = Plan::Project {
            columns: vec![Expression::ColumnRef("name".to_string())],
            source: Box::new(Plan::Scan {
                table: "users".to_string(),
            }),
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

        let plan = Plan::Scan {
            table: "users".to_string(),
        };
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
test optimizer::tests::empty_optimizer_returns_plan_unchanged ... ok
test optimizer::tests::noop_rule_does_not_appear_in_applied ... ok
test optimizer::tests::kill_scans_rule_replaces_scan ... ok
test optimizer::tests::multiple_rules_applied_in_order ... ok

test result: ok. 4 passed; 0 failed
```

> **What just happened?**
>
> We built a framework where any struct implementing `OptimizerRule` can be boxed and added to the optimizer. The optimizer does not know what concrete types it holds -- it only knows they implement `OptimizerRule`. When it calls `rule.optimize(current)`, Rust uses the vtable to find and call the correct function. This is dynamic dispatch in action. The `NoOpRule` and `KillScansRule` tests prove that different rule types can coexist in the same `Vec<Box<dyn OptimizerRule>>` and each gets the correct `optimize()` called.

> **Common Mistakes**
>
> 1. **Forgetting `Box::new()`**: You cannot push a bare struct into `Vec<Box<dyn OptimizerRule>>`. You must wrap it: `optimizer.add_rule(Box::new(MyRule))`. The `Box::new()` allocates the struct on the heap, which is required because the `Vec` needs all elements to be the same size.
>
> 2. **Forgetting to recurse**: The `KillScansRule` must call `self.optimize(*source)` on child nodes. Without recursion, it would only transform the root node. A plan like `Project(Filter(Scan))` would not have its inner Scan replaced.
>
> 3. **`*source` confusion**: When you match `Plan::Filter { source, .. }`, `source` is a `Box<Plan>`. To get the `Plan` inside, you dereference with `*source`. Then to put the result back in a Box, you use `Box::new(self.optimize(*source))`. This unbox-then-rebox pattern is common when working with recursive Box types.

<details>
<summary>Hint: If you get "the trait OptimizerRule is not object safe"</summary>

Check that your trait methods do not use generics or return `Self`. The trait must have only methods that take `&self` and return concrete types (not generic or `Self`). Our trait is safe because `name()` returns `&str` and `optimize()` takes and returns `Plan` -- both are concrete, known types.

</details>

---

## Exercise 2: Constant Folding

**Goal:** Build a rule that evaluates constant expressions at plan time. `1 + 1` becomes `2`. `3 > 5` becomes `false`. If a filter's predicate is always true, remove the filter entirely. If it is always false, replace the entire branch with `EmptyResult`.

### Step 1: Understand the idea

When the planner sees `WHERE 1 + 1 = 2`, it creates this expression tree:

```
        BinaryOp(=)
       /            \
  BinaryOp(+)    Literal(2)
   /       \
Literal(1)  Literal(1)
```

The constant folding rule works bottom-up:

1. Look at `BinaryOp(+)`. Both children are literals (`1` and `1`). We can evaluate this: `1 + 1 = 2`. Replace the subtree with `Literal(2)`.
2. Now the tree looks like `BinaryOp(=)` with `Literal(2)` on both sides. Both children are literals. Evaluate: `2 = 2` is `true`. Replace with `Literal(true)`.
3. The filter's predicate is now `Literal(true)`. A filter that always passes is useless -- remove it entirely.

After constant folding, the plan goes from `Filter(1 + 1 = 2, Scan(users))` to just `Scan(users)`. We eliminated the filter completely, saving a comparison for every row.

### Step 2: Build the binary evaluation function

We need a function that takes an operator and two literal values and computes the result. If both values are constants and the operation makes sense, it returns `Some(result)`. Otherwise, it returns `None`.

```rust
// src/optimizer.rs (continued)

/// Try to evaluate a binary operation on two literal values.
///
/// Returns Some(result) if both values are constants and the
/// operation is supported. Returns None if we cannot evaluate
/// (e.g., mixed types or division by zero).
fn eval_binary(op: &BinaryOperator, left: &Value, right: &Value) -> Option<Value> {
    match (op, left, right) {
        // Integer arithmetic
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
                None // Leave division by zero for the executor to handle
            } else {
                Some(Value::Integer(a / b))
            }
        }

        // Float arithmetic
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
            if *b == 0.0 { None } else { Some(Value::Float(a / b)) }
        }

        // Integer comparisons
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

        // String comparisons
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

        // Anything else: we do not know how to evaluate it
        _ => None,
    }
}
```

This is a big `match` expression, but each arm is simple. It pattern-matches on three things at once: the operator and both values. The `_` wildcard at the bottom catches everything we cannot handle -- mixed types, unsupported type/operator combinations, etc.

Notice how the division cases return `None` for division by zero. We do not want to crash during optimization. Instead, we leave the expression as-is and let the executor handle the error at runtime.

Now add the unary evaluation:

```rust
/// Try to evaluate a unary operation on a literal value.
fn eval_unary(op: &UnaryOperator, value: &Value) -> Option<Value> {
    match (op, value) {
        (UnaryOperator::Not, Value::Boolean(b)) => Some(Value::Boolean(!b)),
        (UnaryOperator::Negate, Value::Integer(n)) => Some(Value::Integer(-n)),
        (UnaryOperator::Negate, Value::Float(f)) => Some(Value::Float(-f)),
        _ => None,
    }
}
```

> **What just happened?**
>
> We built two functions that act like a mini calculator. Given an operator and literal values, they compute the result. The `Option` return type is important -- `Some(value)` means "I computed this," `None` means "I cannot evaluate this (wrong types, division by zero, etc.)." This pattern -- returning `Option` instead of panicking -- is very common in Rust.

### Step 3: Build the fold_constants function

This function recursively walks an expression tree and simplifies wherever possible:

```rust
/// Attempt to fold (simplify) constant expressions.
///
/// Works bottom-up: first fold the children, then check if
/// the current node can be evaluated.
///
/// Examples:
///   1 + 1         => 2
///   (2 * 3) + 1   => 7
///   age + 1       => age + 1  (cannot fold -- age is a column)
///   (1 + 1) > age => 2 > age  (partially folded)
fn fold_constants(expr: Expression) -> Expression {
    match expr {
        // Literals are already as simple as possible.
        // This is a base case of the recursion.
        Expression::Literal(_) => expr,

        // Column references cannot be evaluated at plan time.
        // We do not know the value of "age" until we have an actual row.
        // This is also a base case.
        Expression::ColumnRef(_) => expr,

        // Binary operations: fold children first, then try to evaluate
        Expression::BinaryOp { left, op, right } => {
            // Step 1: Recursively fold the children.
            // *left dereferences the Box to get the Expression inside.
            let left = fold_constants(*left);
            let right = fold_constants(*right);

            // Step 2: If BOTH children are now literals, evaluate.
            match (&left, &right) {
                (Expression::Literal(l), Expression::Literal(r)) => {
                    match eval_binary(&op, l, r) {
                        // Success! Replace the whole subtree with the result.
                        Some(result) => Expression::Literal(result),
                        // Could not evaluate (e.g., division by zero).
                        // Keep the expression but with folded children.
                        None => Expression::BinaryOp {
                            left: Box::new(left),
                            op,
                            right: Box::new(right),
                        },
                    }
                }
                // At least one child is not a literal (e.g., a column ref).
                // Cannot evaluate, but children may have been partially folded.
                _ => Expression::BinaryOp {
                    left: Box::new(left),
                    op,
                    right: Box::new(right),
                },
            }
        }

        // Unary operations: same pattern -- fold operand, then try to evaluate
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

Let us trace through `1 + 1 = 2` step by step to make sure we understand:

```
Starting expression:  BinaryOp(=, BinaryOp(+, Lit(1), Lit(1)), Lit(2))

1. Match BinaryOp(=). Recurse into left child: BinaryOp(+, Lit(1), Lit(1))
2.   Match BinaryOp(+). Recurse into left child: Lit(1)
3.     Match Literal. Return Lit(1).  (base case)
4.   Recurse into right child: Lit(1)
5.     Match Literal. Return Lit(1).  (base case)
6.   Both children are Literal. Call eval_binary(Add, 1, 1) => Some(Integer(2))
7.   Return Lit(2).
8. Left child is now Lit(2). Recurse into right child: Lit(2)
9.   Match Literal. Return Lit(2).  (base case)
10. Both children are Literal. Call eval_binary(Equal, 2, 2) => Some(Boolean(true))
11. Return Lit(true).

Final result: Literal(Boolean(true))
```

The entire expression collapsed to a single `Literal(true)`.

> **What just happened?**
>
> The function uses recursion to work bottom-up through the expression tree. It simplifies children first, then checks if the parent can be simplified. This is important because `(1 + 1) = 2` cannot be evaluated at the top level (the left child is not a literal yet), but after the left child `1 + 1` is folded to `2`, the parent becomes `2 = 2`, which can be evaluated to `true`. Bottom-up processing makes partial folding possible too: `age > (1 + 1)` becomes `age > 2` because the right side gets folded even though the left side (a column) cannot be.

### Step 4: Implement the ConstantFolding rule

Now wrap `fold_constants` in a struct that implements `OptimizerRule`:

```rust
/// Constant folding rule: evaluates constant expressions at plan time.
///
/// This rule can:
/// - Simplify math: 1 + 1 => 2, 3 * 4 => 12
/// - Evaluate comparisons: 3 > 5 => false
/// - Remove always-true filters: Filter(true, source) => source
/// - Replace always-false filters: Filter(false, source) => EmptyResult
pub struct ConstantFolding;

impl OptimizerRule for ConstantFolding {
    fn name(&self) -> &str {
        "ConstantFolding"
    }

    fn optimize(&self, plan: Plan) -> Plan {
        match plan {
            Plan::Filter { predicate, source } => {
                // First, optimize the child plan (recursion)
                let source = self.optimize(*source);

                // Then, fold constants in the predicate
                let predicate = fold_constants(predicate);

                // Check if the predicate became a constant
                match &predicate {
                    // Predicate is always true: remove the filter
                    // (everything passes, so filtering is pointless)
                    Expression::Literal(Value::Boolean(true)) => source,

                    // Predicate is always false: nothing passes
                    Expression::Literal(Value::Boolean(false)) => Plan::EmptyResult,

                    // Predicate still has variables: keep the filter
                    // but with the simplified predicate
                    _ => Plan::Filter {
                        predicate,
                        source: Box::new(source),
                    },
                }
            }

            Plan::Project { columns, source } => {
                // Optimize child plan
                let source = self.optimize(*source);

                // Also fold constants in projection expressions
                // (e.g., SELECT 1 + 1 could become SELECT 2)
                let columns = columns.into_iter()
                    .map(|c| fold_constants(c))
                    .collect();

                Plan::Project {
                    columns,
                    source: Box::new(source),
                }
            }

            // Scan and EmptyResult have no expressions to fold
            Plan::Scan { .. } => plan,
            Plan::EmptyResult => plan,
        }
    }
}
```

The key insight is in the `Filter` arm. After folding the predicate:
- If it became `true`, the filter adds no value. Every row passes. So we remove the filter and return just the source.
- If it became `false`, no row can ever pass. So we replace the entire branch with `EmptyResult`.
- Otherwise, we keep the filter but with a possibly simplified predicate.

### Step 5: Test constant folding

```rust
#[cfg(test)]
mod constant_folding_tests {
    use super::*;

    #[test]
    fn fold_simple_addition() {
        // 1 + 1 should fold to 2
        let expr = Expression::BinaryOp {
            left: Box::new(Expression::Literal(Value::Integer(1))),
            op: BinaryOperator::Add,
            right: Box::new(Expression::Literal(Value::Integer(1))),
        };

        let result = fold_constants(expr);
        assert_eq!(result, Expression::Literal(Value::Integer(2)));
    }

    #[test]
    fn fold_nested_expression() {
        // (1 + 1) = 2 should fold to true
        let expr = Expression::BinaryOp {
            left: Box::new(Expression::BinaryOp {
                left: Box::new(Expression::Literal(Value::Integer(1))),
                op: BinaryOperator::Add,
                right: Box::new(Expression::Literal(Value::Integer(1))),
            }),
            op: BinaryOperator::Equal,
            right: Box::new(Expression::Literal(Value::Integer(2))),
        };

        let result = fold_constants(expr);
        assert_eq!(result, Expression::Literal(Value::Boolean(true)));
    }

    #[test]
    fn fold_preserves_column_refs() {
        // age + 1 cannot be fully folded (age is unknown at plan time)
        let expr = Expression::BinaryOp {
            left: Box::new(Expression::ColumnRef("age".to_string())),
            op: BinaryOperator::Add,
            right: Box::new(Expression::Literal(Value::Integer(1))),
        };

        let result = fold_constants(expr.clone());
        assert_eq!(result, expr); // unchanged
    }

    #[test]
    fn fold_partially_folds_mixed() {
        // age > (1 + 1) should become age > 2
        // The right side folds, the left side (column) does not.
        let expr = Expression::BinaryOp {
            left: Box::new(Expression::ColumnRef("age".to_string())),
            op: BinaryOperator::GreaterThan,
            right: Box::new(Expression::BinaryOp {
                left: Box::new(Expression::Literal(Value::Integer(1))),
                op: BinaryOperator::Add,
                right: Box::new(Expression::Literal(Value::Integer(1))),
            }),
        };

        let result = fold_constants(expr);
        let expected = Expression::BinaryOp {
            left: Box::new(Expression::ColumnRef("age".to_string())),
            op: BinaryOperator::GreaterThan,
            right: Box::new(Expression::Literal(Value::Integer(2))),
        };
        assert_eq!(result, expected);
    }

    #[test]
    fn constant_folding_removes_always_true_filter() {
        let rule = ConstantFolding;

        // Filter(1 + 1 = 2, Scan(users)) should become Scan(users)
        let plan = Plan::Filter {
            predicate: Expression::BinaryOp {
                left: Box::new(Expression::BinaryOp {
                    left: Box::new(Expression::Literal(Value::Integer(1))),
                    op: BinaryOperator::Add,
                    right: Box::new(Expression::Literal(Value::Integer(1))),
                }),
                op: BinaryOperator::Equal,
                right: Box::new(Expression::Literal(Value::Integer(2))),
            },
            source: Box::new(Plan::Scan {
                table: "users".to_string(),
            }),
        };

        let result = rule.optimize(plan);
        assert_eq!(result, Plan::Scan {
            table: "users".to_string(),
        });
    }

    #[test]
    fn constant_folding_replaces_always_false_filter() {
        let rule = ConstantFolding;

        // Filter(1 > 2, Scan(users)) should become EmptyResult
        let plan = Plan::Filter {
            predicate: Expression::BinaryOp {
                left: Box::new(Expression::Literal(Value::Integer(1))),
                op: BinaryOperator::GreaterThan,
                right: Box::new(Expression::Literal(Value::Integer(2))),
            },
            source: Box::new(Plan::Scan {
                table: "users".to_string(),
            }),
        };

        let result = rule.optimize(plan);
        assert_eq!(result, Plan::EmptyResult);
    }
}
```

```
$ cargo test constant_folding_tests
running 6 tests
test optimizer::constant_folding_tests::fold_simple_addition ... ok
test optimizer::constant_folding_tests::fold_nested_expression ... ok
test optimizer::constant_folding_tests::fold_preserves_column_refs ... ok
test optimizer::constant_folding_tests::fold_partially_folds_mixed ... ok
test optimizer::constant_folding_tests::constant_folding_removes_always_true_filter ... ok
test optimizer::constant_folding_tests::constant_folding_replaces_always_false_filter ... ok

test result: ok. 6 passed; 0 failed
```

> **Common Mistakes**
>
> 1. **Forgetting to recurse into children first**: If you try to evaluate `(1 + 1) = 2` at the top level without folding children first, you see `BinaryOp(=, BinaryOp(+, ...), Literal(2))`. The left child is not a `Literal`, so you cannot evaluate. You must fold children first (bottom-up), then check the parent.
>
> 2. **Matching `&predicate` vs `predicate`**: After `let predicate = fold_constants(predicate)`, you own the value. When matching with `match &predicate`, the `&` borrows it so you can still use `predicate` in the `_ =>` arm. Without `&`, the match would move `predicate` into the matched arm and you could not use it later.
>
> 3. **Forgetting `Plan::EmptyResult` in the match**: Every match on `Plan` must handle all variants. If you forget one, the compiler tells you with a "non-exhaustive patterns" error. This is Rust protecting you from missing a case.

---

## Exercise 3: Filter Pushdown

**Goal:** Build a rule that moves filters closer to the data source. When a filter sits above a projection, push it below so that rows are filtered earlier and the projection processes fewer rows.

### Step 1: Understand filter pushdown

Consider this plan for `SELECT name FROM users WHERE age > 18`:

```
Plan BEFORE pushdown:

  Filter (age > 18)           <-- filters AFTER projection
    Project [name, age]
      Scan users

Plan AFTER pushdown:

  Project [name, age]
    Filter (age > 18)          <-- filters BEFORE projection
      Scan users
```

Why does this matter? In the "before" version, the Project processes ALL rows from the Scan, producing a narrower row (just name and age) for every row. Then the Filter throws away rows that do not match. In the "after" version, the Filter throws away rows first, and the Project only processes the surviving rows.

Think of it like sorting a pile of job applications. You could first photocopy every application (projection), then throw away the ones that do not meet the requirements (filtering). Or you could first throw away the bad applications, then photocopy only the good ones. Same result, a lot less photocopying.

### Step 2: Collect column references

Before we can push a filter down, we need to check that the filter only uses columns available in the source below. A filter on "email" cannot be pushed below a projection that only keeps "name" and "age."

```rust
/// Collect all column names referenced in an expression.
///
/// This walks the expression tree and gathers every ColumnRef it finds.
///
/// Examples:
///   age > 18                        => ["age"]
///   age > 18 AND name = 'Alice'     => ["age", "name"]
///   1 + 1                           => []  (no columns)
fn collect_columns(expr: &Expression) -> Vec<String> {
    match expr {
        Expression::Literal(_) => vec![],
        Expression::ColumnRef(name) => vec![name.clone()],
        Expression::BinaryOp { left, right, .. } => {
            let mut cols = collect_columns(left);
            cols.extend(collect_columns(right));
            cols
        }
        Expression::UnaryOp { operand, .. } => {
            collect_columns(operand)
        }
    }
}
```

This is another recursive function. For literals, no columns. For column refs, one column. For operators, collect from both children. The `extend` method appends all elements from one Vec onto another.

### Step 3: Implement the FilterPushdown rule

```rust
/// Filter pushdown rule: moves filters below projections.
///
/// This is safe only when the filter references columns that
/// are available in the projection's source. If the projection
/// drops a column the filter needs, we cannot push down.
pub struct FilterPushdown;

impl OptimizerRule for FilterPushdown {
    fn name(&self) -> &str {
        "FilterPushdown"
    }

    fn optimize(&self, plan: Plan) -> Plan {
        match plan {
            // Look for the pattern: Filter sitting on top of Project
            Plan::Filter { predicate, source } => {
                // First, recursively optimize the child
                let source = self.optimize(*source);

                match source {
                    Plan::Project { columns, source: proj_source } => {
                        // We found Filter(Project(...)).
                        // Can we push the filter below the project?

                        // Step 1: What columns does the filter need?
                        let filter_cols = collect_columns(&predicate);

                        // Step 2: What columns does the projection output?
                        let proj_col_names: Vec<String> = columns.iter()
                            .filter_map(|c| {
                                if let Expression::ColumnRef(name) = c {
                                    Some(name.clone())
                                } else {
                                    None
                                }
                            })
                            .collect();

                        // Step 3: Check that ALL filter columns are available
                        let can_push = filter_cols.iter()
                            .all(|col| proj_col_names.contains(col));

                        if can_push {
                            // Safe to push! Rearrange the tree:
                            // Filter(pred, Project(cols, src))
                            // becomes
                            // Project(cols, Filter(pred, src))
                            Plan::Project {
                                columns,
                                source: Box::new(Plan::Filter {
                                    predicate,
                                    source: proj_source,
                                }),
                            }
                        } else {
                            // Not safe to push -- keep original order
                            Plan::Filter {
                                predicate,
                                source: Box::new(Plan::Project {
                                    columns,
                                    source: proj_source,
                                }),
                            }
                        }
                    }
                    // Source is not a Project -- nothing to push through
                    other => Plan::Filter {
                        predicate,
                        source: Box::new(other),
                    },
                }
            }

            // For other plan types, just recurse into children
            Plan::Project { columns, source } => {
                Plan::Project {
                    columns,
                    source: Box::new(self.optimize(*source)),
                }
            }

            // Leaf nodes: nothing to optimize
            Plan::Scan { .. } => plan,
            Plan::EmptyResult => plan,
        }
    }
}
```

Let us walk through the logic:

1. We look for a `Filter` node. We recursively optimize its child first.
2. If the optimized child is a `Project`, we have the pattern `Filter(Project(...))`.
3. We collect the column names the filter uses (`filter_cols`) and the column names the projection outputs (`proj_col_names`).
4. If every filter column appears in the projection's output, it is safe to push the filter below. We rearrange: `Project(cols, Filter(pred, source))`.
5. If any filter column is missing from the projection, we cannot push. We leave the plan unchanged.

The `filter_map` call deserves explanation. It combines `filter` and `map` in one step:

```rust,ignore
// filter_map: apply a function that returns Option.
// Keep the Some values, discard the Nones.
columns.iter()
    .filter_map(|c| {
        if let Expression::ColumnRef(name) = c {
            Some(name.clone())  // keep this one
        } else {
            None  // skip this one (it is a literal or expression, not a column name)
        }
    })
    .collect()
```

This extracts just the column names from the projection's expression list, skipping any computed expressions.

> **What just happened?**
>
> We built a rule that rearranges the plan tree. Instead of filtering after projection, we filter before projection. This means fewer rows flow through the expensive projection step. The rule is careful to check that pushing down is safe -- if the filter needs a column that the projection drops, pushing down would break the query. This is a key principle of optimization: never change the results, only change the computation path.

### Step 4: Test filter pushdown

```rust
#[cfg(test)]
mod filter_pushdown_tests {
    use super::*;

    #[test]
    fn pushes_filter_below_project() {
        let rule = FilterPushdown;

        // Filter(age > 18, Project([name, age], Scan(users)))
        let plan = Plan::Filter {
            predicate: Expression::BinaryOp {
                left: Box::new(Expression::ColumnRef("age".to_string())),
                op: BinaryOperator::GreaterThan,
                right: Box::new(Expression::Literal(Value::Integer(18))),
            },
            source: Box::new(Plan::Project {
                columns: vec![
                    Expression::ColumnRef("name".to_string()),
                    Expression::ColumnRef("age".to_string()),
                ],
                source: Box::new(Plan::Scan {
                    table: "users".to_string(),
                }),
            }),
        };

        let result = rule.optimize(plan);

        // Expected: Project([name, age], Filter(age > 18, Scan(users)))
        let expected = Plan::Project {
            columns: vec![
                Expression::ColumnRef("name".to_string()),
                Expression::ColumnRef("age".to_string()),
            ],
            source: Box::new(Plan::Filter {
                predicate: Expression::BinaryOp {
                    left: Box::new(Expression::ColumnRef("age".to_string())),
                    op: BinaryOperator::GreaterThan,
                    right: Box::new(Expression::Literal(Value::Integer(18))),
                },
                source: Box::new(Plan::Scan {
                    table: "users".to_string(),
                }),
            }),
        };

        assert_eq!(result, expected);
    }

    #[test]
    fn does_not_push_when_column_missing() {
        let rule = FilterPushdown;

        // Filter on "email", but Project only has ["name"]
        // Cannot push because email is not available below
        let plan = Plan::Filter {
            predicate: Expression::BinaryOp {
                left: Box::new(Expression::ColumnRef("email".to_string())),
                op: BinaryOperator::Equal,
                right: Box::new(Expression::Literal(Value::String(
                    "test@example.com".to_string(),
                ))),
            },
            source: Box::new(Plan::Project {
                columns: vec![Expression::ColumnRef("name".to_string())],
                source: Box::new(Plan::Scan {
                    table: "users".to_string(),
                }),
            }),
        };

        let result = rule.optimize(plan.clone());
        assert_eq!(result, plan); // unchanged
    }

    #[test]
    fn filter_on_scan_stays_in_place() {
        let rule = FilterPushdown;

        // Filter directly on Scan -- nothing to push through
        let plan = Plan::Filter {
            predicate: Expression::BinaryOp {
                left: Box::new(Expression::ColumnRef("age".to_string())),
                op: BinaryOperator::GreaterThan,
                right: Box::new(Expression::Literal(Value::Integer(18))),
            },
            source: Box::new(Plan::Scan {
                table: "users".to_string(),
            }),
        };

        let result = rule.optimize(plan.clone());
        assert_eq!(result, plan); // unchanged -- nothing to push through
    }
}
```

```
$ cargo test filter_pushdown_tests
running 3 tests
test optimizer::filter_pushdown_tests::pushes_filter_below_project ... ok
test optimizer::filter_pushdown_tests::does_not_push_when_column_missing ... ok
test optimizer::filter_pushdown_tests::filter_on_scan_stays_in_place ... ok

test result: ok. 3 passed; 0 failed
```

> **Common Mistakes**
>
> 1. **Pushing when columns are missing**: If a projection only outputs `[name]` and the filter uses `age`, pushing the filter below would cause a runtime error because `age` is no longer available. Always check column availability before pushing.
>
> 2. **Not recursing before checking**: Call `self.optimize(*source)` before inspecting the result. This ensures nested patterns (like filter-above-filter-above-project) are handled correctly from the inside out.
>
> 3. **Accidentally dropping the projection**: When you rearrange `Filter(Project(source))` to `Project(Filter(source))`, make sure the project keeps its column list. A common bug is forgetting to include the `columns` field in the new Project node.

---

## Exercise 4: Putting It All Together

**Goal:** Wire up constant folding and filter pushdown into a default optimizer, and test the full pipeline.

### Step 1: Create the default optimizer

```rust
// src/optimizer.rs (continued)

impl Optimizer {
    /// Create an optimizer with the standard set of rules.
    ///
    /// The order matters! Constant folding runs first because
    /// it simplifies expressions. Filter pushdown runs second
    /// because it benefits from those simplifications.
    ///
    /// Example: if a filter has predicate (1 + 1 = 2), constant
    /// folding reduces it to (true) and removes the filter.
    /// Filter pushdown never even sees it.
    pub fn default_optimizer() -> Self {
        let mut optimizer = Optimizer::new();
        optimizer.add_rule(Box::new(ConstantFolding));
        optimizer.add_rule(Box::new(FilterPushdown));
        optimizer
    }
}
```

Notice how we use `Box::new()` to create each rule and `add_rule()` to add them. The optimizer stores `Box<dyn OptimizerRule>`, so it does not care that `ConstantFolding` and `FilterPushdown` are different types.

### Step 2: Test the full pipeline

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn full_pipeline_removes_constant_filter() {
        let optimizer = Optimizer::default_optimizer();

        // SELECT name FROM users WHERE 1 + 1 = 2
        // Plan: Project([name], Filter(1 + 1 = 2, Scan(users)))
        let plan = Plan::Project {
            columns: vec![Expression::ColumnRef("name".to_string())],
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
                source: Box::new(Plan::Scan {
                    table: "users".to_string(),
                }),
            }),
        };

        let result = optimizer.optimize(plan);

        // After constant folding: 1 + 1 = 2 becomes true, filter removed
        // Result: Project([name], Scan(users))
        let expected = Plan::Project {
            columns: vec![Expression::ColumnRef("name".to_string())],
            source: Box::new(Plan::Scan {
                table: "users".to_string(),
            }),
        };

        assert_eq!(result.plan, expected);
        assert!(result.applied_rules.contains(&"ConstantFolding".to_string()));
    }

    #[test]
    fn full_pipeline_pushes_filter_down() {
        let optimizer = Optimizer::default_optimizer();

        // Filter(age > 18, Project([name, age], Scan(users)))
        let plan = Plan::Filter {
            predicate: Expression::BinaryOp {
                left: Box::new(Expression::ColumnRef("age".to_string())),
                op: BinaryOperator::GreaterThan,
                right: Box::new(Expression::Literal(Value::Integer(18))),
            },
            source: Box::new(Plan::Project {
                columns: vec![
                    Expression::ColumnRef("name".to_string()),
                    Expression::ColumnRef("age".to_string()),
                ],
                source: Box::new(Plan::Scan {
                    table: "users".to_string(),
                }),
            }),
        };

        let result = optimizer.optimize(plan);

        // Filter pushed below Project
        let expected = Plan::Project {
            columns: vec![
                Expression::ColumnRef("name".to_string()),
                Expression::ColumnRef("age".to_string()),
            ],
            source: Box::new(Plan::Filter {
                predicate: Expression::BinaryOp {
                    left: Box::new(Expression::ColumnRef("age".to_string())),
                    op: BinaryOperator::GreaterThan,
                    right: Box::new(Expression::Literal(Value::Integer(18))),
                },
                source: Box::new(Plan::Scan {
                    table: "users".to_string(),
                }),
            }),
        };

        assert_eq!(result.plan, expected);
        assert!(result.applied_rules.contains(&"FilterPushdown".to_string()));
    }

    #[test]
    fn impossible_filter_produces_empty_result() {
        let optimizer = Optimizer::default_optimizer();

        // SELECT name FROM users WHERE 1 > 2
        // 1 > 2 is always false
        let plan = Plan::Project {
            columns: vec![Expression::ColumnRef("name".to_string())],
            source: Box::new(Plan::Filter {
                predicate: Expression::BinaryOp {
                    left: Box::new(Expression::Literal(Value::Integer(1))),
                    op: BinaryOperator::GreaterThan,
                    right: Box::new(Expression::Literal(Value::Integer(2))),
                },
                source: Box::new(Plan::Scan {
                    table: "users".to_string(),
                }),
            }),
        };

        let result = optimizer.optimize(plan);

        // 1 > 2 is false, so the filter becomes EmptyResult
        let expected = Plan::Project {
            columns: vec![Expression::ColumnRef("name".to_string())],
            source: Box::new(Plan::EmptyResult),
        };

        assert_eq!(result.plan, expected);
        assert!(result.applied_rules.contains(&"ConstantFolding".to_string()));
    }
}
```

```
$ cargo test integration_tests
running 3 tests
test optimizer::integration_tests::full_pipeline_removes_constant_filter ... ok
test optimizer::integration_tests::full_pipeline_pushes_filter_down ... ok
test optimizer::integration_tests::impossible_filter_produces_empty_result ... ok

test result: ok. 3 passed; 0 failed
```

> **What just happened?**
>
> Two independent rules, each a separate struct implementing `OptimizerRule`, work together in a pipeline. The constant folding rule simplifies expressions. The filter pushdown rule rearranges the plan tree. Each rule has no knowledge of the other -- they just implement the same trait. The optimizer applies them in sequence through dynamic dispatch. This is the power of trait objects: extensibility without coupling.

---

## Exercise 5: Add Your Own Rule (Challenge)

**Goal:** Implement a short-circuit evaluation rule. This rule recognizes special patterns in boolean expressions and simplifies them:

- `true AND x` simplifies to `x` (if the first part is already true, only `x` matters)
- `false AND x` simplifies to `false` (if the first part is false, the whole AND is false)
- `true OR x` simplifies to `true` (if the first part is true, the whole OR is true)
- `false OR x` simplifies to `x` (if the first part is false, only `x` matters)
- Same patterns with the constant on the right side

This is a stretch exercise. Try implementing it yourself before looking at the hints.

<details>
<summary>Hint 1: The struct definition</summary>

```rust
pub struct ShortCircuitEvaluation;

impl OptimizerRule for ShortCircuitEvaluation {
    fn name(&self) -> &str {
        "ShortCircuitEvaluation"
    }

    fn optimize(&self, plan: Plan) -> Plan {
        // Walk the plan tree, apply short_circuit() to each predicate
        // Use the same pattern as ConstantFolding:
        // - For Filter nodes, apply to the predicate
        // - For Project nodes, recurse into source
        // - For Scan/EmptyResult, return unchanged
        todo!()
    }
}
```

</details>

<details>
<summary>Hint 2: The core simplification function</summary>

```rust
fn short_circuit(expr: Expression) -> Expression {
    match expr {
        Expression::BinaryOp { left, op, right } => {
            let left = short_circuit(*left);
            let right = short_circuit(*right);

            match (&op, &left, &right) {
                // true AND x => x
                (BinaryOperator::And, Expression::Literal(Value::Boolean(true)), _) => right,
                // false AND x => false
                (BinaryOperator::And, Expression::Literal(Value::Boolean(false)), _) => {
                    Expression::Literal(Value::Boolean(false))
                }
                // x AND true => x
                (BinaryOperator::And, _, Expression::Literal(Value::Boolean(true))) => left,
                // x AND false => false
                (BinaryOperator::And, _, Expression::Literal(Value::Boolean(false))) => {
                    Expression::Literal(Value::Boolean(false))
                }
                // true OR x => true
                (BinaryOperator::Or, Expression::Literal(Value::Boolean(true)), _) => {
                    Expression::Literal(Value::Boolean(true))
                }
                // false OR x => x
                (BinaryOperator::Or, Expression::Literal(Value::Boolean(false)), _) => right,
                // x OR true => true
                (BinaryOperator::Or, _, Expression::Literal(Value::Boolean(true))) => {
                    Expression::Literal(Value::Boolean(true))
                }
                // x OR false => x
                (BinaryOperator::Or, _, Expression::Literal(Value::Boolean(false))) => left,
                // No short-circuit pattern matched
                _ => Expression::BinaryOp {
                    left: Box::new(left),
                    op,
                    right: Box::new(right),
                },
            }
        }
        // Non-binary expressions: return unchanged
        other => other,
    }
}
```

</details>

<details>
<summary>Hint 3: Test cases to verify your implementation</summary>

```rust
#[test]
fn short_circuit_true_and_x() {
    let expr = Expression::BinaryOp {
        left: Box::new(Expression::Literal(Value::Boolean(true))),
        op: BinaryOperator::And,
        right: Box::new(Expression::ColumnRef("active".to_string())),
    };

    let result = short_circuit(expr);
    assert_eq!(result, Expression::ColumnRef("active".to_string()));
}

#[test]
fn short_circuit_false_and_x() {
    let expr = Expression::BinaryOp {
        left: Box::new(Expression::Literal(Value::Boolean(false))),
        op: BinaryOperator::And,
        right: Box::new(Expression::ColumnRef("active".to_string())),
    };

    let result = short_circuit(expr);
    assert_eq!(result, Expression::Literal(Value::Boolean(false)));
}

#[test]
fn short_circuit_true_or_x() {
    let expr = Expression::BinaryOp {
        left: Box::new(Expression::Literal(Value::Boolean(true))),
        op: BinaryOperator::Or,
        right: Box::new(Expression::ColumnRef("active".to_string())),
    };

    let result = short_circuit(expr);
    assert_eq!(result, Expression::Literal(Value::Boolean(true)));
}
```

</details>

---

## What We Built

In this chapter, you built a query optimizer that transforms naive plans into efficient ones. Here is what you accomplished:

1. **OptimizerRule trait** -- a clean interface that any optimization rule implements
2. **Optimizer struct** -- holds `Vec<Box<dyn OptimizerRule>>` and applies rules in sequence
3. **Constant folding** -- evaluates expressions like `1 + 1` at plan time, removing always-true/false filters
4. **Filter pushdown** -- moves filters closer to the data source, reducing unnecessary work
5. **Display helpers** -- human-readable plan tree output for debugging

The Rust concepts you learned:

- **Trait objects (`dyn Trait`)** -- erasing concrete types to store different types behind a common interface
- **`Box<dyn Trait>`** -- heap-allocating trait objects because their size is unknown at compile time
- **Dynamic dispatch** -- runtime method lookup through a vtable (two-pointer "fat pointer")
- **Static vs dynamic dispatch** -- `impl Trait` for compile-time resolution, `dyn Trait` for runtime resolution
- **Object safety** -- the rules for which traits can be used as trait objects (no generics, no Self returns)
- **The vtable** -- a compile-time table of function pointers that enables runtime method lookup

The optimizer is the first part of your database that "thinks." The lexer, parser, and planner are mechanical -- they translate SQL into a plan following fixed rules. The optimizer looks at the plan and asks "can I do this faster?" This is the beginning of intelligence in your database engine.

---

## Key Takeaways

1. **`Vec<Box<dyn Trait>>` is the Rust pattern for heterogeneous collections.** When you need different types in one Vec, all sharing a common interface, this is the way.

2. **Dynamic dispatch costs one pointer dereference per method call.** For code that runs once per query (like optimization rules), this is negligible. For inner loops processing millions of rows, prefer static dispatch with `impl Trait` or generics.

3. **Optimizers never change what a query returns -- only how it computes the answer.** This is the fundamental invariant. If an optimization changes the results, it is a bug.

4. **Rule order matters.** Constant folding before filter pushdown gives better results than the reverse. Simplified predicates enable more pushdown opportunities.

5. **Object safety has simple rules.** No generic methods, no `Self` in return types. If the compiler cannot build a vtable, it tells you.
