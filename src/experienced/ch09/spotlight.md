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
