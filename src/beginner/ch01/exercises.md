## Exercise 1: Your First Database (the HashMap)

**Goal:** Create a new Rust project and build a `Database` struct that wraps a `HashMap`, with methods to set, get, delete, and list keys.

### Step 1: Create the project

Open your terminal and type:

```bash
cargo new toydb
cd toydb
```

This creates a new Rust project with two files:

- `Cargo.toml` — your project's configuration file (like `package.json` in JavaScript or `setup.py` in Python)
- `src/main.rs` — your main code file

No external libraries are needed for this chapter. Everything comes from Rust's standard library.

### Step 2: Start with a simple HashMap

Open `src/main.rs` and replace its contents with:

```rust
use std::collections::HashMap;

fn main() {
    let mut data: HashMap<String, String> = HashMap::new();

    data.insert("name".to_string(), "toydb".to_string());

    if let Some(value) = data.get("name") {
        println!("name = {}", value);
    }
}
```

Run it:

```bash
cargo run
```

You should see:

```
name = toydb
```

> **What just happened?**
>
> You created a HashMap, inserted one key-value pair, and looked it up. This is the simplest possible database — a bag of key-value pairs stored in memory. Everything we build from here is an improvement on this foundation.

### Step 3: Wrap it in a struct

A bare `HashMap` works, but wrapping it in a **struct** gives us a place to add behavior and a name to call our database by. Think of a struct like a blueprint for an object.

Replace `src/main.rs` with:

```rust
use std::collections::HashMap;

struct Database {
    data: HashMap<String, String>,
}
```

A struct is a collection of named fields. Our `Database` struct has one field: `data`, which is a `HashMap<String, String>`.

> **Analogy: Struct = Blueprint**
>
> A struct is like a blueprint for a house. The blueprint says "this house has a kitchen, a bedroom, and a bathroom." The struct says "this Database has a data field of type HashMap." You do not live in the blueprint — you build a house from it. Similarly, you create an *instance* of a struct to use it.

### Step 4: Add methods to the struct

Now let's add functions that belong to our `Database`. In Rust, these are called **methods**, and they live inside an `impl` (implementation) block:

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
}
```

Let's understand every piece:

- `impl Database` — "Here are the methods for the `Database` type."
- `fn new() -> Self` — A function called `new` that returns a `Self` (which means "the type we are implementing," in this case `Database`).
- `Database { data: HashMap::new() }` — Create a `Database` instance with an empty `HashMap`.

This `new()` function is a **constructor** — it creates a new instance of our type. Rust does not have a special constructor syntax like some languages. By convention, constructors are named `new`.

### Step 5: Add the `set` method

```rust
impl Database {
    fn new() -> Self {
        Database {
            data: HashMap::new(),
        }
    }

    fn set(&mut self, key: String, value: String) {
        self.data.insert(key, value);
    }
}
```

The `set` method takes three parameters:

- `&mut self` — A mutable reference to the Database instance. The `&mut` means "I need to modify this Database." The `self` refers to the specific Database instance the method is called on.
- `key: String` — The key to store.
- `value: String` — The value to associate with the key.

Inside, it calls `self.data.insert(key, value)` to add the pair to the HashMap.

> **What is `&mut self`?**
>
> When you call `db.set(...)`, Rust needs to know: will this method just *look* at the database, or will it *change* it?
>
> - `&self` means "I will only read" (immutable reference)
> - `&mut self` means "I will read and write" (mutable reference)
>
> Since `set` adds data to the HashMap, it needs `&mut self`.

### Step 6: Add `get`, `delete`, and `list` methods

Add these methods inside the same `impl Database` block:

```rust
    fn get(&self, key: &str) -> Option<&String> {
        self.data.get(key)
    }

    fn delete(&mut self, key: &str) -> bool {
        self.data.remove(key).is_some()
    }

    fn list(&self) -> Vec<(&String, &String)> {
        self.data.iter().collect()
    }
