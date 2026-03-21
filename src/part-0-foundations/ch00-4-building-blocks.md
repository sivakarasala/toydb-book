# The Building Blocks — Variables, Functions & Loops

This is the chapter where you go from printing hardcoded text to writing programs that *compute things*. By the end, you will have a working in-memory key-value store that stores entries, looks them up, counts them, and reports statistics.

That is a real program. You will write it yourself.

We are covering four concepts:

1. **Variables** — storing data
2. **Types** — what kinds of data Rust understands
3. **Functions** — reusable blocks of logic
4. **Control flow** — making decisions and repeating actions

Each builds on the previous one. Take your time. Type every example — do not copy and paste.

---

## Variables: Storing Data

In Chapter 0.3, you hardcoded everything directly into `println!` statements:

```rust
println!("Table: users");
println!("Rows: 42");
```

This works, but it has a problem. What if "users" appears in five different `println!` lines and you want to change it to "products"? You would have to change it in five places. Miss one and your program is inconsistent.

**Variables** solve this. A variable is a named container that holds a value. You set it once, then use the name everywhere.

### Creating a variable with `let`

```rust
fn main() {
    let table_name = "users";
    println!("Table: {}", table_name);
}
```

Let's break down `let table_name = "users";`:

| Part | Meaning |
|------|---------|
| `let` | "I am creating a new variable" |
| `table_name` | The name you chose for the variable |
| `=` | "and its value is" |
| `"users"` | The value being stored |
| `;` | End of the statement |

The `{}` inside the `println!` string is a **placeholder**. When the program runs, Rust replaces `{}` with the value of `table_name`. So the output is:

```text
Table: users
```

You can use multiple placeholders in one `println!`:

```rust
fn main() {
    let table_name = "users";
    let row_count = 42;
    let size_kb = 128.5;
    println!("{}: {} rows, {} KB", table_name, row_count, size_kb);
}
```

Output:

```text
users: 42 rows, 128.5 KB
```

The placeholders `{}` are filled in order — the first `{}` gets `table_name`, the second gets `row_count`, the third gets `size_kb`.

### Immutability: Rust's safety default

Try this program:

```rust
fn main() {
    let row_count = 42;
    println!("Before insert: {} rows", row_count);
    row_count = 43;
    println!("After insert: {} rows", row_count);
}
```

Run it with `cargo run`. You will get an error:

```text
error[E0384]: cannot assign twice to immutable variable `row_count`
 --> src/main.rs:4:5
  |
2 |     let row_count = 42;
  |         --------- first assignment to `row_count`
3 |     println!("Before insert: {} rows", row_count);
4 |     row_count = 43;
  |     ^^^^^^^^^^^^^^ cannot assign twice to immutable variable
  |
help: consider making this binding mutable: `let mut row_count`
```

In Rust, variables are **immutable** by default. That means once you set a value, you cannot change it.

*Why?* Because changing data is one of the most common sources of bugs in programming. If a value is immutable, you can look at the line where it was created and know *with certainty* what it contains — no need to trace through the whole program to see if something changed it later.

Think of it like a database transaction log: once a record is written, it does not change. If you want a new value, you create a new entry. That is a feature, not a limitation.

### Mutable variables with `let mut`

When you genuinely need a value to change — like tracking a running count of rows — use `let mut`:

```rust
fn main() {
    let mut row_count = 42;
    println!("Before insert: {} rows", row_count);
    row_count = 43;
    println!("After insert: {} rows", row_count);
}
```

Output:

```text
Before insert: 42 rows
After insert: 43 rows
```

The `mut` keyword (short for "mutable") tells Rust: "I know this value will change, and that is intentional."

### The rule of thumb

- Use `let` (immutable) by default
- Only add `mut` when you have a reason to change the value later
- If you are not sure, start with `let` — the compiler will tell you if you need `mut`

---

## Types: What Kinds of Data Exist

Every value in Rust has a **type** — a label that tells the compiler what kind of data it is and what you can do with it. You would not try to add "users" + "products" and expect a number. Types prevent that kind of mistake.

Here are the types you will use most often:

### Integers: `i32`

Whole numbers, positive or negative. "i32" means "a 32-bit integer."

```rust
let row_count: i32 = 42;
let column_count: i32 = 5;
let negative_example: i32 = -1;
```

Use `i32` for anything you count in whole numbers: rows, columns, table counts, page numbers.

### Floating-point numbers: `f64`

Numbers with decimal points. "f64" means "a 64-bit floating-point number."

```rust
let size_kb: f64 = 128.5;
let query_time_ms: f64 = 3.14;
let fill_factor: f64 = 0.75;
```

