# Chapter 4: Serialization

In Chapter 3, you built a persistent storage engine that writes bytes to disk and reads them back. But everything is `Vec<u8>` — raw, untyped bytes. Your database can store `"42"` as a string, but it has no idea whether that is a number, a name, or a boolean. A real database needs to understand its data. It needs to encode integers differently from strings, pack structured rows into bytes for storage, and unpack them on read without losing information.

This chapter builds a proper serialization layer. You will add serde and bincode to your project, derive `Serialize` and `Deserialize` on your types, build round-trip tests, and then implement a custom binary format by hand — because understanding what serde does under the hood makes you a better engineer, the same way understanding what the compiler does makes you a better programmer.

By the end of this chapter, you will have:

- A `Value` enum with typed variants (Null, Boolean, Integer, Float, String) that serializes to compact binary
- Round-trip tests proving that encode-then-decode preserves every type exactly
- A `Row` type that packs multiple values into a single byte sequence for storage
- A hand-rolled length-prefixed binary format to compare against serde's magic
- A deep understanding of derive macros, trait derivation, and how serde works

---

## Spotlight: Serde & Derive Macros

Every chapter has one spotlight concept. This chapter's spotlight is **serde and derive macros** — Rust's approach to code generation at compile time and the serialization framework that practically every Rust project depends on.

### The problem with manual serialization

If you wanted to turn a struct into bytes without any libraries, you would write something like this:

```rust
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

This works. For one struct. Now imagine doing this for 50 types, some with nested structs, some with `Option` fields, some with `Vec`s. The boilerplate multiplies, bugs hide in byte offsets, and every struct change requires updating two functions. This is not sustainable.

### What serde does

Serde is Rust's serialization/deserialization framework. The name is a portmanteau: **ser**ialize + **de**serialize. It separates two concerns:

1. **Data model** — your struct knows its fields and types
2. **Format** — JSON, binary, TOML, YAML, MessagePack, etc.

Serde acts as the bridge. Your struct describes itself to serde (via derived traits), and a format crate (like `serde_json` or `bincode`) handles the actual byte layout. Change the format and your struct code does not change. Change the struct and the format code does not change.

```rust
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
struct Point {
    x: f64,
    y: f64,
}
```

That `#[derive(Serialize, Deserialize)]` replaces all the manual `to_bytes`/`from_bytes` code. The derive macro generates the implementation at compile time — no runtime reflection, no dynamic dispatch, no overhead.

### How derive macros work

A derive macro is a procedural macro that reads your struct definition at compile time and generates code. When you write `#[derive(Serialize)]`, the compiler:

1. Passes the token stream of your struct definition to the `Serialize` derive macro
2. The macro inspects every field name and type
3. It generates an `impl Serialize for Point { ... }` block
4. The generated code is compiled along with the rest of your program

You never see the generated code (unless you use `cargo expand`), but it is there. It is regular Rust — no magic at runtime.

### Built-in derive macros

Rust has several built-in derive macros that you will use constantly:

```rust
#[derive(Debug)]           // enables {:?} formatting
#[derive(Clone)]           // enables .clone()
#[derive(PartialEq)]       // enables == and !=
#[derive(Eq)]              // marker: equality is reflexive, symmetric, transitive
#[derive(Hash)]            // enables use as HashMap key
#[derive(Default)]         // enables Type::default()
```

You can stack them:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Point {
    x: f64,
    y: f64,
}
```

Each derive generates its own `impl` block. They are independent — `Debug` does not need `Clone`, `Serialize` does not need `PartialEq`. Derive what you need, nothing more.

### bincode: the compact binary format

For a database, JSON is wasteful. The string `"42"` takes 4 bytes in JSON (the characters `"`, `4`, `2`, `"`), but only 8 bytes as an `i64` in binary — and that 8 bytes holds a number up to 9.2 quintillion. More importantly, binary encoding has fixed-size fields, which means you can seek to exact offsets without parsing.

Bincode is a binary format crate for serde. It is fast, compact, and deterministic — the same value always produces the same bytes.

```rust
use bincode;