```

Let's understand each one:

**`get`** takes `&self` (read-only) and a `key: &str` (a borrowed string — we do not need to own the key to look it up). It returns `Option<&String>` — either `Some` with a reference to the value, or `None` if the key does not exist.

**`delete`** takes `&mut self` (it modifies the database) and returns a `bool` — `true` if the key existed and was removed, `false` if the key was not found. The `.is_some()` call converts the `Option` returned by `HashMap::remove` into a simple `true`/`false`.

**`list`** takes `&self` (read-only) and returns a `Vec` (a growable list) of key-value pairs. The `.iter()` creates an iterator over the HashMap entries, and `.collect()` gathers them into a Vec.

### Step 7: Add the main function and test it

Now add the `main` function after the `impl` block:

```rust
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

Run it:

```bash
cargo run
```

Expected output (the order of "All keys" may vary — HashMap does not guarantee order):

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

> **What just happened?**
>
> You built a complete (if simple) database. It can store key-value pairs, look them up, list all entries, and delete entries. The `{:?}` format in `println!` shows the "debug" representation of a value — that is why you see `Some("toydb")` instead of just `toydb`. It is useful for seeing the structure of what you are working with.

### Common mistakes

**Mistake: Forgetting `.to_string()` when inserting**

```rust
db.set("name", "toydb");  // ERROR: expected String, found &str
```

Fix: convert the `&str` literals to `String`:

```rust
db.set("name".to_string(), "toydb".to_string());
```

**Mistake: Calling a `&mut self` method on an immutable variable**

```rust
let db = Database::new();   // not mutable!
db.set("name".to_string(), "toydb".to_string());  // ERROR
```

Fix: add `mut`:

```rust
let mut db = Database::new();
```

**Mistake: Using `db.get("name").unwrap()` without handling `None`**

If the key does not exist, `.unwrap()` will crash your program with a "panic." Always use `match` or `if let` to handle the `None` case:

```rust
// Bad — crashes if "name" is missing:
let value = db.get("name").unwrap();

// Good — handles both cases:
match db.get("name") {
    Some(value) => println!("Found: {}", value),
    None => println!("Key not found"),
}
```

### Step 8: Put the complete code together

Here is the entire `src/main.rs` at this point:

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

---

## Exercise 2: The Value Enum — Multiple Data Types

**Goal:** Right now, our database stores everything as a `String`. But real databases store numbers, booleans, and null values too. Let's add a `Value` enum that can hold different types of data.

### What is an enum?

An enum (short for "enumeration") is a type that can be one of several variants. Think of it like a multiple-choice question:

> What is in this box?
> - A) Nothing (Null)
> - B) A true/false value (Boolean)
> - C) A whole number (Integer)
> - D) A decimal number (Float)
> - E) A piece of text (String)

In Rust, you write this as:

```rust
enum Value {
    Null,
    Boolean(bool),
    Integer(i64),
    Float(f64),
    String(String),
}
```

Each variant can optionally carry data. `Null` carries nothing. `Boolean(bool)` carries a `bool` value inside it. `Integer(i64)` carries an `i64` number. This is much more powerful than enums in languages like C or Java, where enum variants are just labels.

> **Analogy: Enum = Multiple-choice answer**
>
> Imagine a form that asks "What kind of pet do you have?" with choices like Dog, Cat, Fish, None. A Rust enum is like that form — but each choice can also include extra information. "Dog" might include the dog's name. "Fish" might include how many fish. "None" needs no extra information.
>
> ```rust
> enum Pet {
>     None,                    // no pet
>     Dog(String),             // a dog with a name
>     Cat(String),             // a cat with a name
>     Fish(u32),               // some number of fish
> }
> ```

### Step 1: Define the Value enum

Add this above the `Database` struct in `src/main.rs`:

```rust
#[derive(Debug, Clone)]
enum Value {
    Null,
    Boolean(bool),
    Integer(i64),
    Float(f64),
    String(String),
}
```

The `#[derive(Debug, Clone)]` line is a special instruction to the Rust compiler:

- **`Debug`** — lets us print the value using `{:?}` for debugging.
- **`Clone`** — lets us create copies of the value with `.clone()`.

> **What is `#[derive(...)]`?**
>
> The `derive` attribute tells the Rust compiler to automatically generate code for common operations. Without `#[derive(Debug)]`, you could not print a `Value` with `{:?}`. Without `#[derive(Clone)]`, you could not copy a `Value`. Rust does not assume you want these capabilities — you have to opt in. This keeps things explicit and avoids hidden costs.