Use `f64` for sizes, timings, percentages — anything that can have a fractional part. Note that even `128.0` needs the decimal point to be an `f64`.

### Booleans: `bool`

True or false. Only two possible values.

```rust
let is_indexed: bool = true;
let is_empty: bool = false;
```

Use `bool` for yes/no questions: Is the table indexed? Is the result set empty? Is the connection open?

### Text: `String` and `&str`

Rust has two kinds of text, which can be confusing at first. For now, here is the simple version:

- `&str` (pronounced "string slice") — text that is written directly in your code, like `"users"`. You cannot change it.
- `String` — text that your program creates or modifies at runtime. You can change it.

```rust
let table_name: &str = "users";                       // fixed text — a string slice
let query: String = String::from("SELECT * FROM users"); // dynamic text — a String
```

For Part 0, we will mostly use `&str` because we are working with fixed text. You will learn the full story of `String` vs `&str` when we build the ToyDB engine in Chapter 1.

### Type annotations vs. type inference

In all the examples above, I wrote the type explicitly: `let row_count: i32 = 42;`. But Rust can usually *figure out* the type on its own:

```rust
let row_count = 42;          // Rust infers i32
let size_kb = 128.5;         // Rust infers f64
let table_name = "users";    // Rust infers &str
let is_indexed = true;       // Rust infers bool
```

Both forms are valid. When you are learning, writing the type explicitly can help you remember what each variable is. As you get comfortable, you can let Rust infer types and only annotate when it is ambiguous or when you want to be extra clear.

### Why types matter

Types catch bugs at compile time, before your program ever runs. If you accidentally write:

```rust
let total = "forty-two" * 3;
```

Rust will refuse to compile and tell you that you cannot multiply text by a number. In many other languages, this kind of mistake would silently produce a weird result at runtime. Rust catches it immediately.

---

## Functions: Reusable Blocks of Logic

You already know one function: `fn main()`. It is the entry point of every Rust program. But you can create your own functions to organize your code and avoid repeating yourself.

### Why functions?

Imagine you need to calculate the size of a table (rows x columns x bytes per cell) in five different places in your program. Without functions, you would write the same math five times. If you later discover a bug in that math, you would have to fix it in five places.

A function lets you write the logic *once* and *call* it from anywhere.

### Defining a function

```rust
fn table_size_bytes(rows: i32, columns: i32, bytes_per_cell: f64) -> f64 {
    let size = rows as f64 * columns as f64 * bytes_per_cell;
    size
}
```

Let's break this down piece by piece:

| Part | Meaning |
|------|---------|
| `fn` | "I am defining a function" |
| `table_size_bytes` | The name of the function |
| `(rows: i32, columns: i32, bytes_per_cell: f64)` | **Parameters** — the inputs the function needs, each with a name and type |
| `-> f64` | **Return type** — this function produces an `f64` value |
| `{ ... }` | The **body** — the code that runs when the function is called |
| `rows as f64` | Converts the integer `rows` to a floating-point number so we can multiply it with `bytes_per_cell` |
| `size` | The last expression without a semicolon is the **return value** |

That last point is important and unique to Rust: **the last expression in a function is what gets returned.** No `return` keyword needed (though Rust does have one for early returns).

### Calling a function

```rust
fn table_size_bytes(rows: i32, columns: i32, bytes_per_cell: f64) -> f64 {
    rows as f64 * columns as f64 * bytes_per_cell
}

fn main() {
    let size = table_size_bytes(1000, 5, 64.0);
    println!("Table size: {} bytes", size);
}
```

Output:

```text
Table size: 320000 bytes
```

When you write `table_size_bytes(1000, 5, 64.0)`:

1. Rust passes `1000` into `rows`, `5` into `columns`, and `64.0` into `bytes_per_cell`
2. The function calculates `1000.0 * 5.0 * 64.0 = 320000.0`
3. The result `320000.0` is returned and stored in the `size` variable in `main`

### Functions with no return value

Not every function needs to return something. A function that just prints output does not need a return type:

```rust
fn print_table_header(table_name: &str) {
    println!("===========================");
    println!("  Table: {}", table_name);
    println!("===========================");
}

fn main() {
    print_table_header("users");
}
```

Output:

```text
===========================
  Table: users
===========================
```

When there is no `-> Type` in the function signature, the function returns nothing (technically it returns `()`, called the "unit type," but you do not need to think about that now).

### Function placement

In Rust, it does not matter whether you define a function before or after `main`. This works fine:

```rust
fn main() {
    print_greeting();
}

fn print_greeting() {
    println!("Welcome to ToyDB!");
}
```

