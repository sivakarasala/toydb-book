# Chapter 1: What Is a Database?

You are about to build a database from scratch. Not a wrapper around SQLite. Not a tutorial that hand-waves over the hard parts. A real, working database — storage engine, SQL parser, query executor, client-server protocol, and distributed consensus — all in Rust. This first chapter starts where every database starts: a place to put data and a way to get it back.

By the end of this chapter, you will have:

- A working key-value store backed by Rust's `HashMap`
- A REPL (read-eval-print loop) that accepts SET, GET, DELETE, LIST, and STATS commands
- Support for multiple value types using Rust's `enum`
- A clear mental model of Rust's type system, variable bindings, and ownership semantics around collections

---

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

## Exercise 1: Your First Database (the HashMap)

**Goal:** Create a new Rust project and build a `Database` struct that wraps a `HashMap`, with methods to set, get, delete, and list keys.

### Step 1: Create the project

```bash
cargo new toydb
cd toydb
```

This creates a new Rust project with `src/main.rs` and `Cargo.toml`. No external dependencies needed for this chapter — everything comes from the standard library.

### Step 2: Define the Database struct

Open `src/main.rs` and replace its contents with:

```rust
use std::collections::HashMap;

struct Database {
    data: HashMap<String, String>,
}

impl Database {
    fn new() -> Self {
        Database {
            data: HashMap::new(),
        }
    }

    fn set(&mut self, key: String, value: String) {
        self.data.insert(key, value);
    }

    fn get(&self, key: &str) -> Option<&String> {
        self.data.get(key)
    }

    fn delete(&mut self, key: &str) -> bool {
        self.data.remove(key).is_some()
    }

    fn list(&self) -> Vec<(&String, &String)> {
        self.data.iter().collect()
    }
}

fn main() {
    let mut db = Database::new();

    db.set("name".to_string(), "toydb".to_string());
    db.set("version".to_string(), "0.1.0".to_string());
    db.set("language".to_string(), "Rust".to_string());

    println!("GET name = {:?}", db.get("name"));
    println!("GET missing = {:?}", db.get("missing"));

    println!("\nAll keys:");
    for (key, value) in db.list() {
        println!("  {} = {}", key, value);
    }

    let deleted = db.delete("version");
    println!("\nDELETE version: {}", deleted);
    println!("GET version = {:?}", db.get("version"));
}
```

### Step 3: Run it

```bash
cargo run
```

Expected output (key order may vary — HashMap does not guarantee order):

```
GET name = Some("toydb")
GET missing = None

All keys:
  language = Rust
  name = toydb
  version = 0.1.0

DELETE version: true
GET version = None
```

### Step 4: Understand what you wrote

**The struct** wraps a `HashMap<String, String>`. Wrapping it in a struct (rather than using a bare HashMap) gives us a place to add behavior, enforce invariants, and evolve the storage engine later — without changing the API.

**`&mut self` vs `&self`:** Methods that modify the database (`set`, `delete`) take `&mut self` — a mutable reference to the struct. Methods that only read (`get`, `list`) take `&self` — an immutable reference. The compiler enforces this: you cannot call `set` while someone else is reading via `get`. This is the borrow checker at work, and it prevents data races at compile time.

**`fn get(&self, key: &str) -> Option<&String>`:** The key parameter is `&str` rather than `String`. This is a deliberate API choice — callers can pass a `&str` without allocating a new `String`. The HashMap's `.get()` method accepts anything that can be borrowed as the key type, and `String` implements `Borrow<str>`, so passing `&str` to look up `String` keys works.

<details>
<summary>Hint: If you see "cannot borrow as mutable"</summary>

Make sure your `db` variable is declared with `let mut db`. Without `mut`, the compiler will not let you call methods that take `&mut self`.

</details>

---

## Exercise 2: A Command-Line Interface

**Goal:** Turn your database into an interactive REPL that reads commands from stdin.

### Step 1: Add the REPL loop

Replace the `main` function (keep the `Database` struct and `impl` block) with:

```rust
use std::io::{self, Write};

fn main() {
    let mut db = Database::new();

    println!("toydb v0.1.0");
    println!("Type HELP for available commands.\n");

    loop {
        print!("toydb> ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(0) => break, // EOF (Ctrl+D)
            Ok(_) => {}
            Err(e) => {
                eprintln!("Error reading input: {}", e);
                continue;
            }
        }

        let input = input.trim();
        if input.is_empty() {
            continue;
        }

        let parts: Vec<&str> = input.splitn(3, ' ').collect();
        let command = parts[0].to_uppercase();

        match command.as_str() {
            "SET" => {
                if parts.len() < 3 {
                    println!("Usage: SET <key> <value>");
                    continue;
                }
                let key = parts[1].to_string();
                let value = parts[2].to_string();
                db.set(key, value);
                println!("OK");
            }
            "GET" => {
                if parts.len() < 2 {
                    println!("Usage: GET <key>");
                    continue;
                }
                match db.get(parts[1]) {
                    Some(value) => println!("{}", value),
                    None => println!("(nil)"),
                }
            }
            "DELETE" => {
                if parts.len() < 2 {
                    println!("Usage: DELETE <key>");
                    continue;
                }
                if db.delete(parts[1]) {
                    println!("OK");
                } else {
                    println!("(nil)");
                }
            }
            "LIST" => {
                let entries = db.list();
                if entries.is_empty() {
                    println!("(empty)");
                } else {
                    for (key, value) in entries {
                        println!("{} = {}", key, value);
                    }
                }
            }
            "HELP" => {
                println!("Commands:");
                println!("  SET <key> <value>  — store a key-value pair");
                println!("  GET <key>          — retrieve a value by key");
                println!("  DELETE <key>       — remove a key-value pair");
                println!("  LIST               — show all key-value pairs");
                println!("  HELP               — show this message");
                println!("  EXIT               — quit");
            }
            "EXIT" | "QUIT" => {
                println!("Bye!");
                break;
            }
            _ => {
                println!("Unknown command: '{}'. Type HELP for available commands.", parts[0]);
            }
        }
    }
}
```

### Step 2: Run the REPL

```bash
cargo run
```

Now interact with your database:

```
toydb v0.1.0
Type HELP for available commands.

toydb> SET user:1 Alice
OK
toydb> SET user:2 Bob
OK
toydb> GET user:1
Alice
toydb> LIST
user:1 = Alice
user:2 = Bob
toydb> DELETE user:1
OK
toydb> GET user:1
(nil)
toydb> EXIT
Bye!
```

### Step 3: Understand the parsing

**`splitn(3, ' ')`** splits the input into at most 3 parts. This is important — a value like `"Hello World"` should not be split further. `splitn(3, ' ')` on `"SET greeting Hello World"` produces `["SET", "greeting", "Hello World"]`.

**`to_uppercase()`** makes commands case-insensitive. The user can type `set`, `SET`, or `Set` — they all work.

**`match command.as_str()`** is Rust's pattern matching. Unlike a chain of `if/else if`, `match` is exhaustive — the `_` arm catches anything that does not match. The compiler will warn you if you forget a case in an enum match (we will see this in Exercise 3).

<details>
<summary>Hint: If the prompt does not appear before input</summary>

`print!` (without the `ln`) does not flush stdout by default. That is why we call `io::stdout().flush().unwrap()` after the prompt. Without the flush, the prompt might not appear until after you type and press Enter.

</details>

---

## Exercise 3: Better Value Types

**Goal:** Extend the database to support multiple value types — strings, integers, floats, and booleans — using Rust's `enum`.

Right now, every value is a `String`. Real databases support typed values. Rust's `enum` is the perfect tool for this — it is not the C-style "named integer" enum you might know. Rust enums are *algebraic data types*: each variant can hold different data.

### Step 1: Define the Value enum

Add this above the `Database` struct:

```rust
use std::fmt;

#[derive(Debug, Clone)]
enum Value {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::String(s) => write!(f, "{}", s),
            Value::Integer(n) => write!(f, "{}", n),
            Value::Float(n) => write!(f, "{}", n),
            Value::Boolean(b) => write!(f, "{}", b),
        }
    }
}

impl Value {
    fn type_name(&self) -> &str {
        match self {
            Value::String(_) => "string",
            Value::Integer(_) => "integer",
            Value::Float(_) => "float",
            Value::Boolean(_) => "boolean",
        }
    }

    fn parse(input: &str) -> Value {
        // Try boolean first
        if input.eq_ignore_ascii_case("true") {
            return Value::Boolean(true);
        }
        if input.eq_ignore_ascii_case("false") {
            return Value::Boolean(false);
        }

        // Try integer
        if let Ok(n) = input.parse::<i64>() {
            return Value::Integer(n);
        }

        // Try float
        if let Ok(n) = input.parse::<f64>() {
            return Value::Float(n);
        }

        // Default to string
        Value::String(input.to_string())
    }
}
```

### Step 2: Update the Database struct