let point = Point { x: 1.0, y: 2.0 };
let bytes: Vec<u8> = bincode::serialize(&point).unwrap();
let decoded: Point = bincode::deserialize(&bytes).unwrap();
assert_eq!(point, decoded);
```

> **Coming from JS/Python/Go?**
>
> | Concept | JavaScript | Python | Go | Rust (serde) |
> |---------|-----------|--------|----|-------------|
> | Serialize to JSON | `JSON.stringify(obj)` | `json.dumps(obj)` | `json.Marshal(obj)` | `serde_json::to_string(&obj)` |
> | Deserialize from JSON | `JSON.parse(str)` | `json.loads(str)` | `json.Unmarshal(bytes, &obj)` | `serde_json::from_str(str)` |
> | Struct annotation | none (dynamic) | none (dynamic) | `json:"field_name"` struct tags | `#[derive(Serialize, Deserialize)]` |
> | Binary format | `Buffer` / protobuf | `struct.pack` / pickle | `encoding/binary` / protobuf | `bincode::serialize` / postcard |
> | Code generation | none (reflection) | none (reflection) | `go generate` | derive macros (compile-time) |
> | Zero-cost? | No (runtime reflection) | No (runtime reflection) | Partial (reflect package) | Yes (all generated at compile time) |
>
> The key difference: in JS/Python, serialization discovers struct fields at runtime via reflection. In Go, it uses struct tags parsed at runtime. In Rust, the derive macro generates all the serialization code at compile time. There is zero runtime cost for "figuring out" what fields exist — that work was done before the program ran.

---

## Exercise 1: Add Serde and Define a Typed Value

**Goal:** Replace raw `Vec<u8>` with a typed `Value` enum that knows whether it holds a number, a string, a boolean, or nothing. Add serde + bincode dependencies and derive `Serialize`/`Deserialize`.

### Step 1: Add dependencies

Open `Cargo.toml` and add serde and bincode:

```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
bincode = "1"
```

The `features = ["derive"]` flag enables the `#[derive(Serialize, Deserialize)]` macros. Without it, you would have to implement the traits manually.

### Step 2: Define the Value enum

Create `src/value.rs`:

```rust
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

Notice the five derives: `Debug` for printing, `Clone` because values get copied around, `PartialEq` for assertions, and the two serde traits.

### Step 3: Add Display for human-readable output

```rust
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

### Step 4: Add convenience constructors and type checks

```rust
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

### Step 5: Register the module

In `src/lib.rs` (or `src/main.rs`):

```rust
pub mod value;
```

### Step 6: Verify it compiles

```
$ cargo build
   Compiling serde v1.0.xxx
   Compiling bincode v1.3.x
   Compiling toydb v0.1.0
    Finished dev [unoptimized + debuginfo] target(s) in 5.23s
```

<details>
<summary>Hint: If you get "cannot find derive macro Serialize"</summary>

Make sure your `serde` dependency includes `features = ["derive"]`. Without this feature flag, the derive macros are not available. Your `Cargo.toml` should have:

```toml
serde = { version = "1", features = ["derive"] }
```

Not just `serde = "1"`.

</details>

---

## Exercise 2: Encode, Decode, and Round-Trip Test

**Goal:** Serialize `Value` instances to bytes using bincode, deserialize them back, and prove the round trip is lossless with tests.

### Step 1: Add encode/decode functions

Add to `src/value.rs`:

```rust
use bincode;

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

That is the entire serialization layer. Two methods. Serde and bincode do all the heavy lifting — field discovery, byte layout, error handling. The `map_err` converts bincode's error type into a simple `String` (we will improve this in later chapters).

### Step 2: Write round-trip tests