Rust reads the entire file before compiling, so it knows about all functions regardless of their position.

---

## Control Flow: Making Decisions and Repeating Actions

So far, our programs run every line once, top to bottom. Real programs need to **make decisions** ("if the table is empty, print a warning") and **repeat actions** ("do this for each row in the table"). That is what control flow gives us.

### `if` / `else` — Making decisions

```rust
fn main() {
    let row_count = 42;

    if row_count > 1000 {
        println!("Large table!");
    } else if row_count > 0 {
        println!("Table has data.");
    } else {
        println!("Table is empty.");
    }
}
```

Output:

```text
Table has data.
```

How it works:

1. Rust checks `row_count > 1000`. Is 42 greater than 1000? No. Skip that block.
2. Rust checks `row_count > 0`. Is 42 greater than 0? Yes. Run that block.
3. The `else` block is skipped because a condition already matched.

The conditions are checked **in order, from top to bottom.** The first one that is true wins. All others are skipped.

### Comparison operators

| Operator | Meaning | Example |
|----------|---------|---------|
| `>` | greater than | `row_count > 1000` |
| `<` | less than | `columns < 3` |
| `>=` | greater than or equal to | `size >= 1024.0` |
| `<=` | less than or equal to | `query_time <= 5.0` |
| `==` | equal to | `table_name == "users"` |
| `!=` | not equal to | `status != "offline"` |

Note the double equals `==` for comparison. A single `=` means "assign a value." A double `==` means "check if equal." Mixing them up is a common mistake.

### `match` — Pattern matching

Rust has a powerful alternative to chains of `if/else` called `match`:

```rust
fn main() {
    let status = "active";

    match status {
        "active" => println!("Database is running."),
        "readonly" => println!("Database is in read-only mode."),
        "offline" => println!("Database is offline."),
        _ => println!("Unknown status."),
    }
}
```

Output:

```text
Database is running.
```

The `_` is a **wildcard** — it matches anything not covered by the other arms. Rust requires `match` to be *exhaustive*, meaning every possible value must be handled. The wildcard ensures that.

We will use `match` extensively when parsing database commands in later chapters. For now, just know it exists as a cleaner alternative to long `if/else` chains.

### `for` loops — Repeating with a range

A `for` loop runs a block of code once for each value in a sequence:

```rust
fn main() {
    for row_id in 1..=5 {
        println!("Row {}: (data)", row_id);
    }
}
```

Output:

```text
Row 1: (data)
Row 2: (data)
Row 3: (data)
Row 4: (data)
Row 5: (data)
```

The `1..=5` is a **range**. It means "from 1 to 5, inclusive." Each time through the loop, `row_id` takes the next value in the range: 1, then 2, then 3, then 4, then 5.

Two kinds of ranges:

| Syntax | Meaning | Values |
|--------|---------|--------|
| `1..5` | 1 up to but *not including* 5 | 1, 2, 3, 4 |
| `1..=5` | 1 up to and *including* 5 | 1, 2, 3, 4, 5 |

For row IDs, `1..=5` (inclusive) is almost always what you want, because "5 rows" means rows 1 through 5.

### Loops with mutable variables: running totals

Loops become powerful when combined with a mutable variable that accumulates a result:

```rust
fn main() {
    let bytes_per_row = 64.0;
    let mut total_bytes = 0.0;

    for row_id in 1..=5 {
        total_bytes += bytes_per_row;
        println!("Row {}: {} bytes (running total: {} bytes)",
            row_id, bytes_per_row, total_bytes);
    }

    println!("Total storage: {} bytes", total_bytes);
}
```

Output:

```text
Row 1: 64 bytes (running total: 64 bytes)
Row 2: 64 bytes (running total: 128 bytes)
Row 3: 64 bytes (running total: 192 bytes)
Row 4: 64 bytes (running total: 256 bytes)
Row 5: 64 bytes (running total: 320 bytes)
Total storage: 320 bytes
```

The line `total_bytes += bytes_per_row;` means "add `bytes_per_row` to `total_bytes`." It is shorthand for `total_bytes = total_bytes + bytes_per_row;`.

Notice that `total_bytes` must be `let mut` because its value changes each iteration.

### `while` loops — Repeating with a condition

A `while` loop keeps running as long as a condition is true:

```rust
fn main() {
    let mut remaining_rows = 5;

    while remaining_rows > 0 {
        println!("Deleting row {}...", remaining_rows);
        remaining_rows -= 1;
    }

    println!("Table is now empty.");
}
```

Output:

```text
Deleting row 5...
Deleting row 4...
Deleting row 3...
Deleting row 2...
Deleting row 1...
Table is now empty.
```