Change the `Database` to use `Value` instead of `String`:

```rust
struct Database {
    data: HashMap<String, Value>,
}

impl Database {
    fn new() -> Self {
        Database {
            data: HashMap::new(),
        }
    }

    fn set(&mut self, key: String, value: Value) {
        self.data.insert(key, value);
    }

    fn get(&self, key: &str) -> Option<&Value> {
        self.data.get(key)
    }

    fn delete(&mut self, key: &str) -> bool {
        self.data.remove(key).is_some()
    }

    fn list(&self) -> Vec<(&String, &Value)> {
        self.data.iter().collect()
    }
}
```

### Step 3: Update the REPL

Update the SET and GET arms in your `match` block:

```rust
            "SET" => {
                if parts.len() < 3 {
                    println!("Usage: SET <key> <value>");
                    continue;
                }
                let key = parts[1].to_string();
                let value = Value::parse(parts[2]);
                db.set(key, value);
                println!("OK");
            }
            "GET" => {
                if parts.len() < 2 {
                    println!("Usage: GET <key>");
                    continue;
                }
                match db.get(parts[1]) {
                    Some(value) => println!("({}) {}", value.type_name(), value),
                    None => println!("(nil)"),
                }
            }
```

Also update the LIST arm to show types:

```rust
            "LIST" => {
                let entries = db.list();
                if entries.is_empty() {
                    println!("(empty)");
                } else {
                    for (key, value) in entries {
                        println!("{} = ({}) {}", key, value.type_name(), value);
                    }
                }
            }
```

### Step 4: Test it

```bash
cargo run
```

```
toydb> SET name Alice
OK
toydb> SET age 30
OK
toydb> SET score 9.5
OK
toydb> SET active true
OK
toydb> GET name
(string) Alice
toydb> GET age
(integer) 30
toydb> GET score
(float) 9.5
toydb> GET active
(boolean) true
toydb> LIST
name = (string) Alice
age = (integer) 30
score = (float) 9.5
active = (boolean) true
```

### Step 5: Understand enums and pattern matching

**`#[derive(Debug, Clone)]`** automatically implements the `Debug` trait (for `{:?}` formatting) and `Clone` (for deep-copying values). Derive macros generate boilerplate implementations based on the struct's fields.

**`impl fmt::Display for Value`** implements the `Display` trait, which controls how the value appears when you use `{}` in `println!`. The `match` inside handles each variant differently — this is exhaustive pattern matching. If you add a new variant to the enum and forget to handle it here, the compiler will refuse to build.

**`Value::parse`** tries each type in order: boolean, integer, float, then falls back to string. The `if let Ok(n) = input.parse::<i64>()` syntax combines parsing and pattern matching in one line. The `::<i64>` is a *turbofish* — it tells `parse()` what type to try parsing into.

> **Coming from JS/Python/Go?**
>
> In JavaScript, you would represent mixed types with `typeof` checks at runtime. In Python, values are dynamically typed — you never declare what a variable holds. In Go, you might use `interface{}` (or `any`) and type-assert at runtime.
>
> Rust's enum approach is fundamentally different: the type system knows *at compile time* exactly which variants are possible. Every `match` must handle all variants. There is no runtime type confusion, no `undefined is not a function`, no missing type assertion panic.

<details>
<summary>Hint: If you see "expected String, found Value"</summary>

After changing `HashMap<String, String>` to `HashMap<String, Value>`, you need to update every place that interacts with the map's values. The compiler errors will guide you — read each error message and it will point to the exact line that still expects a `String`.

</details>

---

## Exercise 4: Counting Operations

**Goal:** Track how many GET, SET, and DELETE operations your database has processed, and add a STATS command to display them.

### Step 1: Add counters to the Database struct

```rust
struct Database {
    data: HashMap<String, Value>,
    stats: OperationStats,
}

struct OperationStats {
    gets: usize,
    sets: usize,
    deletes: usize,
}

impl OperationStats {
    fn new() -> Self {
        OperationStats {
            gets: 0,
            sets: 0,
            deletes: 0,
        }
    }

    fn total(&self) -> usize {
        self.gets + self.sets + self.deletes
    }
}
```

### Step 2: Update the Database methods

