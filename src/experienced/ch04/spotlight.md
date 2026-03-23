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