We will use `for` loops much more often than `while` loops, but it is good to know both exist. Use `for` when you know how many times to repeat. Use `while` when you want to keep going until a condition changes.

### `loop` — Infinite loops with explicit break

Sometimes you want a loop that runs until you explicitly tell it to stop:

```rust
fn main() {
    let mut attempts = 0;

    loop {
        attempts += 1;
        println!("Connection attempt {}...", attempts);

        if attempts >= 3 {
            println!("Connected to ToyDB!");
            break;
        }
    }
}
```

Output:

```text
Connection attempt 1...
Connection attempt 2...
Connection attempt 3...
Connected to ToyDB!
```

The `break` keyword exits the loop immediately. Without it, a `loop` runs forever. We will use this pattern later when building a database REPL (read-eval-print loop) that waits for user commands.

---

## Collections: Grouping Data Together

So far, each variable holds one value. But a database needs to hold *many* values — many rows, many keys, many entries. That is where **collections** come in.

### `Vec` — A growable list

A `Vec` (short for "vector") is a list that can grow or shrink. Think of it like a table with one column:

```rust
fn main() {
    let mut names: Vec<&str> = Vec::new();

    names.push("Alice");
    names.push("Bob");
    names.push("Charlie");

    println!("Users: {:?}", names);
    println!("Count: {}", names.len());
}
```

Output:

```text
Users: ["Alice", "Bob", "Charlie"]
Count: 3
```

Let's break this down:

| Part | Meaning |
|------|---------|
| `Vec<&str>` | A vector that holds string slices |
| `Vec::new()` | Create a new, empty vector |
| `names.push("Alice")` | Add "Alice" to the end of the list |
| `names.len()` | Get the number of elements in the list |
| `{:?}` | A special placeholder that prints the whole vector (the regular `{}` does not work for vectors) |

You can loop over a vector with `for`:

```rust
fn main() {
    let tables = vec!["users", "products", "orders"];

    for table in &tables {
        println!("Table: {}", table);
    }

    println!("Total tables: {}", tables.len());
}
```

Output:

```text
Table: users
Table: products
Table: orders
Total tables: 3
```

The `vec!` macro is a shortcut for creating a vector with initial values (like how `println!` is a shortcut for printing). The `&` in `for table in &tables` means "borrow the vector" — we want to look at its contents without taking ownership. Do not worry about ownership yet; you will learn it properly when we build ToyDB in Part 1.

### `String` — Growable text

You have already seen `&str` for fixed text. `String` is the growable version — you can build it up piece by piece:

```rust
fn main() {
    let mut result = String::new();

    result.push_str("SELECT ");
    result.push_str("name, email ");
    result.push_str("FROM users");

    println!("Query: {}", result);
}
```

Output:

```text
Query: SELECT name, email FROM users
```

We will use `String` heavily when building ToyDB's query parser.

---

## Putting It All Together

Let's combine everything — variables, types, functions, if/else, for loops, and Vec — into a single program: a tiny in-memory key-value store.

Here is the plan (pseudocode first, as we learned in Chapter 0.3):

```text
PROGRAM: Tiny Key-Value Store

1. Create empty lists for keys and values
2. Store three entries: name=Alice, email=alice@example.com, role=admin
3. Print the store header
4. Loop through each entry and print it
5. Print the total number of entries
6. Classify the store size: Empty / Small / Large
7. Look up a specific key and print the result
```

And here is the Rust implementation:

```rust
fn classify_store(count: i32) -> &'static str {
    if count > 100 {
        "Large"
    } else if count > 0 {
        "Small"
    } else {
        "Empty"
    }
}

fn find_value<'a>(keys: &[&str], values: &'a [&str], search_key: &str) -> Option<&'a str> {
    for i in 0..keys.len() {
        if keys[i] == search_key {
            return Some(values[i]);
        }
    }
    None
}

fn main() {
    // Storage — two parallel vectors: keys and values
    let keys = vec!["name", "email", "role"];
    let values = vec!["Alice", "alice@example.com", "admin"];

    // Header
    println!("===========================");
    println!("  ToyDB Key-Value Store");
    println!("===========================");

    // List all entries
    for i in 0..keys.len() {
        println!("  {} = {}", keys[i], values[i]);
    }

    // Statistics
    println!("===========================");
    let count = keys.len() as i32;
    let classification = classify_store(count);
    println!("  Entries: {}", count);
    println!("  Size: {}", classification);
    println!("===========================");

    // Lookup
    let search_key = "email";
    match find_value(&keys, &values, search_key) {
        Some(value) => println!("  Lookup '{}': {}", search_key, value),
        None => println!("  Lookup '{}': not found", search_key),
    }
    println!("===========================");
}
```