```rust
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
            "Hello, World! 🌍".to_string(),
            "a".repeat(10_000),
        ];
        for val in test_cases {
            let original = Value::String(val);
            let bytes = original.to_bytes().unwrap();
            let decoded = Value::from_bytes(&bytes).unwrap();
            assert_eq!(original, decoded);
        }
    }

    #[test]
    fn different_types_produce_different_bytes() {
        let int_bytes = Value::Integer(42).to_bytes().unwrap();
        let str_bytes = Value::String("42".to_string()).to_bytes().unwrap();
        assert_ne!(int_bytes, str_bytes);
    }

    #[test]
    fn null_is_compact() {
        let bytes = Value::Null.to_bytes().unwrap();
        // Null should be very small — just the enum discriminant
        assert!(bytes.len() <= 4, "Null serialized to {} bytes", bytes.len());
    }

    #[test]
    fn corrupted_bytes_return_error() {
        let result = Value::from_bytes(&[0xFF, 0xFF, 0xFF]);
        assert!(result.is_err());
    }
}
```

### Step 3: Run the tests

```
$ cargo test value::tests
   Compiling toydb v0.1.0
    Finished test [unoptimized + debuginfo] target(s) in 2.13s
     Running unittests src/lib.rs

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

### Step 4: Inspect the byte layout

Add a helper test that prints the actual bytes. This is not for CI — it is for your understanding:

```rust
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

Notice the pattern: the first 4 bytes are the enum discriminant (0 for Null, 1 for Boolean, etc.), followed by the variant's data. Strings include an 8-byte length prefix before the UTF-8 bytes. This is bincode's default format — it is not magic, just a well-defined encoding scheme.

<details>
<summary>Hint: If your byte values look different</summary>

Bincode's exact encoding depends on the version and configuration. The byte counts and layout shown above are for bincode 1.x with the default configuration. If you use bincode 2.x, the encoding may differ. The important thing is that the round trip works — the exact bytes are an implementation detail.

</details>

---

## Exercise 3: Build a Row Type for Structured Storage

**Goal:** Define a `Row` as a vector of `Value`s, add serialization, and integrate it with the storage engine so the database can store and retrieve structured rows — not just raw key-value pairs.

### Step 1: Define Row

Add to `src/value.rs`:

```rust
/// A row is an ordered sequence of values — one per column.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Row {
    pub values: Vec<Value>,
}

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

impl fmt::Display for Row {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let parts: Vec<String> = self.values.iter().map(|v| v.to_string()).collect();
        write!(f, "({})", parts.join(", "))
    }
}
```

### Step 2: Build a simple table abstraction

Create `src/table.rs`:

```rust
use crate::value::{Row, Value};
use std::collections::BTreeMap;

/// A simple in-memory table with named columns and typed rows.
pub struct Table {
    pub name: String,
    pub columns: Vec<String>,
    rows: BTreeMap<i64, Row>,
    next_id: i64,
}

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

### Step 3: Test the table

```rust
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

### Step 4: Add a demo binary

Create `examples/table_demo.rs`:

```rust
use toydb::value::Value;
use toydb::table::Table;

fn main() {
    let mut users = Table::new("users", vec![
        "name".to_string(),
        "email".to_string(),
        "age".to_string(),
    ]);

    users.insert(vec![
        Value::String("Alice".to_string()),
        Value::String("alice@example.com".to_string()),
        Value::Integer(30),
    ]).unwrap();

    users.insert(vec![
        Value::String("Bob".to_string()),
        Value::String("bob@example.com".to_string()),
        Value::Integer(25),
    ]).unwrap();

    users.insert(vec![
        Value::String("Charlie".to_string()),
        Value::Null,
        Value::Integer(35),
    ]).unwrap();

    println!("=== Users Table ===");
    users.display();
}
```

```
$ cargo run --example table_demo
=== Users Table ===
  id |         name |        email |          age |
----------------------------------------------
   1 |      'Alice' | 'alice@example.com' |           30 |
   2 |        'Bob' | 'bob@example.com' |           25 |
   3 |    'Charlie' |         NULL |           35 |
```

<details>
<summary>Hint: If the column widths look wrong</summary>

