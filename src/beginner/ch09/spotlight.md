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
