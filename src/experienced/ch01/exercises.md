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