The `display()` method uses fixed-width formatting (`{:>12}`). If your values are longer than 12 characters, they will push the columns out of alignment. For production use, you would calculate column widths based on the actual data. For now, the fixed width is fine for understanding.

</details>

---

## Exercise 4: Build a Custom Binary Format by Hand

**Goal:** Implement a length-prefixed binary encoding without serde. This exercise exists to show you what serde does for you — and to build intuition for binary formats that you will need when designing wire protocols in later chapters.

### Step 1: Define the format

The format is simple:

```
[type_tag: 1 byte] [payload: variable]

Type tags:
  0x00 = Null        (no payload)
  0x01 = Boolean     (1 byte: 0x00 or 0x01)
  0x02 = Integer     (8 bytes: little-endian i64)
  0x03 = Float       (8 bytes: little-endian f64)
  0x04 = String      (4 bytes length + N bytes UTF-8)
```

### Step 2: Implement manual encoding

Add to `src/value.rs`:

```rust
impl Value {
    /// Manually encode this value to a length-prefixed binary format.
    /// This exists for learning — in production, use to_bytes() (serde).
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

    /// Manually decode a value from the custom binary format.
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

The return type `Result<(Self, usize), String>` includes the number of bytes consumed. This is essential when decoding multiple values from a stream — you need to know where one value ends and the next begins.

### Step 3: Encode/decode a row manually

```rust
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

### Step 4: Compare the two approaches

```rust
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
        println!("Serde bytes: {:?}", serde_bytes);
        println!("Manual bytes: {:?}", manual_bytes);

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

The manual format is smaller because it uses 1-byte type tags instead of bincode's 4-byte enum discriminants. But the serde version took 2 lines of code to implement. The manual version took 80+. That is the tradeoff — and for a database where you control both ends, either works. Serde wins on development speed; hand-rolled formats win on byte efficiency and protocol stability.

<details>
<summary>Hint: When to choose manual encoding over serde</summary>

Use serde for internal storage (on-disk format, in-process communication). Use manual encoding when:

1. **Wire protocol stability** — you need the format to be identical across language implementations (a Go client must produce the same bytes as a Rust server)
2. **Byte-level control** — you need to pack data into exact offsets for memory-mapped files
3. **Minimal dependencies** — embedded systems or WASM targets where pulling in serde is too heavy
4. **Performance-critical paths** — serde is fast, but hand-rolled code that skips the trait dispatch can be faster for hot paths

For toydb, we will use serde for storage and a custom format for the wire protocol in Chapter 12.

</details>

---

## Rust Gym

### Drill 1: Derive Debug, Clone, PartialEq

Given this struct, add the necessary derives so the code compiles:

```rust
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

All three derives work here because every field type (`String`, `u16`, `usize`) already implements `Debug`, `Clone`, and `PartialEq`. If any field did not implement a trait, the derive would fail with a clear compiler error pointing at the offending field.

</details>

### Drill 2: Serialize/Deserialize a Nested Struct to JSON

Add `serde_json = "1"` to your dependencies and make this code compile and pass:

```rust
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
<summary>Solution</summary>

```rust
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

Both `Address` and `Person` need the derives. Serde handles nested structs automatically — it recursively serializes each field. If `Address` did not derive `Serialize`, the `Person` derive would fail because the macro would not know how to serialize the `address` field.

</details>

### Drill 3: Implement Manual `to_bytes()` / `from_bytes()`

Without using serde, implement `to_bytes()` and `from_bytes()` for this struct:

```rust
struct Measurement {
    sensor_id: u16,
    timestamp: u64,
    value: f32,
}
```

The format should be: `[sensor_id: 2 bytes LE][timestamp: 8 bytes LE][value: 4 bytes LE]` — exactly 14 bytes total.

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

Key insight: using `[u8; 14]` instead of `Vec<u8>` as the return type communicates the fixed size at the type level. The caller knows exactly how many bytes to expect. This is the "define errors out of existence" pattern — a `Vec<u8>` could be any length, but `[u8; 14]` is always 14 bytes.