```rust
impl Database {
    fn new() -> Self {
        Database {
            data: HashMap::new(),
            stats: OperationStats::new(),
        }
    }

    fn set(&mut self, key: String, value: Value) {
        self.data.insert(key, value);
        self.stats.sets += 1;
    }

    fn get(&mut self, key: &str) -> Option<&Value> {
        self.stats.gets += 1;
        self.data.get(key)
    }

    fn delete(&mut self, key: &str) -> bool {
        self.stats.deletes += 1;
        self.data.remove(key).is_some()
    }

    fn list(&self) -> Vec<(&String, &Value)> {
        self.data.iter().collect()
    }

    fn stats(&self) -> &OperationStats {
        &self.stats
    }

    fn entry_count(&self) -> usize {
        self.data.len()
    }
}
```

Notice that `get` now takes `&mut self` instead of `&self` — it needs to mutate the stats counter. This is a design tradeoff: tracking reads requires write access. In a real database, you would use atomic counters or interior mutability (`Cell`, `RefCell`, `AtomicUsize`) to avoid this. We will revisit this pattern in later chapters.

### Step 3: Add the STATS command

Add this arm to the `match` block in `main`, before the `_` arm:

```rust
            "STATS" => {
                let stats = db.stats();
                println!("--- toydb statistics ---");
                println!("  Keys stored:    {}", db.entry_count());
                println!("  SET operations: {}", stats.sets);
                println!("  GET operations: {}", stats.gets);
                println!("  DEL operations: {}", stats.deletes);
                println!("  Total ops:      {}", stats.total());
                println!("------------------------");
            }
```

### Step 4: Fix the borrow checker

After changing `get` to take `&mut self`, the GET arm in the REPL needs to handle borrowing carefully. The existing code should still work because we consume the `Option` result before the mutable borrow ends. But if you hit borrow checker issues, restructure the GET arm like this:

```rust
            "GET" => {
                if parts.len() < 2 {
                    println!("Usage: GET <key>");
                    continue;
                }
                let result = db.get(parts[1]).map(|v| {
                    format!("({}) {}", v.type_name(), v)
                });
                match result {
                    Some(display) => println!("{}", display),
                    None => println!("(nil)"),
                }
            }
```

The `.map()` converts `Option<&Value>` into `Option<String>` — creating an owned string from the borrowed value. Once the `String` is created, the borrow of `db` is released.

### Step 5: Test it

```bash
cargo run
```

```
toydb> SET x 100
OK
toydb> SET y 200
OK
toydb> GET x
(integer) 100
toydb> GET x
(integer) 100
toydb> GET z
(nil)
toydb> DELETE y
OK
toydb> STATS
--- toydb statistics ---
  Keys stored:    1
  SET operations: 2
  GET operations: 3
  DEL operations: 1
  Total ops:      6
------------------------
```

Notice that GET of a nonexistent key ("z") still counts as a GET operation — the database did the work of looking it up, even though nothing was found.

<details>
<summary>Hint: Understanding usize</summary>

We use `usize` for the counters. This is Rust's unsigned pointer-sized integer — 64 bits on a 64-bit system, 32 bits on a 32-bit system. It is the standard type for counting and indexing in Rust. All collection sizes (`.len()`) return `usize`, and all array/slice indexes must be `usize`. You cannot accidentally index with a negative number because `usize` cannot be negative.

</details>

---

## The Complete `main.rs`

Here is the full file after all four exercises:

```rust
use std::collections::HashMap;
use std::fmt;
use std::io::{self, Write};

// --- Value type ---

#[derive(Debug, Clone)]
enum Value {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::String(s) => write!(f, "{}", s),
            Value::Integer(n) => write!(f, "{}", n),
            Value::Float(n) => write!(f, "{}", n),
            Value::Boolean(b) => write!(f, "{}", b),
        }
    }
}

impl Value {
    fn type_name(&self) -> &str {
        match self {
            Value::String(_) => "string",
            Value::Integer(_) => "integer",
            Value::Float(_) => "float",
            Value::Boolean(_) => "boolean",
        }
    }

    fn parse(input: &str) -> Value {
        if input.eq_ignore_ascii_case("true") {
            return Value::Boolean(true);
        }
        if input.eq_ignore_ascii_case("false") {
            return Value::Boolean(false);
        }
        if let Ok(n) = input.parse::<i64>() {
            return Value::Integer(n);
        }
        if let Ok(n) = input.parse::<f64>() {
            return Value::Float(n);
        }
        Value::String(input.to_string())
    }
}

// --- Operation statistics ---

struct OperationStats {
    gets: usize,
    sets: usize,
    deletes: usize,
}

impl OperationStats {
    fn new() -> Self {
        OperationStats {
            gets: 0,
            sets: 0,
            deletes: 0,
        }
    }

    fn total(&self) -> usize {
        self.gets + self.sets + self.deletes
    }
}

// --- Database ---

struct Database {
    data: HashMap<String, Value>,
    stats: OperationStats,
}

impl Database {
    fn new() -> Self {
        Database {
            data: HashMap::new(),
            stats: OperationStats::new(),
        }
    }

    fn set(&mut self, key: String, value: Value) {
        self.data.insert(key, value);
        self.stats.sets += 1;
    }

    fn get(&mut self, key: &str) -> Option<&Value> {
        self.stats.gets += 1;
        self.data.get(key)
    }

    fn delete(&mut self, key: &str) -> bool {
        self.stats.deletes += 1;
        self.data.remove(key).is_some()
    }

    fn list(&self) -> Vec<(&String, &Value)> {
        self.data.iter().collect()
    }

    fn stats(&self) -> &OperationStats {
        &self.stats
    }

    fn entry_count(&self) -> usize {
        self.data.len()
    }
}

// --- REPL ---

fn main() {
    let mut db = Database::new();

    println!("toydb v0.1.0");
    println!("Type HELP for available commands.\n");

    loop {
        print!("toydb> ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(0) => break,
            Ok(_) => {}
            Err(e) => {
                eprintln!("Error reading input: {}", e);
                continue;
            }
        }

        let input = input.trim();
        if input.is_empty() {
            continue;
        }

        let parts: Vec<&str> = input.splitn(3, ' ').collect();
        let command = parts[0].to_uppercase();

        match command.as_str() {
            "SET" => {
                if parts.len() < 3 {
                    println!("Usage: SET <key> <value>");
                    continue;
                }
                let key = parts[1].to_string();
                let value = Value::parse(parts[2]);
                db.set(key, value);
                println!("OK");
            }
            "GET" => {
                if parts.len() < 2 {
                    println!("Usage: GET <key>");
                    continue;
                }
                let result = db.get(parts[1]).map(|v| {
                    format!("({}) {}", v.type_name(), v)
                });
                match result {
                    Some(display) => println!("{}", display),
                    None => println!("(nil)"),
                }
            }
            "DELETE" => {
                if parts.len() < 2 {
                    println!("Usage: DELETE <key>");
                    continue;
                }
                if db.delete(parts[1]) {
                    println!("OK");
                } else {
                    println!("(nil)");
                }
            }
            "LIST" => {
                let entries = db.list();
                if entries.is_empty() {
                    println!("(empty)");
                } else {
                    for (key, value) in entries {
                        println!("{} = ({}) {}", key, value.type_name(), value);
                    }
                }
            }
            "STATS" => {
                let stats = db.stats();
                println!("--- toydb statistics ---");
                println!("  Keys stored:    {}", db.entry_count());
                println!("  SET operations: {}", stats.sets);
                println!("  GET operations: {}", stats.gets);
                println!("  DEL operations: {}", stats.deletes);
                println!("  Total ops:      {}", stats.total());
                println!("------------------------");
            }
            "HELP" => {
                println!("Commands:");
                println!("  SET <key> <value>  — store a key-value pair");
                println!("  GET <key>          — retrieve a value by key");
                println!("  DELETE <key>       — remove a key-value pair");
                println!("  LIST               — show all key-value pairs");
                println!("  STATS              — show operation statistics");
                println!("  HELP               — show this message");
                println!("  EXIT               — quit");
            }
            "EXIT" | "QUIT" => {
                println!("Bye!");
                break;
            }
            _ => {
                println!(
                    "Unknown command: '{}'. Type HELP for available commands.",
                    parts[0]
                );
            }
        }
    }
}
```

---

## Rust Gym

