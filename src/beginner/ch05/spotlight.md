## Spotlight: Lifetimes & References

Every chapter has one spotlight concept. This chapter's spotlight is **lifetimes and references** -- Rust's mechanism for ensuring that borrowed data is always valid, and the foundation of its memory safety guarantees.

### Why this matters for MVCC

When a transaction reads from the store, it borrows a reference to the data. But what if the store is modified (or dropped) while the transaction is still reading? In C, this would be a dangling pointer -- reading freed memory, leading to crashes or security vulnerabilities. In Java or Python, the garbage collector prevents this by keeping data alive as long as anything references it. Rust takes a third path: the compiler tracks how long every reference is valid and refuses to compile code that would create a dangling reference.

### What is a reference?

A reference lets you access data without taking ownership. Think of it as borrowing.

> **Analogy: Borrowing a book vs. buying a book**
>
> If you buy a book, you own it. You can read it, write in it, give it away, or throw it out. You decide when it stops existing.
>
> If you borrow a book from the library, you can read it but you cannot write in it or throw it out. You must return it before the library closes. And you cannot give it to someone else and promise them they can keep it forever -- the library might close!
>
> In Rust, **ownership** is like buying the book. A **shared reference** (`&T`) is like borrowing to read. A **mutable reference** (`&mut T`) is like borrowing a pen to write notes in the margin -- only one person can have the pen at a time.

### Shared references: `&T`

A shared reference lets you read data without modifying it. You can have as many shared references as you want:

```rust
fn main() {
    let name = String::from("toydb");

    let r1 = &name;        // first shared reference
    let r2 = &name;        // second shared reference -- this is fine
    let r3 = &name;        // third shared reference -- still fine

    println!("{}, {}, {}", r1, r2, r3);  // all three work
}
```

Multiple readers can look at the same data simultaneously. This is safe because none of them can change it.

### Mutable references: `&mut T`

A mutable reference lets you modify data, but you can only have one at a time:

```rust,ignore
fn main() {
    let mut name = String::from("toydb");

    let r1 = &mut name;    // mutable reference
    r1.push_str(" v2");    // we can modify through r1
    println!("{}", r1);    // prints "toydb v2"

    // let r2 = &mut name; // ERROR: cannot have two mutable references
}
```

Why only one? Imagine two people both editing the same document at the same time, on the same line. One adds "hello" while the other deletes the line. The result is unpredictable. Rust prevents this by allowing only one mutable reference at a time.

### The borrowing rules

Rust enforces these rules at compile time:

1. You can have **any number of shared references** (`&T`), OR
2. You can have **exactly one mutable reference** (`&mut T`)
3. But **never both at the same time**

```rust,ignore
fn main() {
    let mut data = vec![1, 2, 3];

    let r1 = &data;          // shared reference
    let r2 = &data;          // another shared reference -- OK
    println!("{:?} {:?}", r1, r2);

    let r3 = &mut data;      // mutable reference -- OK because r1 and r2 are done
    r3.push(4);
    println!("{:?}", r3);

    // But NOT both at once:
    // let r4 = &data;       // ERROR if r3 is still in use
    // println!("{:?} {:?}", r3, r4);
}
```

> **What just happened?**
>
> Rust's borrowing rules prevent data races at compile time. A **data race** happens when two pieces of code access the same data at the same time and at least one of them is writing. In other languages, data races cause mysterious bugs that only appear under heavy load. In Rust, they are impossible -- the compiler catches them before your program ever runs.

### What is a lifetime?

A lifetime is the span of code during which a reference is valid. Most of the time, the compiler figures out lifetimes automatically. But sometimes you need to be explicit.

```rust,ignore
fn main() {
    let name = String::from("toydb");
    let r = &name;  // r's lifetime starts here
    println!("{}", r);  // r's lifetime ends here (last use)
    // name is still valid here
}
```

The compiler tracks that `r` borrows from `name`, so `name` cannot be dropped while `r` is still in use. This is usually invisible -- the compiler infers it.

