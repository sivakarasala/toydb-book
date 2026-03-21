# Chapter 4: Serialization — Turning Structs Into Bytes

Your database can store key-value pairs on disk. But look at what it actually stores: raw bytes. Just `Vec<u8>` — an untyped blob. Your database has no idea whether the bytes `[52, 50]` represent the number 42, the string "42", or something else entirely. It is like a filing cabinet that accepts any piece of paper but cannot tell you whether it contains a number, a name, or a shopping list.

A real database needs to understand its data. It needs to know that `42` is a number (so it can do math), that `"Alice"` is a string (so it can search text), and that `true` is a boolean (so it can filter results). This chapter builds that understanding.

You will create a typed `Value` system and learn how to convert Rust structs into bytes (serialization) and back again (deserialization). Along the way, you will discover one of Rust's most powerful features: derive macros that write code for you.

By the end of this chapter, you will have:

- A `Value` enum with typed variants (Null, Boolean, Integer, Float, String) that serializes to compact binary
- Round-trip tests proving that encode-then-decode preserves every type exactly
- A `Row` type that packs multiple values into a single byte sequence for storage
- A hand-rolled binary format to compare against the automatic approach
- A deep understanding of derive macros, serde, and how serialization works under the hood

---

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

## Exercise 1: Add Serde and Define a Typed Value

**Goal:** Replace raw `Vec<u8>` with a typed `Value` enum that knows whether it holds a number, a string, a boolean, or nothing. Add serde + bincode dependencies and derive `Serialize`/`Deserialize`.

### Step 1: Add dependencies to Cargo.toml

Open your `Cargo.toml` file and add two new dependencies:

```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
bincode = "1"
```

Let's understand each part:

- `serde = { version = "1", features = ["derive"] }` -- This adds serde version 1.x. The `features = ["derive"]` part is important: it enables the `#[derive(Serialize, Deserialize)]` macros. Without this feature flag, the derive macros are not available and you would have to implement the traits by hand (hundreds of lines of code).

- `bincode = "1"` -- This adds bincode version 1.x, our binary format encoder/decoder.

> **What just happened?**
>
> `Cargo.toml` is Rust's package manifest. The `[dependencies]` section lists external crates (libraries) your project uses. When you run `cargo build`, Cargo downloads these crates, compiles them, and links them into your project. The `features = ["derive"]` syntax enables optional functionality -- serde ships the derive macros as an optional feature to keep the core library small.

### Step 2: Create the Value enum

Create a new file `src/value.rs`. This will hold our typed value system.

```rust,ignore
use serde::{Serialize, Deserialize};

/// A database value. Every cell in every row is one of these variants.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Value {
    Null,
    Boolean(bool),
    Integer(i64),
    Float(f64),
    String(String),
}
```

Let's break down each part:

**The enum itself:** An enum (short for "enumeration") is a type that can be one of several variants. A `Value` is either `Null`, a `Boolean`, an `Integer`, a `Float`, or a `String`. It cannot be two things at once. It cannot be something else.

**Variants with data:** Some variants carry data inside them. `Boolean(bool)` means "a Boolean variant that contains a `bool` value." `Integer(i64)` means "an Integer variant that contains an `i64` value." `Null` carries no data -- it just means "nothing."

**The five derives:**

- `Debug` -- lets us print values with `{:?}` for debugging
- `Clone` -- lets us copy values (databases need to copy values around a lot)
- `PartialEq` -- lets us compare values with `==` and `!=` (essential for tests)
- `Serialize` -- lets serde convert values to bytes
- `Deserialize` -- lets serde convert bytes back to values

> **What just happened?**
>
> We defined a type that represents every possible value in our database. In SQL terms:
> - `Null` is SQL's `NULL`
> - `Boolean(true)` is SQL's `TRUE`
> - `Integer(42)` is a number like `42`
> - `Float(3.14)` is a decimal number like `3.14`
> - `String("Alice".to_string())` is a string like `'Alice'`
>
> The enum ensures that every value has a known type. You cannot accidentally treat a string as a number because the compiler will not let you -- you must match on the variant first.

### Step 3: Creating Values

Let's see how to create each variant:

```rust,ignore
let nothing = Value::Null;
let flag = Value::Boolean(true);
let age = Value::Integer(30);
let pi = Value::Float(3.14159);
let name = Value::String("Alice".to_string());
```

Notice that `String` values need `.to_string()`. This is because `"Alice"` is a string slice (`&str` -- a reference to text), but our enum stores a `String` (owned text). The `.to_string()` method creates an owned copy.