Time for reps. These drills focus on variables, types, and HashMap — the spotlight concepts for this chapter. Do them in a Rust playground ([play.rust-lang.org](https://play.rust-lang.org)) or in a scratch `main.rs`.

### Drill 1: Type Declarations (Simple)

Declare one variable of each type and print them all in a single formatted string. Use at least: `i32`, `f64`, `String`, `&str`, `bool`, and `usize`.

```rust
fn main() {
    // Declare your variables here

    // Print them all in one line:
    // println!("int={}, float={}, owned={}, borrowed={}, flag={}, count={}", ...);
}
```

<details>
<summary>Solution</summary>

```rust
fn main() {
    let age: i32 = 25;
    let score: f64 = 98.6;
    let name: String = String::from("toydb");
    let label: &str = "database";
    let active: bool = true;
    let count: usize = 42;

    println!(
        "int={}, float={}, owned={}, borrowed={}, flag={}, count={}",
        age, score, name, label, active, count
    );
}
```

Output:
```
int=25, float=98.6, owned=toydb, borrowed=database, flag=true, count=42
```

</details>

### Drill 2: Word Frequency Counter (Medium)

Given a string, count the frequency of each word using a `HashMap<String, usize>`. Print the results sorted by frequency (highest first).

```rust
use std::collections::HashMap;

fn word_frequencies(text: &str) -> HashMap<String, usize> {
    // Your implementation here
    todo!()
}

fn main() {
    let text = "the quick brown fox jumps over the lazy dog the fox";
    let freqs = word_frequencies(text);

    // Sort by frequency (highest first), then alphabetically for ties
    let mut sorted: Vec<_> = freqs.iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));

    for (word, count) in sorted {
        println!("{:>8}: {}", word, count);
    }
}
```

<details>
<summary>Solution</summary>

```rust
use std::collections::HashMap;

fn word_frequencies(text: &str) -> HashMap<String, usize> {
    let mut freqs = HashMap::new();
    for word in text.split_whitespace() {
        let count = freqs.entry(word.to_string()).or_insert(0);
        *count += 1;
    }
    freqs
}

fn main() {
    let text = "the quick brown fox jumps over the lazy dog the fox";
    let freqs = word_frequencies(text);

    let mut sorted: Vec<_> = freqs.iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));

    for (word, count) in sorted {
        println!("{:>8}: {}", word, count);
    }
}
```

Output:
```
     the: 3
     fox: 2
   brown: 1
     dog: 1
   jumps: 1
    lazy: 1
    over: 1
   quick: 1
```

The key insight is `.entry().or_insert(0)`. The `entry` API gives you a mutable reference to the value at that key, inserting a default if the key does not exist. The `*count += 1` dereferences the mutable reference to increment the value. This is the idiomatic way to build frequency maps in Rust.

</details>

### Drill 3: Namespaced Key-Value Store (Advanced)

Implement a two-level `HashMap` — a namespace maps to a key-value store. Support commands like `SET users:name Alice` where `users` is the namespace and `name` is the key.

```rust
use std::collections::HashMap;

struct NamespacedDb {
    namespaces: HashMap<String, HashMap<String, String>>,
}

impl NamespacedDb {
    fn new() -> Self {
        // Your implementation
        todo!()
    }

    /// Parse "namespace:key" into (namespace, key).
    /// If no colon, use "default" as the namespace.
    fn parse_key(input: &str) -> (&str, &str) {
        // Your implementation
        todo!()
    }

    fn set(&mut self, ns: &str, key: &str, value: String) {
        // Your implementation
        todo!()
    }

    fn get(&self, ns: &str, key: &str) -> Option<&String> {
        // Your implementation
        todo!()
    }

    fn list_namespace(&self, ns: &str) -> Vec<(&String, &String)> {
        // Your implementation
        todo!()
    }

    fn list_namespaces(&self) -> Vec<&String> {
        // Your implementation
        todo!()
    }
}

fn main() {
    let mut db = NamespacedDb::new();

    db.set("users", "name", "Alice".to_string());
    db.set("users", "email", "alice@example.com".to_string());
    db.set("config", "port", "5432".to_string());
    db.set("config", "host", "localhost".to_string());

    println!("Namespaces: {:?}", db.list_namespaces());
    println!();

    for ns in db.list_namespaces() {
        println!("[{}]", ns);
        for (key, value) in db.list_namespace(ns) {
            println!("  {} = {}", key, value);
        }
    }

    println!();
    println!("users:name = {:?}", db.get("users", "name"));
    println!("config:port = {:?}", db.get("config", "port"));
    println!("missing:key = {:?}", db.get("missing", "key"));
}
```

<details>
<summary>Solution</summary>

```rust
use std::collections::HashMap;

struct NamespacedDb {
    namespaces: HashMap<String, HashMap<String, String>>,
}

impl NamespacedDb {
    fn new() -> Self {
        NamespacedDb {
            namespaces: HashMap::new(),
        }
    }

    fn parse_key(input: &str) -> (&str, &str) {
        match input.split_once(':') {
            Some((ns, key)) => (ns, key),
            None => ("default", input),
        }
    }

    fn set(&mut self, ns: &str, key: &str, value: String) {
        self.namespaces
            .entry(ns.to_string())
            .or_insert_with(HashMap::new)
            .insert(key.to_string(), value);
    }

    fn get(&self, ns: &str, key: &str) -> Option<&String> {
        self.namespaces.get(ns)?.get(key)
    }

    fn list_namespace(&self, ns: &str) -> Vec<(&String, &String)> {
        match self.namespaces.get(ns) {
            Some(map) => map.iter().collect(),
            None => Vec::new(),
        }
    }

    fn list_namespaces(&self) -> Vec<&String> {
        let mut ns: Vec<&String> = self.namespaces.keys().collect();
        ns.sort();
        ns
    }
}

fn main() {
    let mut db = NamespacedDb::new();

    db.set("users", "name", "Alice".to_string());
    db.set("users", "email", "alice@example.com".to_string());
    db.set("config", "port", "5432".to_string());
    db.set("config", "host", "localhost".to_string());

    println!("Namespaces: {:?}", db.list_namespaces());
    println!();

    for ns in db.list_namespaces() {
        println!("[{}]", ns);
        for (key, value) in db.list_namespace(ns) {
            println!("  {} = {}", key, value);
        }
    }

    println!();
    println!("users:name = {:?}", db.get("users", "name"));
    println!("config:port = {:?}", db.get("config", "port"));
    println!("missing:key = {:?}", db.get("missing", "key"));
}
```

Output:
```
Namespaces: ["config", "users"]

[config]
  host = localhost
  port = 5432
[users]
  email = alice@example.com
  name = Alice

users:name = Some("Alice")
config:port = Some("5432")
missing:key = None
```

Two things to notice in the `get` method: `self.namespaces.get(ns)?.get(key)`. The `?` operator on `Option` is elegant — if `get(ns)` returns `None`, the entire method returns `None` immediately. No nested `match`, no `if let`. The `?` operator propagates the "nothing" case automatically.

In `set`, the `.entry().or_insert_with(HashMap::new)` pattern creates a new inner HashMap only if the namespace does not exist yet. `or_insert_with` takes a closure (a function), unlike `or_insert` which takes a value. This means we only allocate the HashMap when actually needed.

</details>

---

## DSA in Context: Hash Tables

You just built a database on top of a hash table. Here is what is happening underneath.

### How HashMap works

A hash table stores key-value pairs in an array of *buckets*. To find which bucket a key belongs to:

1. **Hash** the key — run it through a hash function to produce a number
2. **Modulo** — take that number modulo the number of buckets to get an index
3. **Look up** — go directly to that bucket

```
Key: "user:1"
     │
     ▼
  hash("user:1") = 7392841028
     │
     ▼
  7392841028 % 16 = 4   (16 buckets)
     │
     ▼
  buckets[4] → ("user:1", "Alice")
```

### Performance characteristics

| Operation | Average case | Worst case |
|-----------|-------------|------------|
| `insert`  | O(1)        | O(n)       |
| `get`     | O(1)        | O(n)       |
| `remove`  | O(1)        | O(n)       |
| `contains_key` | O(1)  | O(n)       |

The worst case happens when many keys hash to the same bucket (*hash collision*). Rust's `HashMap` uses a technique called *Robin Hood hashing* with *SipHash* as the default hash function — chosen for collision resistance rather than raw speed. This makes it safe against hash-flooding denial-of-service attacks, which matters for a database.

### Hash tables vs B-trees in databases

Real databases use both:

- **Hash indexes** — O(1) exact lookups. Perfect for `WHERE id = 42`. Cannot do range queries (`WHERE id > 10 AND id < 50`) because hash order has no relation to key order.
- **B-tree indexes** — O(log n) lookups, but keys are stored in sorted order. Supports range queries, prefix queries, and ordered iteration. This is what most SQL databases use as their default index.

In Chapter 2, we will build a more sophisticated in-memory storage engine. In Chapter 3, we will add persistence — writing data to disk so it survives restarts. The humble HashMap you built today is the conceptual ancestor of every storage engine in this book.

---

## System Design Corner: "Design a Key-Value Store"

This is a classic system design interview question. Your toydb is the simplest valid answer — and a good starting point for discussing tradeoffs.

### The spectrum of key-value stores

| Level | Example | Durability | Concurrency | Distribution |
|-------|---------|------------|-------------|-------------- |
| **In-memory, single-thread** | Your toydb (this chapter) | None — data lost on restart | None — single user | None — single machine |
| **In-memory, persistent** | Redis | AOF/RDB snapshots | Single-threaded event loop | Redis Cluster |
| **Disk-based, single-node** | RocksDB, LevelDB | LSM-tree + WAL | Multi-threaded with locks | Embedded (no network) |
| **Distributed** | DynamoDB, etcd, CockroachDB | Replicated WAL + Raft/Paxos | Sharded + replicated | Multi-node consensus |

### Key questions an interviewer expects you to address

1. **In-memory vs on-disk?** In-memory is fast but limited by RAM and loses data on crash. Disk-based is durable but slower. Most production systems use both — hot data in memory, everything on disk.

2. **How do you handle concurrent access?** Our toydb has a single `&mut self` — only one operation at a time. Redis solves this with a single-threaded event loop. RocksDB uses fine-grained locks. Distributed databases use consensus protocols (Raft, Paxos) to coordinate across nodes.

3. **How do you scale?** *Vertical scaling* means a bigger machine. *Horizontal scaling* means more machines. For horizontal scaling, you need to *partition* (shard) the keyspace — decide which keys live on which nodes. Consistent hashing is a common technique.

4. **What consistency guarantees?** Our toydb is trivially consistent — one copy, one thread. Distributed stores must choose between strong consistency (every read sees the latest write) and eventual consistency (reads might see stale data, but will catch up). This is the CAP theorem in practice.

> **Interview talking point:** *"I would start with an in-memory hash map for the prototype, add a write-ahead log for durability, then introduce sharding with consistent hashing for horizontal scaling. For replication, I would use Raft consensus to ensure strong consistency across replicas."* — This is exactly the architecture we will build across the 18 chapters of this book.

---

## Design Insight: Obvious Code

> *"The best code is code that is obvious — if someone reads it, they immediately understand what it does."*
> — John Ousterhout, *A Philosophy of Software Design*

Look at the API you built:

```rust
db.set("name".to_string(), Value::parse("Alice"));
db.get("name");
db.delete("name");
db.list();
db.stats();
```

There is no cleverness here. The struct is named `Database`. The methods are named `set`, `get`, `delete`, `list`, `stats`. A programmer who has never seen your code can read any of these lines and know exactly what it does. This is not an accident — it is a design choice.

Ousterhout calls this "obvious code." The alternative is "clever code" — code that uses non-obvious tricks, obscure patterns, or requires extensive documentation to understand. Clever code is fun to write and painful to maintain.

Throughout this book, we will prefer obvious names over short names. `OperationStats` over `OpStats`. `entry_count` over `len` (because `len` could mean key count, byte count, or bucket count). `type_name` over `kind`. Every name should answer the question "what is this?" without context.

---

## What You Built

In this chapter, you:

1. **Created a key-value store** — a `Database` struct wrapping `HashMap<String, Value>` with `set`, `get`, `delete`, and `list` operations
2. **Built an interactive REPL** — reading from stdin, parsing commands, dispatching with `match`
3. **Implemented typed values** — using Rust's `enum` with `String`, `Integer`, `Float`, and `Boolean` variants
4. **Added operation tracking** — counting gets, sets, and deletes with a `STATS` command
5. **Learned Rust fundamentals** — `let` vs `let mut`, `String` vs `&str`, `HashMap`, `Option`, `match`, `enum`, `Display` trait, and the borrow checker

Your toydb is ephemeral — close the program and the data is gone. In Chapter 2, we will build a proper in-memory storage engine with better abstractions. In Chapter 3, we will add persistence so data survives restarts. But the shape of the API — `set`, `get`, `delete` — will remain recognizable throughout the entire book. That is the power of getting the interface right from the start.

---

## DS Deep Dive

Want to go deeper on hash tables? Read the narrative deep dive:

[Hash Table — "The key-value locker room"](../ds-narratives/ch01-hash-table-storage.md)

It covers open addressing vs chaining, load factors, resize strategies, and why Rust chose Robin Hood hashing — all through the lens of building a storage engine.

---

## Reference

The code you built in this chapter corresponds to these concepts in the [toydb reference implementation](https://github.com/erikgrinaker/toydb):

| Your code | toydb reference | Concept |
|-----------|----------------|---------|
| `Database` struct | [`src/storage/kv.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/storage/kv.rs) | The `KV` trait defines the key-value interface |
| `HashMap<String, Value>` | `Memory` storage engine | In-memory storage using `BTreeMap` (sorted keys) |
| `Value` enum | [`src/sql/types.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/sql/types.rs) | The `Value` enum with `Null`, `Boolean`, `Integer`, `Float`, `String` |
| `match` command dispatch | [`src/client.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/client.rs) | Client REPL command parsing |
| `OperationStats` | Not in toydb | Production databases expose stats via `SHOW STATUS` or metrics endpoints |