Output:

```text
===========================
  ToyDB Key-Value Store
===========================
  name = Alice
  email = alice@example.com
  role = admin
===========================
  Entries: 3
  Size: Small
===========================
  Lookup 'email': alice@example.com
===========================
```

Take a moment to trace through this program line by line. You understand most of the pieces now:

- `let` and `let mut` for variables
- `i32`, `f64`, `&str`, and `bool` for types
- `Vec` for collections
- `fn classify_store(...)` and `fn find_value(...)` for functions
- `for i in 0..keys.len()` for looping
- `if/else if/else` for classification
- `match` for handling the lookup result

A few new things appeared in the `find_value` function. Let's address them briefly:

- **`Option<&str>`** is Rust's way of saying "this function might return a string, or it might return nothing." `Some(value)` means "found it," and `None` means "not found." This is how Rust handles the possibility of missing data — no null pointers, no crashes.
- **`<'a>`** is a lifetime annotation. It tells Rust that the returned string reference lives as long as the `values` slice does. Do not worry about understanding lifetimes yet. For now, just know that Rust is making sure you cannot use a reference after the data it points to is gone.
- **`return Some(values[i]);`** uses the explicit `return` keyword for an early exit from the function. This is one of the cases where `return` is needed — when you want to stop the loop and return a value before reaching the end of the function.

These concepts (Option, lifetimes, slices) will be explained properly in Part 1. For now, the important thing is that you can read the code and follow the logic: loop through keys, find a match, return the corresponding value.

A note on `&'static str`: You may have noticed `-> &'static str` in the `classify_store` function. The `'static` part is a *lifetime annotation* — it tells Rust that the returned text lives for the entire duration of the program. Since `"Large"`, `"Small"`, and `"Empty"` are written directly in the source code, they are always available. Do not worry about understanding lifetimes yet. For now, just know that when a function returns a hardcoded string, you write `-> &'static str`. You will learn lifetimes properly when we build the ToyDB engine.

---

## Exercises

### Exercise 1: Variables

**Goal:** Declare variables for a database table and print them. Then experience the immutability error and fix it.

**Instructions:**

1. Create a new project: `cargo new table_variables`
2. In `src/main.rs`, declare these variables:
   - `table_name` — a `String` with value `"users"` (use `String::from("users")`)
   - `row_count` — an `i32` with value `0`
   - `size_kb` — an `f64` with value `0.0`
   - `is_indexed` — a `bool` with value `false`
3. Print all four values on separate lines, using `{}` placeholders
4. Now *after* the print statements, try to change `row_count` to `42`. Compile and read the error.
5. Fix the error by making the variable mutable.
6. Print the updated value of `row_count`.

Expected final output:

```text
Table: users
Rows: 0
Size: 0 KB
Indexed: false
Updated — Rows: 42
```

<details>
<summary>Hint 1</summary>

To create a `String`, use `let table_name: String = String::from("users");`. To print it, use `println!("Table: {}", table_name);`.

</details>

<details>
<summary>Hint 2</summary>

When you try to reassign `row_count = 42;` and the compiler complains, read the `help:` line. It will tell you to add `mut` to the declaration: `let mut row_count: i32 = 0;`.

</details>

<details>
<summary>Hint 3</summary>

You need `println!("Updated — Rows: {}", row_count);` after the reassignment.

</details>

<details>
<summary>Solution</summary>

```rust
fn main() {
    let table_name: String = String::from("users");
    let mut row_count: i32 = 0;
    let size_kb: f64 = 0.0;
    let is_indexed: bool = false;

    println!("Table: {}", table_name);
    println!("Rows: {}", row_count);
    println!("Size: {} KB", size_kb);
    println!("Indexed: {}", is_indexed);

    row_count = 42;
    println!("Updated — Rows: {}", row_count);
}
```

When you first write `let row_count: i32 = 0;` (without `mut`) and try `row_count = 42;`, the compiler error says:

```text
error[E0384]: cannot assign twice to immutable variable `row_count`
```

Adding `mut` fixes it. This is Rust's immutability default in action — it made you explicitly say "yes, I want this value to change."

</details>

---

### Exercise 2: Functions

**Goal:** Write a function that calculates table size in bytes, and call it from `main`.

**Instructions:**