> **Common mistake: Forgetting `.to_string()`**
>
> ```rust,ignore
> let name = Value::String("Alice");  // ERROR!
> ```
>
> This fails because `"Alice"` has type `&str`, but the enum expects `String`. Fix it with `.to_string()`:
>
> ```rust,ignore
> let name = Value::String("Alice".to_string());  // OK
> ```
>
> In Rust, `&str` is a borrowed reference to text (you are looking at someone else's string), and `String` is owned text (you have your own copy). The enum needs to own its data, so it uses `String`.

### Step 4: Add Display for human-readable output

We want to print values in a readable format. Rust has a trait called `Display` that controls how a type looks when you use `{}` in `println!`:

```rust,ignore
use std::fmt;

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Null => write!(f, "NULL"),
            Value::Boolean(b) => write!(f, "{}", b),
            Value::Integer(i) => write!(f, "{}", i),
            Value::Float(v) => write!(f, "{}", v),
            Value::String(s) => write!(f, "'{}'", s),
        }
    }
}
```

Let's understand this piece by piece:

**`impl fmt::Display for Value`** -- We are implementing the `Display` trait for our `Value` type. This tells Rust "here is how to format a Value as text."

**`fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result`** -- This is the method signature required by the `Display` trait. `&self` is a reference to the value we are formatting. `f` is the formatter we write to. `fmt::Result` is the return type (either success or a formatting error).

**`match self { ... }`** -- We match on which variant `self` is. Each arm handles one variant. This is called **pattern matching**, and it is how you work with enums in Rust.

**`write!(f, "...")`** -- This is like `println!` but writes to the formatter instead of the screen. It returns a `fmt::Result`.

> **What just happened?**
>
> We told Rust how to display each variant as text. Now we can write:
> ```rust,ignore
> let v = Value::Integer(42);
> println!("{}", v);  // prints: 42
>
> let s = Value::String("Alice".to_string());
> println!("{}", s);  // prints: 'Alice'
> ```
>
> `Debug` (from `#[derive(Debug)]`) prints the Rust representation: `Integer(42)`. `Display` (which we wrote by hand) prints the human-readable version: `42`. Both are useful -- `Debug` for developers, `Display` for users.

### Step 5: Add helper methods to extract values

We often need to get the inner value out of a `Value`. Let's add some helper methods:

```rust,ignore
impl Value {
    /// Returns true if this value is NULL.
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    /// Tries to extract an integer. Returns None if the value is not an Integer.
    pub fn as_integer(&self) -> Option<i64> {
        match self {
            Value::Integer(i) => Some(*i),
            _ => None,
        }
    }

    /// Tries to extract a string reference.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s.as_str()),
            _ => None,
        }
    }

    /// Tries to extract a boolean.
    pub fn as_boolean(&self) -> Option<bool> {
        match self {
            Value::Boolean(b) => Some(*b),
            _ => None,
        }
    }

    /// Tries to extract a float.
    pub fn as_float(&self) -> Option<f64> {
        match self {
            Value::Float(v) => Some(*v),
            _ => None,
        }
    }
}
```

Let's understand the new concepts here:

**`matches!` macro:** `matches!(self, Value::Null)` returns `true` if `self` is `Value::Null`, `false` otherwise. It is shorthand for a full `match` with `true`/`false` arms.

**`Option<i64>`:** This return type says "maybe an i64, maybe nothing." `Some(42)` means "here is the value." `None` means "this value is not an integer." This is safer than returning -1 or throwing an exception -- the caller must handle both cases.

**`Some(*i)`:** The `*i` dereferences the `i64`. When you pattern-match `Value::Integer(i)`, `i` is a reference to the inner value (`&i64`). The `*` copies it out. For small types like `i64` and `bool`, this copy is trivial (8 bytes or less).

**`_ => None`:** The underscore `_` is a wildcard pattern -- it matches everything. This arm says "if the value is anything other than the expected variant, return `None`."

> **What just happened?**
>
> We added methods that safely extract the inner value from each variant. These return `Option` -- Rust's way of saying "this might not have a value." This forces the caller to check:
>
> ```rust,ignore
> let v = Value::Integer(42);
> if let Some(n) = v.as_integer() {
>     println!("The number is {}", n);  // runs, prints 42
> }
>
> let v = Value::String("hello".to_string());
> if let Some(n) = v.as_integer() {
>     println!("The number is {}", n);  // does NOT run -- it is a String, not an Integer
> }
> ```

### Step 6: Register the module

In your `src/lib.rs` (or `src/main.rs`), add:

```rust,ignore
pub mod value;
```

This tells Rust that there is a module called `value` in the file `src/value.rs`.

### Step 7: Verify it compiles

```
$ cargo build
   Compiling serde v1.0.xxx
   Compiling bincode v1.3.x
   Compiling toydb v0.1.0
    Finished dev [unoptimized + debuginfo] target(s) in 5.23s
```

If you see errors about "cannot find derive macro Serialize," check that your `serde` dependency includes `features = ["derive"]`.

> **Common mistake: Missing the derive feature**
>
> ```toml
> # WRONG -- derive macros will not be available
> serde = "1"
>
> # RIGHT -- derive macros are enabled
> serde = { version = "1", features = ["derive"] }
> ```
>
> Without `features = ["derive"]`, serde does not include the procedural macros needed for `#[derive(Serialize, Deserialize)]`.

---

## Exercise 2: Encode, Decode, and Round-Trip Test

**Goal:** Serialize `Value` instances to bytes using bincode, deserialize them back, and prove the round trip is lossless with tests.

### Step 1: Add encode/decode methods

Add to `src/value.rs`:

```rust,ignore
impl Value {
    /// Serialize this value to a compact binary representation.
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        bincode::serialize(self)
            .map_err(|e| format!("Serialization failed: {}", e))
    }

    /// Deserialize a value from bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        bincode::deserialize(bytes)
            .map_err(|e| format!("Deserialization failed: {}", e))
    }
}
```

That is the entire serialization layer. Two methods. Let's understand each piece:

**`bincode::serialize(self)`** -- This takes a reference to our `Value` and returns `Result<Vec<u8>, Box<bincode::ErrorKind>>`. If serialization succeeds, you get a `Vec<u8>` (a vector of bytes). If it fails, you get an error.

**`.map_err(|e| format!("..."))`** -- This converts bincode's error type into a simple `String`. The `map_err` method takes a closure (an inline function) that transforms the error. We do this because `String` is easier to work with than bincode's internal error type. (We will improve error handling in later chapters.)

**`bincode::deserialize(bytes)`** -- This takes a byte slice (`&[u8]`) and tries to reconstruct a `Value`. If the bytes are valid, you get `Ok(Value::...)`. If the bytes are corrupted or the wrong format, you get an error.

> **What just happened?**
>
> We added two methods that convert `Value` to bytes and back. The magic is that we did not write any byte-manipulation code. Serde's derive macros (from Step 2 of Exercise 1) generated all the serialization logic at compile time. We just call `bincode::serialize` and it knows how to handle every variant of our enum -- `Null`, `Boolean`, `Integer`, `Float`, and `String` -- because the derived code told it about every field.

### Step 2: Understand what `Result` means here

Both methods return `Result<T, String>`. Let's trace what happens:

```rust,ignore
// Happy path:
let v = Value::Integer(42);
let bytes = v.to_bytes();  // Ok(vec![2, 0, 0, 0, 42, 0, 0, 0, 0, 0, 0, 0])

// Error path:
let bad_bytes = vec![0xFF, 0xFF, 0xFF];
let v = Value::from_bytes(&bad_bytes);  // Err("Deserialization failed: ...")
```

The caller uses `.unwrap()` in tests (which panics on errors) or `?` in production code (which propagates errors). We covered this in Chapter 3.

### Step 3: Write round-trip tests

A **round-trip test** proves that serializing and then deserializing produces the same value. This is the most important test for any serialization code:

```rust,ignore
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_null() {
        let original = Value::Null;
        let bytes = original.to_bytes().unwrap();
        let decoded = Value::from_bytes(&bytes).unwrap();
        assert_eq!(original, decoded);
    }

    #[test]
    fn round_trip_boolean() {
        for val in [true, false] {
            let original = Value::Boolean(val);
            let bytes = original.to_bytes().unwrap();
            let decoded = Value::from_bytes(&bytes).unwrap();
            assert_eq!(original, decoded);
        }
    }

    #[test]
    fn round_trip_integer() {
        let test_cases = vec![0_i64, 1, -1, i64::MAX, i64::MIN, 42];
        for val in test_cases {
            let original = Value::Integer(val);
            let bytes = original.to_bytes().unwrap();
            let decoded = Value::from_bytes(&bytes).unwrap();
            assert_eq!(original, decoded);
        }
    }

    #[test]
    fn round_trip_float() {
        let test_cases = vec![0.0_f64, 1.5, -3.14, f64::MAX, f64::MIN];
        for val in test_cases {
            let original = Value::Float(val);
            let bytes = original.to_bytes().unwrap();
            let decoded = Value::from_bytes(&bytes).unwrap();
            assert_eq!(original, decoded);
        }
    }

    #[test]
    fn round_trip_string() {
        let test_cases = vec![
            "".to_string(),
            "hello".to_string(),
            "Hello, World!".to_string(),
            "a".repeat(10_000),
        ];
        for val in test_cases {
            let original = Value::String(val);
            let bytes = original.to_bytes().unwrap();
            let decoded = Value::from_bytes(&bytes).unwrap();
            assert_eq!(original, decoded);
        }
    }
}
```

Let's understand what is happening in the tests:

**`#[cfg(test)]`** -- This attribute means "only compile this module when running tests." The test code is not included in your final binary.

**`mod tests { use super::*; }`** -- This creates a sub-module called `tests` and imports everything from the parent module (`super::*` means "everything from one level up").

**`#[test]`** -- Marks a function as a test case. `cargo test` will find and run it.

**`assert_eq!(original, decoded)`** -- Checks that two values are equal. If they are not, the test fails with a helpful error message showing both values. This works because we derived `PartialEq`.

**`"a".repeat(10_000)`** -- Creates a string of 10,000 `a` characters. This tests that large strings survive the round trip. The underscore in `10_000` is just for readability -- Rust ignores it.

> **What just happened?**
>
> We wrote tests for every variant of our `Value` enum. Each test follows the same pattern:
> 1. Create an original value
> 2. Serialize it to bytes
> 3. Deserialize the bytes back to a value
> 4. Assert the original and decoded values are equal
>
> If any step fails, the test catches it. We test edge cases too: empty strings, zero, negative numbers, the largest and smallest possible numbers.

### Step 4: Add tests for error cases

```rust,ignore
    #[test]
    fn different_types_produce_different_bytes() {
        let int_bytes = Value::Integer(42).to_bytes().unwrap();
        let str_bytes = Value::String("42".to_string()).to_bytes().unwrap();
        assert_ne!(int_bytes, str_bytes);
    }

    #[test]
    fn null_is_compact() {
        let bytes = Value::Null.to_bytes().unwrap();
        // Null should be very small -- just the enum discriminant
        assert!(bytes.len() <= 4, "Null serialized to {} bytes", bytes.len());
    }

    #[test]
    fn corrupted_bytes_return_error() {
        let result = Value::from_bytes(&[0xFF, 0xFF, 0xFF]);
        assert!(result.is_err());
    }
```

**`assert_ne!`** -- The opposite of `assert_eq!`. It asserts that two values are *not* equal. The integer 42 and the string "42" must produce different bytes -- otherwise our database would confuse numbers with strings.

**`assert!(result.is_err())`** -- Checks that the result is an error. Random garbage bytes should not deserialize into a valid `Value`.

### Step 5: Run the tests

```
$ cargo test value::tests
running 8 tests
test value::tests::round_trip_null ... ok
test value::tests::round_trip_boolean ... ok
test value::tests::round_trip_integer ... ok
test value::tests::round_trip_float ... ok
test value::tests::round_trip_string ... ok
test value::tests::different_types_produce_different_bytes ... ok
test value::tests::null_is_compact ... ok
test value::tests::corrupted_bytes_return_error ... ok

test result: ok. 8 passed; 0 failed; 0 ignored
```

### Step 6: Inspect the byte layout

Add a test that prints the actual bytes. This helps you understand what bincode produces:

```rust,ignore
    #[test]
    fn inspect_byte_layout() {
        let values = vec![
            Value::Null,
            Value::Boolean(true),
            Value::Integer(42),
            Value::Float(3.14),
            Value::String("hi".to_string()),
        ];

        for val in &values {
            let bytes = val.to_bytes().unwrap();
            println!("{:>20} -> {} bytes: {:?}", val, bytes.len(), bytes);
        }
    }
```

```
$ cargo test inspect_byte_layout -- --nocapture
                NULL -> 4 bytes: [0, 0, 0, 0]
                true -> 5 bytes: [1, 0, 0, 0, 1]
                  42 -> 12 bytes: [2, 0, 0, 0, 42, 0, 0, 0, 0, 0, 0, 0]
                3.14 -> 12 bytes: [3, 0, 0, 0, 31, 133, 235, 81, 184, 30, 9, 64]
                'hi' -> 14 bytes: [4, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 104, 105]
```

> **What just happened?**
>
> The `-- --nocapture` flag tells `cargo test` to show `println!` output (normally it is hidden for passing tests).
>
> Look at the pattern in the bytes:
> - The first 4 bytes are the **enum discriminant**: 0 for Null, 1 for Boolean, 2 for Integer, 3 for Float, 4 for String. This tells the deserializer which variant to expect.
> - After the discriminant comes the **variant data**: 1 byte for booleans, 8 bytes for integers and floats, 8 bytes of length + the actual characters for strings.
>
> The `{:>20}` format means "right-align in a 20-character field" -- it makes the output line up neatly.

> **Common mistake: Worrying about the exact bytes**
>
> The specific bytes bincode produces depend on the bincode version and configuration. Do not hardcode expected bytes in your tests. Instead, test the round trip: serialize then deserialize and check equality. The exact bytes are an implementation detail.

---

## Exercise 3: Build a Row Type for Structured Storage

**Goal:** Define a `Row` as a vector of `Value`s and a `Table` as a collection of rows with named columns. This moves our database from raw key-value pairs to structured data.

### Step 1: Define the Row struct

Add to `src/value.rs`:

```rust,ignore
/// A row is an ordered sequence of values -- one per column.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Row {
    pub values: Vec<Value>,
}
```

A `Row` holds a `Vec<Value>` -- a vector (growable list) of values. Each value corresponds to one column. A row for a "users" table with columns `name`, `age`, `active` might be:

```rust,ignore
Row { values: vec![
    Value::String("Alice".to_string()),
    Value::Integer(30),
    Value::Boolean(true),
]}
```

Notice that `Row` also derives `Serialize` and `Deserialize`. This works because `Vec<Value>` is serializable (serde supports `Vec` out of the box) and `Value` is serializable (we derived it earlier). Serde handles nesting automatically.

### Step 2: Add methods to Row

```rust,ignore
impl Row {
    pub fn new(values: Vec<Value>) -> Self {
        Row { values }
    }

    /// Serialize the entire row to bytes for storage.
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        bincode::serialize(self)
            .map_err(|e| format!("Row serialization failed: {}", e))
    }

    /// Deserialize a row from bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        bincode::deserialize(bytes)
            .map_err(|e| format!("Row deserialization failed: {}", e))
    }
}
```

**`Row { values }`** -- This is a shorthand. When the variable name matches the field name, you can write `Row { values }` instead of `Row { values: values }`. This is called **field init shorthand**.

### Step 3: Add Display for Row

```rust,ignore
impl fmt::Display for Row {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let parts: Vec<String> = self.values.iter().map(|v| v.to_string()).collect();
        write!(f, "({})", parts.join(", "))
    }
}
```

Let's break down this line by line:

**`self.values.iter()`** -- Creates an iterator over the values in the row.

**`.map(|v| v.to_string())`** -- Transforms each `Value` into a `String` by calling our `Display` implementation. The `|v|` is a closure parameter -- `v` is each value.

**`.collect()`** -- Collects the iterator into a `Vec<String>`.

**`parts.join(", ")`** -- Joins the strings with ", " between them. `["Alice", "30", "true"]` becomes `"Alice, 30, true"`.

**`write!(f, "({})", ...)`** -- Wraps the result in parentheses.

Result: `Row { values: [String("Alice"), Integer(30)] }` displays as `('Alice', 30)`.

### Step 4: Build a simple Table

Create `src/table.rs`:

```rust,ignore
use crate::value::{Row, Value};
use std::collections::BTreeMap;

/// A simple in-memory table with named columns and typed rows.
pub struct Table {
    pub name: String,
    pub columns: Vec<String>,
    rows: BTreeMap<i64, Row>,
    next_id: i64,
}
```

**`crate::value::{Row, Value}`** -- Imports `Row` and `Value` from our `value` module. `crate::` means "from the root of this project."

**`BTreeMap<i64, Row>`** -- A sorted map from integer IDs to rows. `BTreeMap` keeps keys in sorted order (unlike `HashMap`). We use `i64` as the key type because row IDs are integers.

**`next_id`** -- An auto-increment counter for generating unique row IDs.

### Step 5: Implement Table methods

```rust,ignore
impl Table {
    pub fn new(name: &str, columns: Vec<String>) -> Self {
        Table {
            name: name.to_string(),
            columns,
            rows: BTreeMap::new(),
            next_id: 1,
        }
    }

    /// Insert a row. Returns the auto-generated ID.
    pub fn insert(&mut self, values: Vec<Value>) -> Result<i64, String> {
        if values.len() != self.columns.len() {
            return Err(format!(
                "Expected {} values, got {}",
                self.columns.len(),
                values.len()
            ));
        }
        let id = self.next_id;
        self.next_id += 1;
        self.rows.insert(id, Row::new(values));
        Ok(id)
    }

    /// Get a row by ID.
    pub fn get(&self, id: i64) -> Option<&Row> {
        self.rows.get(&id)
    }

    /// Return all rows in ID order.
    pub fn scan(&self) -> Vec<(i64, &Row)> {
        self.rows.iter().map(|(&id, row)| (id, row)).collect()
    }

    /// Delete a row by ID. Returns true if the row existed.
    pub fn delete(&mut self, id: i64) -> bool {
        self.rows.remove(&id).is_some()
    }
}
```

Let's understand the insert method step by step:

**Column count validation:** If someone tries to insert 2 values into a 3-column table, we return an error immediately. `return Err(...)` exits the function early with an error.

**Auto-increment ID:** We grab the current `next_id`, then increment it. This ensures every row gets a unique ID: 1, 2, 3, etc.

**`self.rows.insert(id, Row::new(values))`** -- Puts the row into the BTreeMap at the given ID.

**`Ok(id)`** -- Returns the ID of the newly inserted row. The caller can use this to fetch the row later.

> **What just happened?**
>
> We built a table abstraction. Think of it like a spreadsheet: columns have names, each row has one value per column, and every row has a unique ID. The `insert` method validates that you provide the right number of values. The `get` method retrieves a row by ID. The `scan` method returns all rows in order. The `delete` method removes a row.

### Step 6: Add a display method for pretty-printing

```rust,ignore
impl Table {
    /// Pretty-print the table contents.
    pub fn display(&self) {
        // Header
        print!("{:>4} | ", "id");
        for col in &self.columns {
            print!("{:>12} | ", col);
        }
        println!();
        println!("{}", "-".repeat(6 + self.columns.len() * 15));

        // Rows
        for (&id, row) in &self.rows {
            print!("{:>4} | ", id);
            for val in &row.values {
                print!("{:>12} | ", val);
            }
            println!();
        }
    }
}
```

**`{:>4}`** -- Right-align in a 4-character-wide field.
**`{:>12}`** -- Right-align in a 12-character-wide field.
**`"-".repeat(n)`** -- Creates a string of `n` dashes for the separator line.

### Step 7: Test the table

```rust,ignore
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_retrieve() {
        let mut table = Table::new("users", vec![
            "name".to_string(),
            "age".to_string(),
            "active".to_string(),
        ]);

        let id1 = table.insert(vec![
            Value::String("Alice".to_string()),
            Value::Integer(30),
            Value::Boolean(true),
        ]).unwrap();

        let id2 = table.insert(vec![
            Value::String("Bob".to_string()),
            Value::Integer(25),
            Value::Boolean(false),
        ]).unwrap();

        assert_eq!(id1, 1);
        assert_eq!(id2, 2);

        let row = table.get(id1).unwrap();
        assert_eq!(row.values[0], Value::String("Alice".to_string()));
        assert_eq!(row.values[1], Value::Integer(30));
    }

    #[test]
    fn wrong_column_count_is_error() {
        let mut table = Table::new("users", vec![
            "name".to_string(),
            "age".to_string(),
        ]);

        let result = table.insert(vec![Value::String("Alice".to_string())]);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Expected 2 values, got 1"));
    }

    #[test]
    fn delete_removes_row() {
        let mut table = Table::new("users", vec!["name".to_string()]);
        let id = table.insert(vec![Value::String("Alice".to_string())]).unwrap();

        assert!(table.get(id).is_some());
        assert!(table.delete(id));
        assert!(table.get(id).is_none());
        assert!(!table.delete(id)); // already deleted
    }

    #[test]
    fn scan_returns_all_rows_ordered() {
        let mut table = Table::new("t", vec!["v".to_string()]);
        table.insert(vec![Value::Integer(3)]).unwrap();
        table.insert(vec![Value::Integer(1)]).unwrap();
        table.insert(vec![Value::Integer(2)]).unwrap();

        let rows: Vec<i64> = table.scan()
            .iter()
            .map(|(id, _)| *id)
            .collect();

        assert_eq!(rows, vec![1, 2, 3]); // ordered by insertion ID
    }

    #[test]
    fn row_round_trip_through_bytes() {
        let row = Row::new(vec![
            Value::String("Alice".to_string()),
            Value::Integer(30),
            Value::Boolean(true),
            Value::Null,
        ]);

        let bytes = row.to_bytes().unwrap();
        let decoded = Row::from_bytes(&bytes).unwrap();
        assert_eq!(row, decoded);
    }
}
```

> **What just happened?**
>
> We tested every table operation:
> - Insert returns incrementing IDs (1, 2, 3...)
> - Get retrieves the correct row
> - Inserting the wrong number of values is an error
> - Delete removes the row and returns whether it existed
> - Scan returns all rows in ID order
> - Rows survive the serialize/deserialize round trip

### Step 8: Register the table module

In `src/lib.rs`:

```rust,ignore
pub mod table;
```

Run all the tests:

```
$ cargo test
running 13 tests
...
test result: ok. 13 passed; 0 failed; 0 ignored
```

---

## Exercise 4: Build a Custom Binary Format by Hand

**Goal:** Implement a manual binary encoding without serde. This exercise exists to show you what serde does for you -- and to build intuition for how binary formats work under the hood.

### Step 1: Understand the format

We will define a simple format:

```
[type_tag: 1 byte] [payload: variable]

Type tags:
  0x00 = Null        (no payload)
  0x01 = Boolean     (1 byte: 0x00 or 0x01)
  0x02 = Integer     (8 bytes: little-endian i64)
  0x03 = Float       (8 bytes: little-endian f64)
  0x04 = String      (4 bytes length + N bytes UTF-8)
```

The first byte tells us what type of value follows. Then we read the appropriate number of payload bytes based on the type.

> **Analogy: Labeled storage boxes**
>
> Imagine a warehouse where every box has a colored sticker on it: red = fragile dishes, blue = books, green = clothes. The sticker tells the worker how to handle the box before they open it. Our type tag is the same idea -- one byte that tells the decoder what comes next.

### Step 2: Understand `to_le_bytes` and `from_le_bytes`

Before we write the encoding, you need to understand how numbers become bytes:

```rust,ignore
let n: i64 = 42;
let bytes = n.to_le_bytes();           // [42, 0, 0, 0, 0, 0, 0, 0]
let back = i64::from_le_bytes(bytes);  // 42
assert_eq!(n, back);
```

**`to_le_bytes()`** -- Converts a number to bytes in **little-endian** order (least significant byte first). An `i64` produces exactly 8 bytes. A `u32` produces exactly 4 bytes.

**`from_le_bytes(bytes)`** -- Converts bytes back to a number. You need to provide the exact right number of bytes (8 for `i64`, 4 for `u32`).

**Why "little-endian"?** Computers store multi-byte numbers in a specific order. Little-endian (LE) means the smallest byte comes first. The number 258 is stored as `[2, 1, 0, 0]` -- the "2" (least significant) is first. Most modern CPUs (x86, ARM) use little-endian, so this matches the native byte order and avoids conversion overhead.

### Step 3: Implement manual encoding

Add to `src/value.rs`:

```rust,ignore
impl Value {
    /// Manually encode this value to a type-tagged binary format.
    /// This exists for learning -- in production, use to_bytes() (serde).
    pub fn encode_manual(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        match self {
            Value::Null => {
                buf.push(0x00);
            }
            Value::Boolean(b) => {
                buf.push(0x01);
                buf.push(if *b { 1 } else { 0 });
            }
            Value::Integer(i) => {
                buf.push(0x02);
                buf.extend_from_slice(&i.to_le_bytes());
            }
            Value::Float(f) => {
                buf.push(0x03);
                buf.extend_from_slice(&f.to_le_bytes());
            }
            Value::String(s) => {
                buf.push(0x04);
                let bytes = s.as_bytes();
                let len = bytes.len() as u32;
                buf.extend_from_slice(&len.to_le_bytes());
                buf.extend_from_slice(bytes);
            }
        }
        buf
    }
}
```

Let's trace through each variant:

**Null:** Just write the type tag `0x00`. One byte total. There is nothing else to encode.

**Boolean:** Write the type tag `0x01`, then one byte for the value: `1` for true, `0` for false. Two bytes total.

**Integer:** Write the type tag `0x02`, then the 8-byte little-endian representation of the `i64`. Nine bytes total.

**Float:** Write the type tag `0x03`, then the 8-byte little-endian representation of the `f64`. Nine bytes total.

**String:** Write the type tag `0x04`, then a 4-byte `u32` length, then the actual UTF-8 bytes. Variable total: 5 bytes + string length.

**`buf.push(0x00)`** -- Adds one byte to the end of the vector. `0x00` is hexadecimal for zero.

**`buf.extend_from_slice(&i.to_le_bytes())`** -- Adds all 8 bytes of the little-endian representation to the vector.

**`s.as_bytes()`** -- Converts a Rust string to its UTF-8 byte representation. The string "hi" becomes `[104, 105]` (ASCII values of 'h' and 'i').

> **What just happened?**
>
> We manually built the byte representation of each value variant. The key insight is that every encoded value starts with a type tag -- one byte that identifies the variant. The decoder will read this tag first to know what type of data follows and how many bytes to read.

### Step 4: Implement manual decoding

```rust,ignore
impl Value {
    /// Manually decode a value from the custom binary format.
    /// Returns the decoded value and the number of bytes consumed.
    pub fn decode_manual(bytes: &[u8]) -> Result<(Self, usize), String> {
        if bytes.is_empty() {
            return Err("Empty input".to_string());
        }

        let tag = bytes[0];
        match tag {
            0x00 => Ok((Value::Null, 1)),

            0x01 => {
                if bytes.len() < 2 {
                    return Err("Boolean: not enough bytes".to_string());
                }
                Ok((Value::Boolean(bytes[1] != 0), 2))
            }

            0x02 => {
                if bytes.len() < 9 {
                    return Err("Integer: not enough bytes".to_string());
                }
                let arr: [u8; 8] = bytes[1..9]
                    .try_into()
                    .map_err(|_| "Integer: invalid bytes".to_string())?;
                Ok((Value::Integer(i64::from_le_bytes(arr)), 9))
            }

            0x03 => {
                if bytes.len() < 9 {
                    return Err("Float: not enough bytes".to_string());
                }
                let arr: [u8; 8] = bytes[1..9]
                    .try_into()
                    .map_err(|_| "Float: invalid bytes".to_string())?;
                Ok((Value::Float(f64::from_le_bytes(arr)), 9))
            }

            0x04 => {
                if bytes.len() < 5 {
                    return Err("String: not enough bytes for length".to_string());
                }
                let len_arr: [u8; 4] = bytes[1..5]
                    .try_into()
                    .map_err(|_| "String: invalid length bytes".to_string())?;
                let len = u32::from_le_bytes(len_arr) as usize;

                if bytes.len() < 5 + len {
                    return Err(format!(
                        "String: expected {} bytes, got {}",
                        len,
                        bytes.len() - 5
                    ));
                }
                let s = std::str::from_utf8(&bytes[5..5 + len])
                    .map_err(|e| format!("String: invalid UTF-8: {}", e))?;
                Ok((Value::String(s.to_string()), 5 + len))
            }

            _ => Err(format!("Unknown type tag: 0x{:02X}", tag)),
        }
    }
}
```

The return type `Result<(Self, usize), String>` includes the number of bytes consumed. This is essential when decoding multiple values from a stream -- you need to know where one value ends and the next begins.

Let's trace the Integer decoding:

1. **Check we have enough bytes:** `if bytes.len() < 9` -- we need 1 byte for the tag + 8 bytes for the i64 = 9 bytes minimum.

2. **Extract the 8 data bytes:** `bytes[1..9]` is a slice of bytes from position 1 to position 8 (9 is exclusive). This skips the type tag.

3. **Convert slice to fixed-size array:** `.try_into()` converts the slice `&[u8]` into `[u8; 8]`. This can fail if the slice is the wrong length, so it returns a `Result`.

4. **Convert bytes to number:** `i64::from_le_bytes(arr)` reconstructs the original number.

5. **Return the value and byte count:** `Ok((Value::Integer(n), 9))` -- we decoded an integer and consumed 9 bytes.

> **What just happened?**
>
> We reversed the encoding process. The decoder reads the type tag, branches on it, reads the appropriate number of bytes, and reconstructs the value. Each branch validates that enough bytes are available before trying to read them -- this prevents crashes from truncated or corrupted data.
>
> The `0x{:02X}` format specifier prints a number as hexadecimal with at least 2 digits. So tag `255` prints as `0xFF` and tag `0` prints as `0x00`.

### Step 5: Encode and decode rows manually

```rust,ignore
impl Row {
    /// Manually encode a row: [value_count: 4 bytes] [value1] [value2] ...
    pub fn encode_manual(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        let count = self.values.len() as u32;
        buf.extend_from_slice(&count.to_le_bytes());
        for val in &self.values {
            buf.extend(val.encode_manual());
        }
        buf
    }

    /// Manually decode a row from bytes.
    pub fn decode_manual(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < 4 {
            return Err("Row: not enough bytes for count".to_string());
        }
        let count_arr: [u8; 4] = bytes[0..4]
            .try_into()
            .map_err(|_| "Row: invalid count bytes".to_string())?;
        let count = u32::from_le_bytes(count_arr) as usize;

        let mut values = Vec::with_capacity(count);
        let mut offset = 4;

        for i in 0..count {
            if offset >= bytes.len() {
                return Err(format!("Row: unexpected end at value {}", i));
            }
            let (val, consumed) = Value::decode_manual(&bytes[offset..])?;
            values.push(val);
            offset += consumed;
        }

        Ok(Row::new(values))
    }
}
```

The row encoding is straightforward:

1. Write a 4-byte count (how many values in the row)
2. Write each value using `encode_manual`

The row decoding reads the count, then loops that many times, decoding one value each iteration. The `offset` variable tracks where we are in the byte stream -- each decoded value advances it by the number of bytes consumed.

**`Vec::with_capacity(count)`** -- Creates a vector with space pre-allocated for `count` elements. This avoids repeated memory allocations as we push values.

**`&bytes[offset..]`** -- A slice starting at position `offset` and going to the end. We pass a different slice to `decode_manual` each time so it always starts reading from the right position.

### Step 6: Test the manual format

```rust,ignore
#[cfg(test)]
mod manual_tests {
    use super::*;

    #[test]
    fn manual_round_trip_all_types() {
        let values = vec![
            Value::Null,
            Value::Boolean(true),
            Value::Boolean(false),
            Value::Integer(42),
            Value::Integer(-1),
            Value::Integer(i64::MAX),
            Value::Float(3.14),
            Value::String("hello".to_string()),
            Value::String("".to_string()),
        ];

        for original in &values {
            let bytes = original.encode_manual();
            let (decoded, consumed) = Value::decode_manual(&bytes).unwrap();
            assert_eq!(original, &decoded);
            assert_eq!(consumed, bytes.len());
        }
    }

    #[test]
    fn manual_row_round_trip() {
        let row = Row::new(vec![
            Value::Integer(1),
            Value::String("Alice".to_string()),
            Value::Boolean(true),
            Value::Null,
        ]);

        let bytes = row.encode_manual();
        let decoded = Row::decode_manual(&bytes).unwrap();
        assert_eq!(row, decoded);
    }

    #[test]
    fn compare_sizes() {
        let row = Row::new(vec![
            Value::Integer(1),
            Value::String("Alice".to_string()),
            Value::Boolean(true),
        ]);

        let serde_bytes = row.to_bytes().unwrap();
        let manual_bytes = row.encode_manual();

        println!("Serde/bincode: {} bytes", serde_bytes.len());
        println!("Manual format: {} bytes", manual_bytes.len());

        // Both should round-trip correctly
        assert_eq!(row, Row::from_bytes(&serde_bytes).unwrap());
        assert_eq!(row, Row::decode_manual(&manual_bytes).unwrap());
    }

    #[test]
    fn manual_rejects_invalid_tag() {
        let result = Value::decode_manual(&[0xFF]);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown type tag"));
    }

    #[test]
    fn manual_rejects_truncated_integer() {
        let result = Value::decode_manual(&[0x02, 0x01, 0x02]); // only 2 data bytes
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not enough bytes"));
    }
}
```

```
$ cargo test manual_tests -- --nocapture
running 5 tests
test value::manual_tests::manual_round_trip_all_types ... ok
test value::manual_tests::manual_row_round_trip ... ok
test value::manual_tests::compare_sizes ... ok
Serde/bincode: 30 bytes
Manual format: 23 bytes
test value::manual_tests::manual_rejects_invalid_tag ... ok
test value::manual_tests::manual_rejects_truncated_integer ... ok

test result: ok. 5 passed; 0 failed; 0 ignored
```

> **What just happened?**
>
> Our manual format is smaller (23 bytes vs 30 bytes) because it uses 1-byte type tags instead of bincode's 4-byte enum discriminants. But the serde version took 2 lines of code to implement. The manual version took 80+ lines. That is the tradeoff:
>
> - **Serde:** Fast to write, easy to maintain, slightly larger output
> - **Manual:** More code, more bugs, smaller output, total control
>
> For most use cases, serde wins. You only need manual encoding when you need byte-level control (wire protocols, embedded systems, extreme performance).

### Common mistakes with manual encoding

**Mistake: Forgetting to check byte lengths before reading**

```rust,ignore
// WRONG -- crashes if bytes is too short
let arr: [u8; 8] = bytes[1..9].try_into().unwrap();

// RIGHT -- check first, then read
if bytes.len() < 9 {
    return Err("not enough bytes".to_string());
}
let arr: [u8; 8] = bytes[1..9].try_into().unwrap();
```

**Mistake: Using big-endian on one side and little-endian on the other**

```rust,ignore
// WRONG -- encode with LE, decode with BE
let bytes = n.to_le_bytes();
let back = i64::from_be_bytes(bytes);  // different endianness!

// RIGHT -- use the same endianness for both
let bytes = n.to_le_bytes();
let back = i64::from_le_bytes(bytes);  // same
```

**Mistake: Not tracking bytes consumed in the decoder**

If your decoder does not return how many bytes it consumed, you cannot decode multiple values from a stream. Always return `(value, bytes_consumed)`.

---

## Rust Gym

### Drill 1: Derive Debug, Clone, PartialEq

Given this struct, add the necessary derives so the code compiles:

```rust,ignore
struct Config {
    host: String,
    port: u16,
    max_connections: usize,
}

fn main() {
    let config = Config {
        host: "localhost".to_string(),
        port: 5432,
        max_connections: 100,
    };

    let backup = config.clone();          // needs Clone
    println!("{:?}", config);             // needs Debug
    assert_eq!(config, backup);           // needs PartialEq
}
```

<details>
<summary>Hint: What derives do you need?</summary>

You need three derives -- one for each operation used in `main()`. Look at the comments next to each line.

</details>

<details>
<summary>Solution</summary>

```rust
#[derive(Debug, Clone, PartialEq)]
struct Config {
    host: String,
    port: u16,
    max_connections: usize,
}

fn main() {
    let config = Config {
        host: "localhost".to_string(),
        port: 5432,
        max_connections: 100,
    };

    let backup = config.clone();
    println!("{:?}", config);
    assert_eq!(config, backup);
}
```

All three derives work because every field type (`String`, `u16`, `usize`) already implements `Debug`, `Clone`, and `PartialEq`. If any field did not implement a trait, the derive would fail with a compiler error pointing at the offending field.

</details>

### Drill 2: Serialize a Struct to JSON and Back

Add `serde_json = "1"` to your dependencies. Add the right derives to make this code compile and run:

```rust,ignore
use serde::{Serialize, Deserialize};

struct Address {
    street: String,
    city: String,
    zip: String,
}

struct Person {
    name: String,
    age: u32,
    address: Address,
}

fn main() {
    let person = Person {
        name: "Alice".to_string(),
        age: 30,
        address: Address {
            street: "123 Main St".to_string(),
            city: "Springfield".to_string(),
            zip: "62701".to_string(),
        },
    };

    let json = serde_json::to_string_pretty(&person).unwrap();
    println!("{}", json);

    let decoded: Person = serde_json::from_str(&json).unwrap();
    assert_eq!(person.name, decoded.name);
    assert_eq!(person.address.city, decoded.address.city);
}
```

<details>
<summary>Hint: Both structs need derives</summary>

Serde needs to serialize `Address` as part of `Person`. That means both `Address` and `Person` need `#[derive(Serialize, Deserialize)]`. If you only derive on `Person`, the compile will fail because `address` is an `Address` field that serde does not know how to serialize.

</details>

<details>
<summary>Solution</summary>

```rust,ignore
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
struct Address {
    street: String,
    city: String,
    zip: String,
}

#[derive(Serialize, Deserialize)]
struct Person {
    name: String,
    age: u32,
    address: Address,
}

fn main() {
    let person = Person {
        name: "Alice".to_string(),
        age: 30,
        address: Address {
            street: "123 Main St".to_string(),
            city: "Springfield".to_string(),
            zip: "62701".to_string(),
        },
    };

    let json = serde_json::to_string_pretty(&person).unwrap();
    println!("{}", json);

    let decoded: Person = serde_json::from_str(&json).unwrap();
    assert_eq!(person.name, decoded.name);
    assert_eq!(person.address.city, decoded.address.city);
}
```

Output:

```json
{
  "name": "Alice",
  "age": 30,
  "address": {
    "street": "123 Main St",
    "city": "Springfield",
    "zip": "62701"
  }
}
```

Serde handles nested structs automatically -- it recursively serializes each field. If `Address` did not derive `Serialize`, the `Person` derive would fail because the macro would not know how to serialize the `address` field.

</details>

### Drill 3: Manual `to_bytes` / `from_bytes`

Without using serde, implement `to_bytes()` and `from_bytes()` for this struct:

```rust,ignore
struct Measurement {
    sensor_id: u16,
    timestamp: u64,
    value: f32,
}
```

The format should be: `[sensor_id: 2 bytes LE][timestamp: 8 bytes LE][value: 4 bytes LE]` -- exactly 14 bytes total.

<details>
<summary>Hint: Use `to_le_bytes()` for each field</summary>

Each numeric type has `to_le_bytes()` that returns a fixed-size array: `u16` returns `[u8; 2]`, `u64` returns `[u8; 8]`, `f32` returns `[u8; 4]`. Use `copy_from_slice` to write each into the right position in a `[u8; 14]` output array.

</details>

<details>
<summary>Solution</summary>

```rust
struct Measurement {
    sensor_id: u16,
    timestamp: u64,
    value: f32,
}

impl Measurement {
    fn to_bytes(&self) -> [u8; 14] {
        let mut buf = [0u8; 14];
        buf[0..2].copy_from_slice(&self.sensor_id.to_le_bytes());
        buf[2..10].copy_from_slice(&self.timestamp.to_le_bytes());
        buf[10..14].copy_from_slice(&self.value.to_le_bytes());
        buf
    }

    fn from_bytes(bytes: &[u8; 14]) -> Self {
        Measurement {
            sensor_id: u16::from_le_bytes([bytes[0], bytes[1]]),
            timestamp: u64::from_le_bytes(bytes[2..10].try_into().unwrap()),
            value: f32::from_le_bytes(bytes[10..14].try_into().unwrap()),
        }
    }
}

#[test]
fn measurement_round_trip() {
    let m = Measurement {
        sensor_id: 42,
        timestamp: 1700000000,
        value: 23.5,
    };
    let bytes = m.to_bytes();
    assert_eq!(bytes.len(), 14);
    let decoded = Measurement::from_bytes(&bytes);
    assert_eq!(m.sensor_id, decoded.sensor_id);
    assert_eq!(m.timestamp, decoded.timestamp);
    assert_eq!(m.value, decoded.value);
}
```

Using `[u8; 14]` instead of `Vec<u8>` as the return type communicates the fixed size at the type level. The caller knows exactly how many bytes to expect.

</details>

---

## What You Built

In this chapter, you:

1. **Added serde and bincode** -- the foundation of Rust's serialization ecosystem, with derive macros that generate encoding/decoding code at compile time
2. **Built a typed Value enum** -- five variants covering every SQL data type your database needs, with safe extraction methods
3. **Wrote round-trip tests** -- proving that every `Value` variant survives the encode-decode cycle, including edge cases like empty strings, extreme numbers, and corrupted input
4. **Created a Row and Table abstraction** -- moving from raw key-value pairs to structured data with named columns and typed values
5. **Implemented a custom binary format** -- understanding what serde does under the hood, and learning when manual encoding is the right choice

Your database now understands its data. It knows the difference between the integer `42` and the string `"42"`. It can store structured rows with multiple columns. And it can serialize everything to compact binary for storage or transmission.

But the database still has a critical limitation: if two users read and write at the same time, they will see inconsistent data. One user's write might appear halfway through another user's read. Chapter 5 introduces MVCC -- Multi-Version Concurrency Control -- the mechanism that gives every reader a consistent snapshot, even while writers are modifying data.

---

## Exercises

**Exercise 4.1: Add a `Bytes` variant to Value**

Add a `Bytes(Vec<u8>)` variant to the `Value` enum for storing raw binary data (like images or files). Implement both serde and manual encoding/decoding for it.

<details>
<summary>Hint</summary>

Add the variant to the enum, add a new type tag (e.g., `0x05`) in `encode_manual`, and add a new arm in `decode_manual`. Serde handles the new variant automatically -- just add it to the enum and re-derive.

</details>

**Exercise 4.2: Size comparison report**

Write a test that creates a row with 10 different values and compares the size of serde/bincode encoding vs manual encoding. Print a table showing the size of each individual value in both formats.

<details>
<summary>Hint</summary>

```rust,ignore
for val in &values {
    let serde_size = val.to_bytes().unwrap().len();
    let manual_size = val.encode_manual().len();
    println!("{:>20} | serde: {:>3} | manual: {:>3}", val, serde_size, manual_size);
}
```

</details>

**Exercise 4.3: Add a `DataType` enum**

Create a `DataType` enum with variants `Boolean`, `Integer`, `Float`, `String`. Add a method `Value::data_type(&self) -> Option<DataType>` that returns `None` for `Null` and the appropriate `DataType` for everything else. This will be useful when we add column type checking in later chapters.

<details>
<summary>Hint</summary>

```rust,ignore
#[derive(Debug, Clone, PartialEq)]
pub enum DataType {
    Boolean,
    Integer,
    Float,
    String,
}

impl Value {
    pub fn data_type(&self) -> Option<DataType> {
        match self {
            Value::Null => None,
            Value::Boolean(_) => Some(DataType::Boolean),
            // ...
        }
    }
}
```

</details>

---

## Key Takeaways

- **Serialization** converts structured data to bytes. Deserialization converts bytes back to structured data.
- **Serde** is Rust's serialization framework. It separates data model (your structs) from format (JSON, bincode, etc.).
- **Derive macros** auto-generate trait implementations at compile time. `#[derive(Serialize, Deserialize)]` replaces hundreds of lines of manual encoding code.
- **Bincode** is a compact binary format ideal for database storage. JSON is better for human-readable output.
- **Round-trip tests** are the most important serialization tests: encode, decode, verify equality.
- **Every derive requires all fields to support the trait.** If one field does not implement `Clone`, you cannot derive `Clone` on the struct.
- **Manual encoding** gives you byte-level control at the cost of more code and more bugs. Use it when you need wire protocol stability or minimal dependencies.

---

### Reference implementation

The files you built in this chapter correspond to these files in the reference codebase:

| Your file | Reference |
|-----------|-----------|
| `src/value.rs` | [`src/sql/types.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/sql/types.rs) -- `Value` enum with serialization |
| `src/table.rs` | [`src/sql/engine/local.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/sql/engine/local.rs) -- table operations |
| Manual encoding | [`src/encoding.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/encoding.rs) -- custom key encoding for ordered storage |
| Round-trip tests | Tests within each module |