</details>

---

## DSA in Context: Binary Encoding

Serialization is a data structures problem in disguise. Every encoding scheme makes tradeoffs between three competing goals:

### Fixed-length vs variable-length encoding

**Fixed-length:** Every integer is 8 bytes, every float is 8 bytes, regardless of the actual value. The number `1` takes the same space as `9,223,372,036,854,775,807`.

```
Integer(1)    -> [0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]  (8 bytes)
Integer(MAX)  -> [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x7F]  (8 bytes)
```

Advantage: you can seek to the Nth field by calculating `offset = N * field_size`. No parsing needed. This is why SQLite uses fixed-size records in its B-tree pages.

**Variable-length (varint):** Small numbers use fewer bytes. Protocol Buffers and SQLite's record format use this:

```
Integer(1)    -> [0x01]                    (1 byte)
Integer(300)  -> [0xAC, 0x02]             (2 bytes)
Integer(MAX)  -> [0xFF, 0xFF, ..., 0x01]  (10 bytes)
```

Advantage: most real-world numbers are small. Variable-length encoding saves 50-70% of space for typical data. Disadvantage: you must parse sequentially — you cannot jump to the 5th field without reading fields 1-4.

### Endianness

Should the byte `0x01 0x00` mean 1 (little-endian, least significant byte first) or 256 (big-endian, most significant byte first)?

- **Little-endian (LE):** x86, ARM (default), most modern CPUs. Rust's `to_le_bytes()`.
- **Big-endian (BE):** Network byte order, Java's default. Rust's `to_be_bytes()`.

Our format uses little-endian because that matches the CPU's native byte order, avoiding conversion overhead. Network protocols traditionally use big-endian (hence "network byte order"), but modern protocols like Protocol Buffers use variable-length encoding that sidesteps the question entirely.

### Schema evolution

What happens when you add a field to your struct? With bincode's default format, old data cannot be deserialized into the new struct — the byte layout changed. With formats like Protocol Buffers or MessagePack, fields are tagged with numbers, so new fields can be added without breaking old data.

```
// Version 1: { name: String, age: i64 }
// Bincode: [name_len][name_bytes][age_bytes]

// Version 2: { name: String, age: i64, email: String }
// Bincode: [name_len][name_bytes][age_bytes][email_len][email_bytes]
// Old data is missing email_len — deserialization fails!
```

For toydb, schema evolution is not a concern yet — we control both the writer and reader, and we can migrate data. But in Chapter 12, when we design the client-server wire protocol, we will need to think carefully about backwards compatibility.

---

## System Design Corner: Wire Protocols

In a system design interview, "how does the client talk to the database?" is a foundational question. The answer is: through a wire protocol — a binary format that defines how requests and responses are encoded on the network.

### PostgreSQL's wire protocol

PostgreSQL uses a message-based protocol. Every message starts with a type byte and a 4-byte length:

```
[type: 1 byte][length: 4 bytes BE][payload: N bytes]

Examples:
  'Q' + length + "SELECT * FROM users\0"    (simple query)
  'D' + length + column_count + col1 + ...   (data row)
  'Z' + length + status                      (ready for query)
```

The server and client take turns sending messages. The protocol is stateful — you must authenticate before querying, and you must send a Sync message after each query batch.

### MySQL's wire protocol

MySQL uses a similar but distinct format with sequence IDs for packet ordering:

```
[length: 3 bytes LE][sequence_id: 1 byte][payload: N bytes]
```

The 3-byte length limits individual packets to 16MB. Larger results are split across multiple packets.

### Design tradeoffs

| Aspect | Text protocol (HTTP/JSON) | Binary protocol (Postgres) |
|--------|--------------------------|---------------------------|
| Debugging | Easy — curl, browser | Hard — need wireshark or custom tools |
| Parsing speed | Slow — text parsing | Fast — fixed offsets |
| Bandwidth | High — verbose | Low — compact |
| Schema evolution | Easy — ignore unknown fields | Hard — must version carefully |
| Streaming | Difficult | Natural — message boundaries |
| Implementation | Any HTTP library | Custom parser needed |

