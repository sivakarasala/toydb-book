## Spotlight: Serde & Derive Macros

Every chapter has one spotlight concept. This chapter's spotlight is **serde and derive macros** — Rust's approach to auto-generating code at compile time and the serialization framework that practically every Rust project depends on.

### What is serialization?

> **Analogy: Packing a suitcase**
>
> Imagine you have a desk with your laptop, three books, a water bottle, and some pens. You need to fit everything into a suitcase for travel. You cannot just toss the desk into the suitcase -- you need to fold everything flat, arrange items carefully, and pack them in a specific order so you can unpack them later and put everything back exactly where it was.
>
> Serialization is the same thing for data. Your Rust structs live in memory as complex, structured objects. To save them to a file or send them over a network, you need to "pack" them into a flat sequence of bytes. Deserialization is unpacking -- turning those bytes back into the original struct.

In code terms:

```rust,ignore
// A struct in memory (has structure, fields, types)
struct Point {
    x: f64,
    y: f64,
}

// Serialized to bytes (flat, no structure)
// [0, 0, 0, 0, 0, 0, 240, 63, 0, 0, 0, 0, 0, 0, 0, 64]
```

The struct `Point { x: 1.0, y: 2.0 }` becomes a flat sequence of 16 bytes. Those bytes can be written to a file, sent over a network, or stored in a database. When you read them back, you deserialize them into a `Point` again.

### Why we need serialization

Our BitCask storage engine from Chapter 3 stores data as `Vec<u8>` -- raw bytes. But we want to store structured data like this:

```rust,ignore
// We want to store this...
let name = Value::String("Alice".to_string());
let age = Value::Integer(30);
let active = Value::Boolean(true);

// ...into a storage engine that only understands bytes:
storage.set("user:1:name", /* bytes here */);
```

Serialization is the bridge between our typed Rust world and the raw bytes world of storage.

### The problem with doing it by hand

If you wanted to turn a struct into bytes without any libraries, you would write something like this:

```rust,ignore
struct Point {
    x: f64,
    y: f64,
}

impl Point {
    fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.x.to_le_bytes());
        buf.extend_from_slice(&self.y.to_le_bytes());
        buf
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < 16 {
            return Err("Not enough bytes for Point".to_string());
        }
        let x = f64::from_le_bytes(bytes[0..8].try_into().unwrap());
        let y = f64::from_le_bytes(bytes[8..16].try_into().unwrap());
        Ok(Point { x, y })
    }
}
```

This works. For one struct. Now imagine doing this for 50 types, some with nested structs, some with `Option` fields, some with `Vec`s. The boilerplate multiplies, bugs hide in byte offsets, and every time you change a struct, you must update two functions. This does not scale.

### What is serde?

Serde is Rust's serialization/deserialization framework. The name is a portmanteau: **ser**ialize + **de**serialize. It separates two concerns:

1. **Data model** -- your struct knows its fields and types
2. **Format** -- JSON, binary, TOML, YAML, MessagePack, etc.

Serde acts as the bridge. Your struct describes itself to serde (via derived traits), and a format crate (like `serde_json` or `bincode`) handles the actual byte layout. Change the format and your struct code stays the same. Change the struct and the format code stays the same.

### What are derive macros?

Before we see serde in action, you need to understand derive macros. You have already used one:

```rust,ignore
#[derive(Debug)]
struct Point {
    x: f64,
    y: f64,
}
```

When you write `#[derive(Debug)]`, you are telling the Rust compiler: "please write the `Debug` trait implementation for me." Without it, you would need to write this yourself:

```rust,ignore
use std::fmt;

impl fmt::Debug for Point {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Point")
            .field("x", &self.x)
            .field("y", &self.y)
            .finish()
    }
}
```

That is tedious and repetitive. The derive macro reads your struct definition at compile time, sees that it has fields `x` and `y`, and generates this implementation automatically. You never see the generated code, but it is there -- compiled into your program just like code you wrote by hand.