### When lifetimes become visible

When a function returns a reference, the compiler needs to know: which input does the output borrow from? Sometimes it cannot figure this out on its own:

```rust,ignore
// This does NOT compile:
fn longest(a: &str, b: &str) -> &str {
    if a.len() > b.len() { a } else { b }
}
```

The return value borrows from either `a` or `b`, but the compiler does not know which. You must add a **lifetime annotation**:

```rust
fn longest<'a>(a: &'a str, b: &'a str) -> &'a str {
    if a.len() > b.len() { a } else { b }
}

fn main() {
    let a = String::from("hello");
    let b = String::from("hi");
    let result = longest(&a, &b);
    println!("Longest: {}", result);
}
```

The `'a` (pronounced "lifetime a" or "tick a") is a lifetime parameter. It says: "the returned `&str` is valid for as long as both input `&str`s are valid."

> **Analogy: "Promise to return the book before the library closes"**
>
> When you borrow a book from the library, there is an implicit promise: you will return it before the library closes. The lifetime annotation is that promise made explicit. `'a` says "this reference will be valid for at least this long."
>
> If you borrow books from two libraries that close at different times (10 PM and 8 PM), the promise must be based on the earlier closing time (8 PM). That is why `'a` constrains the return value to the *shorter* of the two input lifetimes.

### Lifetime annotations on structs

When a struct holds a reference, it needs a lifetime annotation:

```rust,ignore
struct Transaction<'a> {
    store: &'a Store,      // borrows from a Store
    version: u64,
}
```

This says: a `Transaction` cannot outlive the `Store` it borrows from. If the `Store` is dropped, every `Transaction` referencing it becomes invalid. The compiler enforces this.

### When lifetimes get in the way

In practice, lifetimes are most annoying when a struct holds a reference. The compiler forces you to thread lifetime parameters through every struct and function that touches the reference. Sometimes the cleanest solution is to avoid references entirely and own the data:

```rust,ignore
// Instead of borrowing (requires lifetime annotations everywhere):
struct Transaction<'a> {
    store: &'a Store,
}

// Own the data (no lifetime annotations needed):
struct Transaction {
    store: Store,
}

// Or use shared ownership (no lifetime annotations needed):
use std::sync::Arc;
struct Transaction {
    store: Arc<Store>,
}
```

For our MVCC implementation, we will own the data in the `Transaction` struct. This avoids lifetime gymnastics while still being safe.

> **What just happened?**
>
> We learned three strategies for handling data in structs:
> 1. **Borrow it** (`&T`) -- requires lifetime annotations, but no copying
> 2. **Own it** (`T`) -- no lifetime annotations, but the data is moved or cloned
> 3. **Share it** (`Arc<T>`) -- no lifetime annotations, reference-counted ownership
>
> There is no single best choice. Borrowing is most efficient but most complex. Owning is simplest but may require cloning. Sharing is flexible but has a small runtime cost. We will use owning for now and introduce `Arc` in later chapters.

### Common mistakes with references

**Mistake: Returning a reference to a local variable**

```rust,ignore
fn make_greeting() -> &str {
    let s = String::from("hello");
    &s  // ERROR: s is dropped at the end of this function
}
```

The string `s` is created inside the function and destroyed when the function returns. Returning a reference to it would be a dangling pointer. Fix: return the owned `String` instead:

```rust,ignore
fn make_greeting() -> String {
    String::from("hello")  // move the owned String out
}
```

**Mistake: Modifying data while a shared reference exists**

```rust,ignore
let mut data = vec![1, 2, 3];
let first = &data[0];    // shared reference to first element
data.push(4);            // ERROR: push might reallocate, invalidating first
println!("{}", first);
```

`push` might move the vector's data to a new memory location, which would make `first` point to freed memory. The compiler catches this.

**Mistake: Thinking lifetimes change how long data lives**

Lifetime annotations do not change when data is created or destroyed. They are a description, not a command. `'a` says "this reference is valid for at least this long" -- it does not extend the life of the data.

---
