## Spotlight: Lifetimes & References

Every chapter has one spotlight concept. This chapter's spotlight is **lifetimes and references** — Rust's mechanism for ensuring that borrowed data is always valid, and the foundation of its memory safety guarantees.

### The problem lifetimes solve

In C, you can return a pointer to a local variable:

```c
int* get_value() {
    int x = 42;
    return &x;  // x is destroyed when the function returns
}
// The caller now has a pointer to garbage — a dangling pointer
```

This compiles, runs, and silently corrupts memory. In Java or Python, the garbage collector prevents this by keeping objects alive as long as any reference exists. Rust takes a third path: the compiler tracks how long every reference is valid and refuses to compile code that would create a dangling reference.

### References: borrowing without owning

A reference lets you access data without taking ownership. There are two kinds:

```rust
let name = String::from("toydb");

let r1 = &name;        // shared reference: can read, cannot modify
let r2 = &name;        // multiple shared references are OK
println!("{} {}", r1, r2);

let r3 = &mut name;    // ERROR: cannot borrow as mutable — r1 and r2 are still in scope
```

The borrowing rules are simple but strict:

1. You can have **any number of shared references** (`&T`), OR
2. You can have **exactly one mutable reference** (`&mut T`)
3. But **never both at the same time**

This is not a limitation — it is a feature. It prevents data races at compile time. If you have a `&mut` reference, no one else can read or write the data. If you have a `&` reference, the data cannot change underneath you.

### What is a lifetime?

A lifetime is the span of code during which a reference is valid. Most of the time, the compiler infers lifetimes automatically. But sometimes you need to tell the compiler: "these two references live for the same duration" or "this returned reference lives as long as this input."

```rust
// The compiler infers: r lives as long as name
let name = String::from("toydb");
let r = &name;  // r's lifetime starts here
println!("{}", r);  // r's lifetime ends here (last use)
```

When a function returns a reference, the compiler needs to know which input the output borrows from:

```rust
// This does NOT compile — the compiler cannot infer the lifetime
fn longest(a: &str, b: &str) -> &str {
    if a.len() > b.len() { a } else { b }
}
```

The return value borrows from either `a` or `b`, but the compiler does not know which. You must annotate:

```rust
// 'a means: the returned reference lives at least as long as both inputs
fn longest<'a>(a: &'a str, b: &'a str) -> &'a str {
    if a.len() > b.len() { a } else { b }
}
```

The `'a` is a lifetime parameter. It says: "the returned `&str` is valid for as long as both input `&str`s are valid." If the caller drops one of the inputs, the returned reference becomes invalid — and the compiler will catch it.

### Lifetime annotations on structs

When a struct holds a reference, it needs a lifetime annotation:

```rust
struct Transaction<'a> {
    store: &'a Store,      // borrows from a Store
    version: u64,
}
```

This says: a `Transaction` cannot outlive the `Store` it borrows from. If the `Store` is dropped, every `Transaction` referencing it becomes invalid. The compiler enforces this — you cannot use a `Transaction` after its `Store` is gone.

### When lifetimes get in the way

In practice, lifetimes are most annoying when you want a struct to hold a reference to something. The compiler forces you to thread lifetime parameters through every struct and function that touches the reference. Sometimes the cleanest solution is to avoid references entirely:

```rust
// Instead of borrowing:
struct Transaction<'a> {
    store: &'a Store,     // must track lifetime
}

// Own the data:
struct Transaction {
    store: Store,         // no lifetime needed
}

// Or use shared ownership:
use std::sync::Arc;
struct Transaction {
    store: Arc<Store>,    // reference-counted, no lifetime needed
}
```

For our MVCC implementation, we will own the data in the `Transaction` struct and use interior mutability. This avoids lifetime gymnastics while still being safe.

> **Coming from JS/Python/Go?**
>
> | Concept | JavaScript | Python | Go | Rust |
> |---------|-----------|--------|----|------|
> | Memory management | Garbage collector | Garbage collector | Garbage collector | Ownership + borrowing |
> | Dangling references | Impossible (GC) | Impossible (GC) | Impossible (GC) | Compile-time error |
> | Data races | Possible (shared state) | GIL prevents (mostly) | Possible (goroutines) | Compile-time error |
> | Shared access | Always allowed | Always allowed | Always allowed | `&T` — read only |
> | Mutable access | Always allowed | Always allowed | Always allowed (mutex optional) | `&mut T` — exclusive |
> | Lifetime annotations | N/A | N/A | N/A | `'a` on references |
>
> The key difference: in GC'd languages, the runtime figures out when to free memory. In Rust, the compiler figures it out at compile time. The cost is that you sometimes need to annotate lifetimes. The benefit is zero runtime overhead, no GC pauses, and data races are impossible.

---