> **Analogy: Auto-generating boilerplate**
>
> Think of derive macros like a photocopier with a template. You hand it your struct definition (the original document), and it produces a perfectly filled-out implementation (the copies). Every time you add or remove a field, the copier automatically adjusts. No manual editing needed.

### Rust's built-in derive macros

Rust has several built-in derive macros that you will use constantly:

```rust
#[derive(Debug)]           // enables {:?} formatting for printing
#[derive(Clone)]           // enables .clone() to make copies
#[derive(PartialEq)]       // enables == and != comparisons
#[derive(Eq)]              // marker: equality is total (every value equals itself)
#[derive(Hash)]            // enables use as HashMap key
#[derive(Default)]         // enables Type::default() for a "zero value"
struct Example;
```

You can stack multiple derives on one struct:

```rust
#[derive(Debug, Clone, PartialEq)]
struct Point {
    x: f64,
    y: f64,
}

fn main() {
    let p = Point { x: 1.0, y: 2.0 };
    let p2 = p.clone();           // Clone
    println!("{:?}", p);          // Debug
    assert_eq!(p, p2);            // PartialEq
}
```

Each derive generates its own independent `impl` block. `Debug` does not need `Clone`, `Clone` does not need `PartialEq`. Derive what you need, nothing more.

> **What just happened?**
>
> `#[derive(...)]` is an attribute that tells the compiler to auto-generate trait implementations. Each trait name in the parentheses produces a separate `impl` block. The generated code is regular Rust -- no magic at runtime. The compiler does all the work before your program runs.

### Important rule: derives only work if all fields support the trait

```rust,ignore
#[derive(Clone)]
struct Wrapper {
    file: std::fs::File,  // ERROR: File does not implement Clone
}
```

If you try to derive `Clone` for a struct that contains a `File` (which cannot be cloned -- what would it mean to clone an open file?), the compiler gives a clear error. Every field must implement the trait you are deriving.

### Serde's derive macros

Serde provides two derive macros: `Serialize` and `Deserialize`. They work exactly like the built-in derives, but instead of generating `Debug` or `Clone` implementations, they generate code for converting your struct to and from any data format:

```rust,ignore
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
struct Point {
    x: f64,
    y: f64,
}
```

That `#[derive(Serialize, Deserialize)]` replaces all the manual `to_bytes`/`from_bytes` code we wrote earlier. The derive macro generates the implementation at compile time -- no runtime reflection, no dynamic dispatch, no overhead.

### What is bincode?

For a database, JSON is wasteful. The string `"42"` takes 4 bytes in JSON (the characters `"`, `4`, `2`, `"`), but only 8 bytes as a raw `i64` in binary -- and that 8 bytes holds any number up to 9.2 quintillion. Binary encoding is also faster to parse and produces a consistent byte layout.

Bincode is a binary format crate for serde. It is fast, compact, and deterministic -- the same value always produces the same bytes.

```rust,ignore
let point = Point { x: 1.0, y: 2.0 };
let bytes: Vec<u8> = bincode::serialize(&point).unwrap();
let decoded: Point = bincode::deserialize(&bytes).unwrap();
assert_eq!(point, decoded);
```

Three lines. Serialize to bytes, deserialize back, verify they match. Serde and bincode do all the heavy lifting.

> **What just happened?**
>
> We used two crates together:
> - **serde** provides the `Serialize` and `Deserialize` traits (and the derive macros to generate implementations)
> - **bincode** provides the actual format -- it knows how to lay out bytes for integers, strings, enums, etc.
>
> This separation means you can swap formats without changing your structs. Replace `bincode::serialize` with `serde_json::to_string` and you get JSON instead of binary. Your struct code does not change at all.

### JSON vs bincode: a quick comparison

| Aspect | JSON (`serde_json`) | Binary (`bincode`) |
|--------|--------------------|--------------------|
| Human-readable | Yes | No |
| Size | Larger (text + field names) | Smaller (just values) |
| Speed | Slower (text parsing) | Faster (fixed offsets) |
| Use case | APIs, config files, debugging | Storage, wire protocols |

For a database's internal storage, bincode wins on both size and speed. For debugging and configuration, JSON is better because you can read it.

---