1. Create a new project: `cargo new size_calculator`
2. Write a function `fn table_size_bytes(rows: i32, columns: i32, bytes_per_cell: f64) -> f64` that returns `rows * columns * bytes_per_cell` (remember to convert `rows` and `columns` to `f64` before multiplying)
3. In `main`, call the function for a users table: 1000 rows, 5 columns, 64 bytes per cell
4. Store the result in a variable called `size`
5. Print: `"users table — Size: XXXX bytes"` where XXXX is the calculated value

Expected output:

```text
users table — Size: 320000 bytes
```

<details>
<summary>Hint 1</summary>

To convert an `i32` to `f64`, use `as f64`. For example: `rows as f64`.

</details>

<details>
<summary>Hint 2</summary>

The function body should be: `rows as f64 * columns as f64 * bytes_per_cell`. Remember, the last expression (without a semicolon) is the return value.

</details>

<details>
<summary>Hint 3</summary>

Call the function like this: `let size = table_size_bytes(1000, 5, 64.0);`. Note that `bytes_per_cell` must be `64.0` (with a decimal), not `64`, because the parameter type is `f64`.

</details>

<details>
<summary>Solution</summary>

```rust
fn table_size_bytes(rows: i32, columns: i32, bytes_per_cell: f64) -> f64 {
    rows as f64 * columns as f64 * bytes_per_cell
}

fn main() {
    let size = table_size_bytes(1000, 5, 64.0);
    println!("users table — Size: {} bytes", size);
}
```

A few things to notice:

- The function is defined *outside* of `main`. It is a separate block of code.
- The body has no semicolon after the multiplication — this makes it the return value.
- The function takes `i32` values for rows and columns (because they are whole numbers) and `f64` for bytes_per_cell (because it can have decimals). Inside the function, we convert the integers to `f64` before multiplying.

Try changing the values and running again. What is the size for 10000 rows, 8 columns, 128 bytes per cell?

</details>

---

### Exercise 3: Control Flow

**Goal:** Write a function that classifies a table based on its row count.

**Instructions:**

1. Continue in the same project (or create `cargo new table_classifier`)
2. Write a function `fn classify_table(row_count: i32) -> &'static str` that returns:
   - `"Empty"` if row_count is 0
   - `"Small"` if row_count is between 1 and 1000 (inclusive)
   - `"Medium"` if row_count is between 1001 and 100000
   - `"Large"` if row_count is greater than 100000
3. In `main`, test it with four different row counts and print the results:
   - 0 (should print "Empty")
   - 42 (should print "Small")
   - 50000 (should print "Medium")
   - 1000000 (should print "Large")

Expected output:

```text
Rows: 0 — Empty
Rows: 42 — Small
Rows: 50000 — Medium
Rows: 1000000 — Large
```

<details>
<summary>Hint 1</summary>

The function uses `if/else if/else`:

```rust
fn classify_table(row_count: i32) -> &'static str {
    if row_count > 100000 {
        // return "Large"
    } else if ... {
        // ...
    } else {
        // ...
    }
}
```

The return value is the string in each branch — no semicolon, no `return` keyword needed.

</details>

<details>
<summary>Hint 2</summary>

The order of conditions matters. Check `> 100000` first, then `> 1000`, then `> 0`, then the `else` catches zero.

</details>

<details>
<summary>Hint 3</summary>

Print each result like this:

```rust
println!("Rows: {} — {}", 0, classify_table(0));
```

You can call a function directly inside `println!`.

</details>

<details>
<summary>Solution</summary>

```rust
fn classify_table(row_count: i32) -> &'static str {
    if row_count > 100000 {
        "Large"
    } else if row_count > 1000 {
        "Medium"
    } else if row_count > 0 {
        "Small"
    } else {
        "Empty"
    }
}

fn main() {
    println!("Rows: {} — {}", 0, classify_table(0));
    println!("Rows: {} — {}", 42, classify_table(42));
    println!("Rows: {} — {}", 50000, classify_table(50000));
    println!("Rows: {} — {}", 1000000, classify_table(1000000));
}
```

Notice that the strings `"Large"`, `"Medium"`, `"Small"`, and `"Empty"` have no semicolons — they are the return values of the function. Each `if/else` branch is an expression that evaluates to a value. Rust uses the value of whichever branch is true as the function's return value.

Try adding a fifth classification: "Massive" for row counts above 10 million. Where would you add it?

</details>

---

### Exercise 4: Loops + Everything Together

**Goal:** Write a complete program that simulates inserting rows into a table, prints each insert, calculates running statistics, and classifies the table at the end.

This is the final exercise of Part 0 — it combines variables, types, functions, if/else, and for loops into one program.

**Instructions:**

1. Create a new project: `cargo new table_builder`
2. Write two functions:
   - `fn table_size_bytes(rows: i32, columns: i32, bytes_per_cell: f64) -> f64`
   - `fn classify_table(row_count: i32) -> &'static str`
