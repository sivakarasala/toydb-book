## Spotlight: Variables, Types & HashMap

Every chapter in this book has one **spotlight concept** — the Rust idea we dig into deeply. This chapter's spotlight is **variables, types, and HashMap** — the foundation of every data structure you will build.

### Variables: `let` bindings and immutability by default

In Rust, you declare variables with `let`. Unlike most languages you know, variables are **immutable by default**:

```rust
let name = "toydb";       // immutable — cannot be reassigned
let mut version = 1;       // mutable — can be reassigned
version += 1;              // OK
// name = "mydb";          // ERROR: cannot assign twice to immutable variable
```

This is a deliberate design choice. Immutability by default means the compiler catches accidental mutations. You opt into mutability explicitly with `mut`, which makes your intent clear to anyone reading the code.

### Rust's core types

Rust is statically typed with strong type inference. You rarely need to annotate types — the compiler figures them out:

```rust
let count = 42;            // i32 — default integer type (signed 32-bit)
let pi = 3.14159;          // f64 — default float type (64-bit)
let name = "toydb";        // &str — a string slice (borrowed, read-only)
let owned = String::from("toydb");  // String — owned, heap-allocated
let active = true;         // bool
let max: u64 = 1_000_000;  // explicit annotation: unsigned 64-bit integer
```

The numeric types are explicit about their size: `i8`, `i16`, `i32`, `i64`, `i128` for signed integers; `u8`, `u16`, `u32`, `u64`, `u128` for unsigned. There is also `usize` — an unsigned integer the same width as a pointer — which you will use for indexing and counting.

### `String` vs `&str`

This trips up every newcomer. Rust has two main string types:

- **`&str`** — a *string slice*. A read-only view into string data. Zero-cost, does not own the data. This is what string literals like `"hello"` produce.
- **`String`** — an *owned string*. Heap-allocated, growable, owned by the variable. You create one with `String::from("hello")` or `"hello".to_string()`.

```rust
let key: &str = "user:1";                       // borrowed, read-only
let owned_key: String = String::from("user:1");  // owned, can be modified
```

When you store data in a `HashMap`, you need owned values — the map must own the keys and values it holds. You cannot hand it a `&str` borrowed from somewhere else, because that reference might become invalid. This is Rust's ownership system protecting you from dangling pointers.

> **Coming from JS/Python/Go?**
>
> | Concept | JavaScript | Python | Go | Rust |
> |---------|-----------|--------|----|----- |
> | Immutable binding | `const x = 5;` | *(no keyword)* | `const x = 5` | `let x = 5;` |
> | Mutable binding | `let x = 5;` | `x = 5` | `var x = 5` | `let mut x = 5;` |
> | String (owned) | `"hello"` (all strings) | `"hello"` (all strings) | `"hello"` (all strings) | `String::from("hello")` |
> | String (borrowed) | *(N/A — GC handles it)* | *(N/A — GC handles it)* | *(N/A — GC handles it)* | `"hello"` (`&str`) |
> | Hash map | `new Map()` | `dict()` or `{}` | `map[string]string{}` | `HashMap::new()` |
>
> Notice that Rust's `let` is closer to JavaScript's `const` than to JavaScript's `let`. And Rust is the only language in this table that distinguishes between owned and borrowed strings — because it is the only one without a garbage collector.

### HashMap: Rust's hash table

A `HashMap<K, V>` maps keys of type `K` to values of type `V`. It lives in the standard library:

```rust
use std::collections::HashMap;

let mut db: HashMap<String, String> = HashMap::new();

db.insert("name".to_string(), "toydb".to_string());  // insert a key-value pair
db.insert("version".to_string(), "0.1".to_string());

// get() returns Option<&V> — it might not exist
if let Some(value) = db.get("name") {
    println!("name = {}", value);  // prints: name = toydb
}

db.remove("version");              // remove a key
println!("has version? {}", db.contains_key("version")); // false
```

Three things to notice:

1. **`insert` takes owned values.** You pass `String`, not `&str`. The HashMap takes ownership of both key and value. This is why we call `.to_string()` — it converts the `&str` literal into an owned `String`.

2. **`get` returns `Option<&V>`.** Not the value directly. The key might not exist, and Rust forces you to handle that case. No `null`, no `undefined`, no `KeyError` exception. Just `Some(value)` or `None`.

3. **`remove` returns `Option<V>`.** It gives you back the owned value (transferring ownership from the map to you), or `None` if the key did not exist.

This is Rust's ownership system in action. When you `insert`, the map owns the data. When you `get`, you borrow it (the `&` in `&V`). When you `remove`, ownership transfers back to you. Every piece of data has exactly one owner at any time.

---