### Step 2: Create Value instances

Let's practice creating values of each variant:

```rust
let v1 = Value::Null;
let v2 = Value::Boolean(true);
let v3 = Value::Integer(42);
let v4 = Value::Float(3.14);
let v5 = Value::String("hello".to_string());
```

Notice the syntax: `EnumName::VariantName(data)`. The `::` is how you access things inside a type in Rust.

### Step 3: Add a Display implementation

When we print a `Value`, we want something readable — not the debug format. Rust provides the `Display` trait for this. A **trait** is like a contract that says "if you implement me, you must provide this behavior." We will explore traits deeply in Chapter 2. For now, just follow the pattern:

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

> **What just happened?**
>
> We told Rust how to display each variant of our `Value` enum:
> - `Null` displays as `NULL`
> - `Boolean(true)` displays as `true`
> - `Integer(42)` displays as `42`
> - `Float(3.14)` displays as `3.14`
> - `String("hello")` displays as `'hello'` (with single quotes, like SQL)
>
> The `match` statement checks which variant we have and runs the corresponding code. The `self` parameter refers to the `Value` instance. The `f` parameter is where the output goes (the screen, a string, etc.).

### Step 4: Update the Database to use Value

Change the `Database` struct to use `Value` instead of `String`:

```rust
struct Database {
    data: HashMap<String, Value>,
}
```

And update the methods:

```rust
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

    fn len(&self) -> usize {
        self.data.len()
    }
}
```

We also added a `len` method that returns how many key-value pairs are in the database. The `usize` type is Rust's standard type for counting things — it is an unsigned integer whose size matches your computer's architecture (64-bit on modern machines).

### Step 5: Test it

Update `main` to use the new typed values:

```rust
fn main() {
    let mut db = Database::new();

    db.set("name".to_string(), Value::String("toydb".to_string()));
    db.set("version".to_string(), Value::Float(0.1));
    db.set("max_connections".to_string(), Value::Integer(100));
    db.set("debug_mode".to_string(), Value::Boolean(false));
    db.set("description".to_string(), Value::Null);

    println!("Database has {} entries:\n", db.len());
    for (key, value) in db.list() {
        println!("  {} = {}", key, value);
    }

    println!("\nGET name = {:?}", db.get("name"));
    println!("GET missing = {:?}", db.get("missing"));
}
```

Run it:

```bash
cargo run
```

You should see output like:

```
Database has 5 entries:

  debug_mode = false
  description = NULL
  max_connections = 100
  name = 'toydb'
  version = 0.1

GET name = Some(String("toydb"))
GET missing = None
```

> **What just happened?**
>
> Our database now stores different types of data in the same HashMap. A `String` value, a `Float`, an `Integer`, a `Boolean`, and a `Null` — all living side by side. This is the power of enums: one type that can hold many different kinds of data, while the compiler ensures you always handle each kind correctly.

### Common mistakes with enums

**Mistake: Accessing the inner value without matching**

```rust
let v = Value::Integer(42);
// let n = v.0;  // ERROR: cannot access enum data directly
```

Fix: use `match` or `if let`:

```rust
if let Value::Integer(n) = v {
    println!("The number is {}", n);
}
```

**Mistake: Confusing `Value::String` with Rust's `String`**

Our enum has a variant called `String`, which happens to share a name with Rust's `String` type. This is allowed because they live in different namespaces. `Value::String(...)` is our enum variant. `String::from(...)` is Rust's standard string type. If it helps, think of `Value::String` as "a Value that *contains* a String."

---

## Exercise 3: Build a REPL

**Goal:** Build a REPL (Read-Eval-Print Loop) so you can interact with your database from the terminal. Type commands like `SET name toydb` and `GET name` and see the results.

### What is a REPL?

A REPL is a program that:

1. **R**eads input from the user
2. **E**valuates (processes) the input
3. **P**rints the result
4. **L**oops back to step 1

If you have ever used Python's interactive mode or a browser's JavaScript console, you have used a REPL.

### Step 1: Read input from the terminal

Rust's `std::io::stdin()` lets you read lines from the terminal:

```rust
use std::io;

fn main() {
    let mut input = String::new();

    print!("toydb> ");
    io::stdout().flush().unwrap();  // make sure the prompt appears

    io::stdin().read_line(&mut input).unwrap();

    println!("You typed: {}", input.trim());
}
```

Let's break this down:

- `String::new()` creates an empty, owned string.
- `print!` prints without a newline (unlike `println!`).
- `io::stdout().flush()` forces the prompt to appear immediately (by default, output is buffered).
- `read_line(&mut input)` reads one line from the terminal and appends it to `input`. It takes `&mut input` because it needs to modify the string by adding characters to it.
- `.trim()` removes the trailing newline character.

### Step 2: Parse the command

We need to split the user's input into parts. If they type `SET name toydb`, we need to extract the command (`SET`), the key (`name`), and the value (`toydb`).

```rust
fn parse_command(input: &str) -> Vec<&str> {
    input.trim().splitn(3, ' ').collect()
}
```

This function:
- `.trim()` removes whitespace from both ends
- `.splitn(3, ' ')` splits the string into at most 3 parts on spaces (so `SET name hello world` gives `["SET", "name", "hello world"]`)
- `.collect()` gathers the parts into a `Vec<&str>` (a list of string slices)

### Step 3: Build the REPL loop

A `loop` in Rust runs forever until you explicitly `break` out of it:

```rust
loop {
    // code here runs repeatedly

    if some_condition {
        break;  // exit the loop
    }
}
```

### Step 4: Parse values from string input

When the user types `SET age 30`, we need to figure out that `30` is an integer, not a string. Add this function:

```rust
fn parse_value(input: &str) -> Value {
    // Try to parse as integer
    if let Ok(n) = input.parse::<i64>() {
        return Value::Integer(n);
    }

    // Try to parse as float
    if let Ok(n) = input.parse::<f64>() {
        return Value::Float(n);
    }

    // Check for booleans
    match input.to_lowercase().as_str() {
        "true" => Value::Boolean(true),
        "false" => Value::Boolean(false),
        "null" | "none" => Value::Null,
        _ => Value::String(input.to_string()),
    }
}
```

> **What just happened?**
>
> The `.parse::<i64>()` method tries to convert a string to an `i64`. It returns a `Result` — either `Ok(number)` if it worked, or `Err(error)` if the string was not a valid number. The `if let Ok(n) = ...` pattern extracts the number if parsing succeeded, or skips to the next attempt if it failed.
>
> The `to_lowercase()` call makes boolean checking case-insensitive, so `TRUE`, `True`, and `true` all work.
>
> The `_` in the match is a **wildcard** — it matches anything not already matched. So any string that is not a number, boolean, or null gets stored as `Value::String`.

### Step 5: Put it all together

Here is the complete `src/main.rs` with the REPL:

```rust
use std::collections::HashMap;
use std::fmt;
use std::io::{self, Write};

// ── Value type ─────────────────────────────────────────────────

#[derive(Debug, Clone)]
enum Value {
    Null,
    Boolean(bool),
    Integer(i64),
    Float(f64),
    String(String),
}

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

// ── Database ───────────────────────────────────────────────────

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

    fn len(&self) -> usize {
        self.data.len()
    }
}

// ── Helpers ────────────────────────────────────────────────────

fn parse_value(input: &str) -> Value {
    if let Ok(n) = input.parse::<i64>() {
        return Value::Integer(n);
    }
    if let Ok(n) = input.parse::<f64>() {
        return Value::Float(n);
    }
    match input.to_lowercase().as_str() {
        "true" => Value::Boolean(true),
        "false" => Value::Boolean(false),
        "null" | "none" => Value::Null,
        _ => Value::String(input.to_string()),
    }
}

// ── REPL ───────────────────────────────────────────────────────

fn main() {
    let mut db = Database::new();

    println!("ToyDB v0.1.0");
    println!("Commands: SET <key> <value>, GET <key>, DELETE <key>, LIST, STATS, QUIT");
    println!();

    loop {
        // Print the prompt
        print!("toydb> ");
        io::stdout().flush().unwrap();

        // Read a line of input
        let mut input = String::new();
        if io::stdin().read_line(&mut input).unwrap() == 0 {
            // End of input (Ctrl+D on Unix, Ctrl+Z on Windows)
            break;
        }

        // Split the input into parts
        let parts: Vec<&str> = input.trim().splitn(3, ' ').collect();

        // Skip empty lines
        if parts.is_empty() || parts[0].is_empty() {
            continue;
        }

        // Process the command
        match parts[0].to_uppercase().as_str() {
            "SET" => {
                if parts.len() < 3 {
                    println!("Usage: SET <key> <value>");
                    continue;
                }
                let key = parts[1].to_string();
                let value = parse_value(parts[2]);
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

            "DELETE" | "DEL" => {
                if parts.len() < 2 {
                    println!("Usage: DELETE <key>");
                    continue;
                }
                if db.delete(parts[1]) {
                    println!("(1)");
                } else {
                    println!("(0)");
                }
            }

            "LIST" | "KEYS" => {
                let entries = db.list();
                if entries.is_empty() {
                    println!("(empty)");
                } else {
                    for (key, value) in entries {
                        println!("  {} = {}", key, value);
                    }
                }
            }

            "STATS" => {
                println!("Keys: {}", db.len());
            }

            "QUIT" | "EXIT" | "Q" => {
                println!("Goodbye!");
                break;
            }

            _ => {
                println!("Unknown command: {}", parts[0]);
                println!("Commands: SET, GET, DELETE, LIST, STATS, QUIT");
            }
        }
    }
}
```

### Step 6: Run the REPL

```bash
cargo run
```

Try these commands:

```
toydb> SET name toydb
OK
toydb> SET version 0.1
OK
toydb> SET max_connections 100
OK
toydb> SET debug false
OK
toydb> GET name
'toydb'
toydb> GET version
0.1
toydb> GET max_connections
100
toydb> GET missing
(nil)
toydb> LIST
  debug = false
  max_connections = 100
  name = 'toydb'
  version = 0.1
toydb> STATS
Keys: 4
toydb> DELETE version
(1)
toydb> DELETE version
(0)
toydb> STATS
Keys: 3
toydb> QUIT
Goodbye!
```

> **What just happened?**
>
> You built a complete interactive database! It reads commands from the terminal, processes them, and shows results. The REPL loop runs forever until you type QUIT (or press Ctrl+D).
>
> The `match` statement on the command string checks each possibility. The `continue` keyword skips the rest of the loop body and goes back to reading the next command. The `break` keyword exits the loop entirely.

### Common mistakes

**Mistake: Forgetting `io::stdout().flush()`**

Without `flush()`, the `print!("toydb> ")` prompt might not appear until after you type your input. This is because Rust buffers standard output for performance. `flush()` forces the buffer to be written immediately.

**Mistake: Not handling empty input**

If the user just presses Enter, the input is `"\n"`. After trimming, it is `""`. Without the `if parts[0].is_empty()` check, the program would try to match an empty string as a command and print "Unknown command."

**Mistake: Forgetting `continue` after error messages**

```rust
"SET" => {
    if parts.len() < 3 {
        println!("Usage: SET <key> <value>");
        // Without `continue`, execution falls through to the code below!
    }
    // This code runs even when parts.len() < 3 — bug!
    let key = parts[1].to_string();  // panic: index out of bounds
}
```

---

## Exercise 4: Add a STATS Command with Type Counts

**Goal:** Enhance the STATS command to show a breakdown of how many values of each type are stored.

### Step 1: Add a stats method to Database

Add this method inside the `impl Database` block:

```rust
    fn stats(&self) -> (usize, usize, usize, usize, usize, usize) {
        let mut nulls = 0;
        let mut booleans = 0;
        let mut integers = 0;
        let mut floats = 0;
        let mut strings = 0;

        for value in self.data.values() {
            match value {
                Value::Null => nulls += 1,
                Value::Boolean(_) => booleans += 1,
                Value::Integer(_) => integers += 1,
                Value::Float(_) => floats += 1,
                Value::String(_) => strings += 1,
            }
        }

        (self.data.len(), nulls, booleans, integers, floats, strings)
    }
```