3. In `main`:
   - Set variables: table_name = "users", columns = 5, bytes_per_cell = 64.0, rows_to_insert = 5
   - Print a header with the table name
   - Create a mutable variable `total_rows` starting at 0
   - Use a `for` loop over `1..=rows_to_insert` to:
     - Increment `total_rows`
     - Calculate current table size: `total_rows as f64 * columns as f64 * bytes_per_cell`
     - Print: `"  Inserted row X — total: Y rows, Z bytes"` where X is the row number
   - After the loop, print the final size using `table_size_bytes`
   - Call `classify_table` and print the classification

Expected output:

```text
=== Building table: users ===
  Inserted row 1 — total: 1 rows, 320 bytes
  Inserted row 2 — total: 2 rows, 640 bytes
  Inserted row 3 — total: 3 rows, 960 bytes
  Inserted row 4 — total: 4 rows, 1280 bytes
  Inserted row 5 — total: 5 rows, 1600 bytes
==============================
Total rows: 5
Total size: 1600 bytes
Classification: Small
```

<details>
<summary>Hint 1</summary>

Start with the skeleton:

```rust
fn table_size_bytes(...) -> f64 {
    // ...
}

fn classify_table(...) -> &'static str {
    // ...
}

fn main() {
    let table_name = "users";
    let columns = 5;
    let bytes_per_cell = 64.0;
    let rows_to_insert = 5;

    // Header
    println!("=== Building table: {} ===", table_name);

    // Loop
    let mut total_rows = 0;
    for row_number in 1..=rows_to_insert {
        // ... fill this in
    }

    // Summary
    // ... fill this in
}
```

</details>

<details>
<summary>Hint 2</summary>

Inside the loop, increment the row count and calculate the current size:

```rust
total_rows += 1;
let current_size = total_rows as f64 * columns as f64 * bytes_per_cell;
```

Then print the insert information:

```rust
println!("  Inserted row {} — total: {} rows, {} bytes",
    row_number, total_rows, current_size);
```

</details>

<details>
<summary>Hint 3</summary>

After the loop, use `table_size_bytes` for the final size, and `classify_table` for the classification:

```rust
let final_size = table_size_bytes(total_rows, columns, bytes_per_cell);
let classification = classify_table(total_rows);
println!("Total rows: {}", total_rows);
println!("Total size: {} bytes", final_size);
println!("Classification: {}", classification);
```

</details>

<details>
<summary>Solution</summary>

```rust
fn table_size_bytes(rows: i32, columns: i32, bytes_per_cell: f64) -> f64 {
    rows as f64 * columns as f64 * bytes_per_cell
}

fn classify_table(row_count: i32) -> &'static str {
    if row_count > 100000 {
        "Large"
    } else if row_count > 1000 {
        "Medium"
    } else if row_count > 0 {
        "Small"
    } else {
        "Empty"
    }
}

fn main() {
    // Input
    let table_name = "users";
    let columns = 5;
    let bytes_per_cell = 64.0;
    let rows_to_insert = 5;

    // Header
    println!("=== Building table: {} ===", table_name);

    // Loop through each insert
    let mut total_rows = 0;
    for row_number in 1..=rows_to_insert {
        total_rows += 1;
        let current_size = total_rows as f64 * columns as f64 * bytes_per_cell;
        println!("  Inserted row {} — total: {} rows, {} bytes",
            row_number, total_rows, current_size);
    }

    // Summary
    println!("==============================");
    let final_size = table_size_bytes(total_rows, columns, bytes_per_cell);
    let classification = classify_table(total_rows);
    println!("Total rows: {}", total_rows);
    println!("Total size: {} bytes", final_size);
    println!("Classification: {}", classification);
}
```

Run it with `cargo run`:

```text
=== Building table: users ===
  Inserted row 1 — total: 1 rows, 320 bytes
  Inserted row 2 — total: 2 rows, 640 bytes
  Inserted row 3 — total: 3 rows, 960 bytes
  Inserted row 4 — total: 4 rows, 1280 bytes
  Inserted row 5 — total: 5 rows, 1600 bytes
==============================
Total rows: 5
Total size: 1600 bytes
Classification: Small
```

**Bonus challenge:** Try changing `rows_to_insert` to 2000. What classification does the table get? Now try 200000 — does it become "Large"?

You now have a real program that does real computation. Every line of it uses something you learned in this chapter.

</details>

---

### Exercise 5: Build a Mini Key-Value Store

**Goal:** Combine `Vec`, loops, and functions to build a tiny key-value store that stores entries and looks them up.