Most new databases (CockroachDB, TiDB) implement the PostgreSQL wire protocol for compatibility — any PostgreSQL client library works out of the box. This is a powerful strategy: instead of building a new ecosystem, piggyback on an existing one.

> **Interview talking point:** *"Our database uses a binary wire protocol with type-tagged messages. Each message starts with a 1-byte type tag and a 4-byte big-endian length, followed by the payload. We chose binary over HTTP/JSON for two reasons: lower latency for high-throughput workloads, and natural support for streaming result sets. For development and debugging, we also support a text-based protocol on a separate port."*

---

## Design Insight: Information Hiding

In *A Philosophy of Software Design*, Ousterhout argues that the most important technique for managing complexity is **information hiding** — each module should encapsulate knowledge that is not needed by other modules.

Serde is a masterclass in information hiding. Consider what happens when you write:

```rust
#[derive(Serialize, Deserialize)]
struct Row {
    values: Vec<Value>,
}
```

The caller of `Row::to_bytes()` does not know:
- Whether the format is little-endian or big-endian
- Whether strings are length-prefixed or null-terminated
- Whether the enum discriminant is 1 byte or 4 bytes
- Whether Vec length is u32 or u64
- How nested structs are flattened

All of this is hidden behind the derive macro and the bincode crate. The caller says "serialize this" and gets bytes. The implementation can change (swap bincode for postcard, switch to big-endian) without touching any caller code.

Compare this to the manual encoding from Exercise 4. Every caller must know the format — type tag values, byte offsets, endianness. Change any detail and every call site breaks. The manual format leaks information; the serde format hides it.

This does not mean manual encoding is wrong. Sometimes you need the control. But Ousterhout's insight is that information leakage is the primary source of complexity in software, and you should default to hiding unless you have a specific reason to expose.

In toydb, our `Value::to_bytes()` hides the encoding format. The storage engine calls `to_bytes()` and `from_bytes()` without knowing whether the underlying format is bincode, postcard, or hand-rolled. If we later switch formats for better performance, the storage engine does not change.

---

## What You Built

In this chapter, you:

1. **Added serde and bincode** — the foundation of Rust's serialization ecosystem, with derive macros that generate encoding/decoding code at compile time
2. **Built round-trip tests** — proving that every `Value` variant survives the encode-decode cycle, including edge cases like empty strings, extreme numbers, and corrupted input
3. **Created a Row and Table abstraction** — moving from raw key-value pairs to structured data with named columns and typed values
4. **Implemented a custom binary format** — understanding what serde does under the hood, and when manual encoding is the right choice

Your database now understands its data. It knows the difference between the integer `42` and the string `"42"`. It can store structured rows with multiple columns. And it can serialize everything to compact binary for storage or transmission.

But the database still has a critical limitation: if two users read and write at the same time, they will see inconsistent data. One user's write might appear halfway through another user's read. Chapter 5 introduces MVCC — Multi-Version Concurrency Control — the mechanism that gives every reader a consistent snapshot, even while writers are modifying data.

---

### DS Deep Dive

Serde's derive macros look like magic, but they are built on Rust's procedural macro system — code that writes code. This deep dive explores how proc macros work, the Visitor pattern that serde uses internally, and how to build your own derive macro from scratch.

**-> [Serde Internals & Procedural Macros -- "The Code That Writes Code"](../ds-narratives/ch04-serde-internals.md)**

---

### Reference implementation

The files you built in this chapter correspond to these files in the reference codebase:

| Your file | Reference |
|-----------|-----------|
| `src/value.rs` | [`src/sql/types.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/sql/types.rs) — `Value` enum with serialization |
| `src/table.rs` | [`src/sql/engine/local.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/sql/engine/local.rs) — table operations |
| Manual encoding | [`src/encoding.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/encoding.rs) — custom key encoding for ordered storage |
| Round-trip tests | Tests within each module |