> **What just happened?**
>
> The `stats` method iterates over all values in the HashMap and counts how many of each variant exist. The `self.data.values()` method gives us an iterator over just the values (ignoring keys).
>
> The return type `(usize, usize, usize, usize, usize, usize)` is a **tuple** — a fixed-size collection of values. The first element is the total count, followed by counts for each type. Tuples are useful when you want to return multiple values from a function without defining a new struct.

### Step 2: Update the STATS command in the REPL

Replace the `"STATS"` match arm:

```rust
            "STATS" => {
                let (total, nulls, bools, ints, floats, strings) = db.stats();
                println!("Total keys: {}", total);
                if total > 0 {
                    println!("  Null:    {}", nulls);
                    println!("  Boolean: {}", bools);
                    println!("  Integer: {}", ints);
                    println!("  Float:   {}", floats);
                    println!("  String:  {}", strings);
                }
            }
```

The `let (total, nulls, bools, ints, floats, strings) = db.stats();` line **destructures** the tuple — it unpacks the six values into six separate variables in one line.

### Step 3: Test it

```bash
cargo run
```

```
toydb> SET name toydb
OK
toydb> SET age 30
OK
toydb> SET active true
OK
toydb> SET score 9.5
OK
toydb> SET notes null
OK
toydb> STATS
Total keys: 5
  Null:    1
  Boolean: 1
  Integer: 1
  Float:   1
  String:  1
```

---

## Exercises

These exercises reinforce what you learned. Try them before moving to Chapter 2.

**Exercise 1.1: Add an UPDATE command**

Add an `UPDATE <key> <value>` command that only works if the key already exists. If the key exists, update its value and print `OK`. If not, print `(key not found)`.

<details>
<summary>Hint</summary>

Check if the key exists with `db.get(key).is_some()` before calling `db.set(...)`.

```rust
"UPDATE" => {
    if parts.len() < 3 {
        println!("Usage: UPDATE <key> <value>");
        continue;
    }
    if db.get(parts[1]).is_some() {
        let key = parts[1].to_string();
        let value = parse_value(parts[2]);
        db.set(key, value);
        println!("OK");
    } else {
        println!("(key not found)");
    }
}
```

</details>

**Exercise 1.2: Add a COUNT command**

Add a `COUNT` command that prints the total number of keys. (This is simpler than STATS — just the count.)

<details>
<summary>Hint</summary>

```rust
"COUNT" => {
    println!("{}", db.len());
}
```

</details>

**Exercise 1.3: Add a TYPE command**

Add a `TYPE <key>` command that prints the type of the value stored at a key: `null`, `boolean`, `integer`, `float`, or `string`.

<details>
<summary>Hint</summary>

Add a method to `Value`:

```rust
impl Value {
    fn type_name(&self) -> &str {
        match self {
            Value::Null => "null",
            Value::Boolean(_) => "boolean",
            Value::Integer(_) => "integer",
            Value::Float(_) => "float",
            Value::String(_) => "string",
        }
    }
}
```

Then in the REPL:

```rust
"TYPE" => {
    if parts.len() < 2 {
        println!("Usage: TYPE <key>");
        continue;
    }
    match db.get(parts[1]) {
        Some(value) => println!("{}", value.type_name()),
        None => println!("(nil)"),
    }
}
```

</details>

**Exercise 1.4: Add a CLEAR command**

Add a `CLEAR` command that deletes all keys from the database.

<details>
<summary>Hint</summary>

Add a method to `Database`:

```rust
fn clear(&mut self) {
    self.data.clear();
}
```

Then in the REPL:

```rust
"CLEAR" => {
    db.clear();
    println!("OK");
}
```

</details>

---

## Key Takeaways

- **Variables in Rust are immutable by default.** Use `mut` to make them mutable. This prevents accidental changes.
- **Rust has strong, static types** with inference. The compiler figures out types for you, but you can annotate them when it helps.
- **`String` is owned text, `&str` is borrowed text.** Collections like HashMap need owned data.
- **Enums with data** let one type hold multiple kinds of values. Use `match` to handle each variant.
- **`Option<T>`** replaces null. It forces you to handle the "value might not exist" case.
- **HashMap** is a key-value store — the simplest possible database.
- **A REPL** is the simplest user interface for a database: read a command, process it, print the result, repeat.