This exercise previews the core idea behind ToyDB: storing and retrieving data.

**Instructions:**

1. Create a new project: `cargo new mini_kvstore`
2. In `main`:
   - Create two mutable vectors: `keys` (for `&str` values) and `values` (for `&str` values)
   - Push three key-value pairs: ("name", "Alice"), ("age", "30"), ("city", "Tokyo")
   - Print all entries using a `for` loop
   - Print the total number of entries
   - Write a lookup: loop through `keys` and find the value for `"city"`

Expected output:

```text
=== Mini Key-Value Store ===
  name = Alice
  age = 30
  city = Tokyo
Entries: 3
Lookup 'city': Tokyo
```

<details>
<summary>Hint 1</summary>

Create the vectors like this:

```rust
let mut keys: Vec<&str> = Vec::new();
let mut values: Vec<&str> = Vec::new();

keys.push("name");
values.push("Alice");
```

</details>

<details>
<summary>Hint 2</summary>

Loop through entries using an index:

```rust
for i in 0..keys.len() {
    println!("  {} = {}", keys[i], values[i]);
}
```

Note: `0..keys.len()` starts at 0 because vector indices start at 0 in Rust.

</details>

<details>
<summary>Hint 3</summary>

For the lookup, loop through and check each key:

```rust
let search_key = "city";
for i in 0..keys.len() {
    if keys[i] == search_key {
        println!("Lookup '{}': {}", search_key, values[i]);
    }
}
```

</details>

<details>
<summary>Solution</summary>

```rust
fn main() {
    let mut keys: Vec<&str> = Vec::new();
    let mut values: Vec<&str> = Vec::new();

    // Insert entries
    keys.push("name");
    values.push("Alice");

    keys.push("age");
    values.push("30");

    keys.push("city");
    values.push("Tokyo");

    // Print all entries
    println!("=== Mini Key-Value Store ===");
    for i in 0..keys.len() {
        println!("  {} = {}", keys[i], values[i]);
    }

    // Statistics
    println!("Entries: {}", keys.len());

    // Lookup
    let search_key = "city";
    for i in 0..keys.len() {
        if keys[i] == search_key {
            println!("Lookup '{}': {}", search_key, values[i]);
        }
    }
}
```

Run it with `cargo run`:

```text
=== Mini Key-Value Store ===
  name = Alice
  age = 30
  city = Tokyo
Entries: 3
Lookup 'city': Tokyo
```

This is the essence of a database: store data, list data, find data. The version we just built is extremely simple — it only searches by looping through every entry (called a "linear scan"). Real databases use smarter strategies (indexes, hash maps, B-trees) to make lookups fast even with millions of rows. You will learn all of those techniques as we build ToyDB.

**Bonus challenge:** What happens if you search for a key that does not exist, like `"phone"`? The current program prints nothing. Can you add an `else` case that prints `"Key not found"` after the loop? (Hint: you will need a `bool` variable to track whether the key was found.)

</details>

---

## What You've Learned

This was the most important chapter of Part 0. Here is everything you now know:

- **Variables** store data. Use `let` for immutable values (the default) and `let mut` when a value needs to change.
- **Types** tell Rust what kind of data a variable holds: `i32` for whole numbers, `f64` for decimals, `bool` for true/false, `&str` and `String` for text.
- **Functions** let you write logic once and use it anywhere. They take parameters (inputs) and return values (outputs). The last expression without a semicolon is the return value.
- **`if/else`** lets your program make decisions based on conditions.
- **`match`** provides pattern matching — a clean way to handle multiple cases.
- **`for` loops** repeat a block of code for each value in a range. Combined with mutable variables, they can accumulate results (like running totals).
- **`while` loops** repeat as long as a condition is true.
- **`loop`** runs forever until you `break` out of it.
- **`Vec`** is a growable list for storing multiple values.
- **`String`** is growable text that your program can build up piece by piece.

You can now write programs that store data, calculate results, make decisions, and repeat actions. Those four capabilities are the foundation of every program ever written — including every database.

---

## What's Next

Part 0 is complete. You have gone from "what is a terminal?" to writing a working Rust program with functions, loops, conditional logic, and a mini key-value store. That is a serious accomplishment.

In [Chapter 1: Hello, ToyDB!](../beginner/ch01-hello-toydb.md), we begin building the actual ToyDB database engine. You will set up the project structure, create a REPL that accepts commands, and start implementing the storage layer that will hold real data.

The concepts from Part 0 — variables, functions, types, loops, if/else, Vec — will appear on every single page of the rest of this book. You now have the vocabulary to read and write Rust. It is time to build something real.
