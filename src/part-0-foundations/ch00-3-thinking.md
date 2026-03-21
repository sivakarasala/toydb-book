# Thinking Like a Programmer

In Chapter 0.2, you wrote your first Rust program and watched the compiler turn your code into something the computer could run. You learned `fn main()`, `println!`, and the build-run cycle.

But writing code is only half of programming. The other half — the harder half — is **thinking**. Before you type a single character, you need to know *what* to tell the computer to do and *in what order*.

This chapter teaches you that thinking process. By the end, you will be able to:

- Break any problem into a sequence of small steps
- Recognize the Input-Process-Output pattern that every program follows
- Write pseudocode (a plan in plain English) before writing Rust
- Read Rust compiler error messages without panicking
- Debug methodically instead of guessing randomly

No new Rust syntax in this chapter — just three short programs and a lot of thinking practice. This is the chapter that separates people who *memorize code* from people who *understand programming*.

---

## Breaking Problems into Steps

Imagine you are explaining to a friend how to look up a phone number in a contacts list. They have never used a phone before. You would not say "just find the number." You would give them **specific steps in a specific order**:

1. Open the contacts list
2. Look through the names from the top
3. Stop when you find the name you want
4. Read the phone number next to that name
5. Write it down

That list is a **program**. Each line is an **instruction**. The order matters — you cannot read the phone number before you find the name, and you cannot find the name without looking through the list.

Programming works exactly the same way. A computer is your extremely obedient but extremely literal friend. It will do precisely what you say, in the exact order you say it. It will never "figure out what you meant." If you skip a step or put steps in the wrong order, the result will be wrong.

### The database analogy

Let's say we want to build a program that stores and retrieves a person's name. Before writing any code, we need to answer:

- What information do we need? (a key to identify the record, the data to store)
- What do we do with that information? (save it somewhere, look it up later)
- What do we show the user? (the stored value when they ask for it)

These three questions map directly to the most fundamental pattern in all of computing.

---

## Input, Processing, Output

Every program you will ever write follows this pattern:

```
INPUT  →  PROCESSING  →  OUTPUT
```

- **Input**: The data your program starts with. It could come from the user typing on a keyboard, from a file, from a database, or from the internet.
- **Processing**: What your program *does* with the data. Calculations, comparisons, sorting, filtering, transforming.
- **Output**: What your program produces. Text on the screen, a file saved to disk, a web page, a notification.

### Example: Key-value store

Let's trace this pattern for a simple data storage program:

| Stage | What happens | Example |
|-------|-------------|---------|
| **Input** | A key and a value | Key: "name", Value: "Alice" |
| **Processing** | Store the pair in memory | Save "name" → "Alice" |
| **Output** | Confirm the operation | "Stored: name = Alice" |

That is the entire program. Three stages. Every feature of the ToyDB database we will build later — the storage engine, the query parser, the indexing system — is just this pattern repeated and combined in different ways.

### Example: Record lookup

| Stage | What happens | Example |
|-------|-------------|---------|
| **Input** | A key to search for | Key: "name" |
| **Processing** | Search through stored data for the key | Find "name" → "Alice" |
| **Output** | Display the result | "Found: Alice" |

### Example: Row counter

| Stage | What happens | Example |
|-------|-------------|---------|
| **Input** | A collection of records | 5 records in the table |
| **Processing** | Count them | count = 5 |
| **Output** | Display the count | "Table has 5 rows" |

Once you start seeing this pattern, you will see it everywhere — not just in code, but in spreadsheets, recipes, tax forms, and assembly instructions.

---

## Pseudocode: Planning Before Coding

**Pseudocode** is a plan written in plain English (or any human language) that describes what a program should do, step by step. It is not real code. No computer can run it. Its purpose is to help *you* think clearly before you start worrying about syntax.

Here is pseudocode for a simple key-value store:

```text
PROGRAM: Key-Value Store

1. Set the key to "name"
2. Set the value to "Alice"
3. Print that the value has been stored
4. Retrieve the value using the key
5. Print the retrieved value
6. If the value was found, print "Lookup successful!"
7. Otherwise, print "Key not found."
```

Notice a few things about this pseudocode:

- **Each line does one thing.** No line tries to do three things at once.
- **The order matters.** We store the value before we look it up, and we look it up before we print it.
- **It uses plain words.** "Set", "Print", "Retrieve", "If." No curly braces, no semicolons.
- **It is specific.** Not "do some storage" but "set the key to 'name' and the value to 'Alice'."

### Why bother with pseudocode?

When you sit down to write Rust code, you have to think about two things simultaneously:

1. **What** the program should do (the logic)
2. **How** to say it in Rust (the syntax)

That is two hard things at once. Pseudocode lets you solve problem #1 first, on its own, so that when you open your editor, you only have to solve problem #2. One hard thing at a time.

Professional programmers write pseudocode, sketches, and outlines before coding. It is not a beginner crutch — it is a professional practice.

---

## Reading Error Messages

Here is a truth that surprises most beginners: **you will spend more time reading error messages than writing code.** This is normal. This is healthy. Error messages are not failures — they are the compiler *helping you*.

Rust has one of the best compilers in any programming language. It does not just say "something is wrong." It tells you:

- **What** is wrong
- **Where** it went wrong (file name, line number, column number)
- **Why** it is wrong
- Often, **how to fix it**

Let's look at three common errors and learn to read them.

### Error 1: The missing semicolon

Here is a program with a bug:

```rust
fn main() {
    println!("Table: users")
    println!("Rows: 42")
}
```

If you try to compile this with `cargo run`, Rust prints:

```text
error: expected `;`, found `println`
 --> src/main.rs:3:5
  |
2 |     println!("Table: users")
  |                              ^ help: add `;` here
3 |     println!("Rows: 42")
  |     ^^^^^^^ unexpected token
```

Let's decode each part:

| Part | Meaning |
|------|---------|
| `error:` | This is an error (your code will not compile) |
| `expected ';', found 'println'` | Rust expected a semicolon but found the next `println` instead |
| `--> src/main.rs:3:5` | The problem is in file `src/main.rs`, at line 3, column 5 |
| `help: add ';' here` | Rust is literally telling you the fix |

**The fix:** Add a semicolon at the end of line 2:

```rust
fn main() {
    println!("Table: users");
    println!("Rows: 42");
}
```

Every statement in Rust ends with a semicolon. Forget one, and the compiler catches it instantly.

### Error 2: The mismatched type

```rust
fn main() {
    let row_count: i32 = "forty-two";
    println!("Rows: {}", row_count);
}
```

Compiler output:

```text
error[E0308]: mismatched types
 --> src/main.rs:2:26
  |
2 |     let row_count: i32 = "forty-two";
  |                    ---   ^^^^^^^^^^^^ expected `i32`, found `&str`
  |                    |
  |                    expected due to this
```

| Part | Meaning |
|------|---------|
| `error[E0308]` | Error code E0308 — you can search "Rust E0308" for details |
| `mismatched types` | You tried to put the wrong kind of data into a variable |
| `expected 'i32', found '&str'` | The variable expects a number (`i32`) but you gave it text (`"forty-two"`) |
| `expected due to this` | The arrow points to `: i32`, showing *why* Rust expected a number |

**The fix:** Use an actual number, not the word "forty-two":

```rust
fn main() {
    let row_count: i32 = 42;
    println!("Rows: {}", row_count);
}
```

Don't worry about `i32` and `&str` yet — you will learn about types in Chapter 0.4. The point here is that the compiler told you *exactly* what was wrong and *exactly* where.

### Error 3: The undefined variable

```rust
fn main() {
    let table_name = "users";
    println!("Table: {}", tabel_name);
}
```

Compiler output:

```text
error[E0425]: cannot find value `tabel_name` in this scope
 --> src/main.rs:3:27
  |
3 |     println!("Table: {}", tabel_name);
  |                            ^^^^^^^^^^ help: a local variable with a similar name exists: `table_name`
```

| Part | Meaning |
|------|---------|
| `cannot find value 'tabel_name'` | You used a name that does not exist |
| `help: a local variable with a similar name exists: 'table_name'` | Rust noticed your typo and suggested the correct name |

**The fix:** Correct the spelling:

```rust
fn main() {
    let table_name = "users";
    println!("Table: {}", table_name);
}
```

The Rust compiler caught a *typo* and suggested the right word. Most compilers do not do this. Rust's error messages are famously helpful — learn to read them and they become your best debugging tool.

### The error-reading recipe

Every time you see a compiler error, follow these four steps:

1. **Read the first line.** It tells you *what* kind of error (missing semicolon, wrong type, unknown name, etc.)
2. **Look at the file and line number.** Open that file, go to that line.
3. **Read the arrows and highlights.** They show *exactly* which characters are wrong.
4. **Read the `help:` line.** If there is one, it often contains the exact fix.

Do not skip to step 4. Understanding *what* went wrong (steps 1-3) is how you learn. Just applying the suggested fix without understanding teaches you nothing.

---

## The Debugging Mindset

Errors are inevitable. Even programmers with 30 years of experience get compiler errors every single day. The difference between a beginner and an expert is not the number of errors — it is *how they respond to errors*.

### What beginners do (and why it doesn't work)

1. See a red error message
2. Panic
3. Change something random
4. Compile again
5. See a *different* error
6. Panic harder
7. Change something else random
8. Repeat until frustrated

This is called **random debugging**, and it almost never works. Each random change can introduce a *new* bug, so you end up with more errors than you started with.

### What programmers do

1. **Read the error.** The whole thing. Every word.
2. **Isolate the problem.** Which line? Which word on that line? What was Rust expecting versus what it found?
3. **Form a hypothesis.** "I think the problem is a missing semicolon on line 7."
4. **Test the hypothesis.** Make *one* change. Compile again. Did the error go away?
5. **If it did not work,** read the *new* error message. It might be a different problem, or your fix might have been close but not quite right. Go back to step 1.

The key principle: **change one thing at a time.** If you change three things at once and the error goes away, you do not know which change fixed it — and you will not learn anything.

### A real debugging session

Let's say you write this program:

```rust
fn main() {
    println!("ToyDB v0.1.0")
    println!("Status: Ready");
    println!("Tables loaded: 3")
}
```

You run `cargo run` and see an error about a missing semicolon. Following the method:

1. **Read:** "expected `;`, found `println`" on line 3
2. **Isolate:** Line 2 is missing a semicolon at the end
3. **Hypothesis:** Adding `;` after the closing `)` on line 2 will fix it
4. **Test:** Add the semicolon, run `cargo run` again

New error: missing semicolon on line 4. Same problem, same fix. Add it.

Now `cargo run` works:

```text
ToyDB v0.1.0
Status: Ready
Tables loaded: 3
```

Two errors. Two calm fixes. Total time: 30 seconds. No panic required.

---

## Thinking About Databases

Before we move on, let's practice the thinking skills from this chapter on the problem we are going to solve throughout this book: building a database.

What does a database actually *do*? At its core, a database is a program that:

1. **Stores data** — you give it information, and it remembers it
2. **Retrieves data** — you ask for information, and it gives it back
3. **Organizes data** — it keeps data structured so retrieval is fast

That is the Input-Process-Output pattern again:

| Stage | What happens |
|-------|-------------|
| **Input** | A command from the user: "store this" or "find that" |
| **Processing** | Save the data to memory/disk, or search through stored data |
| **Output** | Confirmation of storage, or the data that was found |

Every database in the world — PostgreSQL, MySQL, SQLite, MongoDB — follows this same fundamental pattern. They differ in *how* they process the data (how they organize it, how they make searches fast, how they handle multiple users at once), but the basic shape is always the same.

Here is pseudocode for the simplest possible database:

```text
PROGRAM: Simplest Database

1. Create an empty storage area
2. Store the pair: key = "name", value = "Alice"
3. Store the pair: key = "email", value = "alice@example.com"
4. Print all stored pairs
5. Look up the value for key "name"
6. Print the result
```

That is ToyDB in its most basic form. Over the course of this book, we will make it more sophisticated — adding tables, queries, indexes, and persistence — but it all starts with this simple idea: store data, retrieve data.

---

## Exercises

### Exercise 1: Write Pseudocode for a Record Store

**Goal:** Practice breaking a problem into steps *before* writing any code.

**Instructions:**

Open a text file (or use pen and paper). Write pseudocode — plain English instructions, numbered — for a program that does the following:

1. Stores the name of a database table
2. Stores the number of rows in that table
3. Stores the number of columns in that table
4. Prints a summary line like: "Table 'users' has 42 rows and 5 columns"

Remember the Input-Process-Output pattern:
- What are the **inputs**? (What data does the program need?)
- What is the **processing**? (What does the program do with the data?)
- What is the **output**? (What does the program display?)

You do not need to write any Rust code. Just the plan.

<details>
<summary>Hint 1</summary>

Start by listing the three inputs on separate lines:

```text
1. Set the table name to ...
2. Set the row count to ...
3. Set the column count to ...
```

</details>

<details>
<summary>Hint 2</summary>

The "processing" here is simple — there is no calculation. The program just *combines* the inputs into a sentence. Your pseudocode might just say:

```text
4. Print a summary with the table name, row count, and column count
```

</details>

<details>
<summary>Solution</summary>

```text
PROGRAM: Table Summary

1. Set the table name to "users"
2. Set the number of rows to 42
3. Set the number of columns to 5
4. Print: "Table '[table name]' has [rows] rows and [columns] columns"
```

There is no single correct answer — your pseudocode might use different words. The key things to check:

- Did you identify the three inputs?
- Did you describe the output clearly?
- Are the steps in a logical order (set values before using them)?

</details>

---

### Exercise 2: From Pseudocode to Rust

**Goal:** Turn your pseudocode into a working Rust program using only `println!` (with hardcoded values, since we have not learned variables yet).

**Instructions:**

1. Create a new project: `cargo new table_summary`
2. Open `src/main.rs`
3. Replace the contents with `println!` statements that produce this output:

```text
=== Table Summary ===
Table: users
Rows: 42
Columns: 5
Table 'users' has 42 rows and 5 columns
=====================
```

4. Run it with `cargo run` and confirm the output matches

<details>
<summary>Hint 1</summary>

You need one `println!` statement for each line of output. The `===` lines are just decoration — use `println!("=====================");` or similar.

</details>

<details>
<summary>Hint 2</summary>

Remember that every `println!` statement needs:
- An exclamation mark after `println`
- Parentheses around the text
- Double quotes around the string
- A semicolon at the end

</details>

<details>
<summary>Solution</summary>

```rust
fn main() {
    println!("=== Table Summary ===");
    println!("Table: users");
    println!("Rows: 42");
    println!("Columns: 5");
    println!("Table 'users' has 42 rows and 5 columns");
    println!("=====================");
}
```

Run it:

```bash
cd table_summary
cargo run
```

Expected output:

```text
=== Table Summary ===
Table: users
Rows: 42
Columns: 5
Table 'users' has 42 rows and 5 columns
=====================
```

Yes, the values are hardcoded — we are repeating "users" and "42" in multiple places. That is annoying, and it is exactly *why* we need variables (Chapter 0.4). But for now, this works.

</details>

---

### Exercise 3: Bug Fixing Challenge

**Goal:** Practice reading compiler errors and fixing bugs methodically.

For each broken program below: **read the error message**, **explain what is wrong in your own words**, and **fix the code**. Do not just look at the solution — try to fix it yourself first.

#### Bug 1: The Easy One

```rust
fn main() {
    println!("Table: users")
    println!("Rows: 42");
}
```

Run this with `cargo run`. Read the error. Fix it.

<details>
<summary>Hint</summary>

Look carefully at line 2. What is missing at the end?

</details>

<details>
<summary>Solution</summary>

**The error:**
```text
error: expected `;`, found `println`
 --> src/main.rs:3:5
  |
2 |     println!("Table: users")
  |                              ^ help: add `;` here
```

**What is wrong:** Line 2 is missing a semicolon at the end. In Rust, every statement must end with `;`.

**The fix:**
```rust
fn main() {
    println!("Table: users");
    println!("Rows: 42");
}
```

</details>

---

#### Bug 2: The Spelling One

```rust
fn main() {
    prinltn!("ToyDB v0.1.0");
    println!("Status: Ready");
    println!("Tables: 3");
}
```

Run this with `cargo run`. Read the error. Fix it.

<details>
<summary>Hint</summary>

Read the first line very carefully, character by character. Compare it to the `println!` on the other lines.

</details>

<details>
<summary>Solution</summary>

**The error:**
```text
error: cannot find macro `prinltn` in this scope
 --> src/main.rs:2:5
  |
2 |     prinltn!("ToyDB v0.1.0");
  |     ^^^^^^^ help: a macro with a similar name exists: `println`
```

**What is wrong:** `prinltn!` is misspelled. The letters "l" and "t" are swapped. It should be `println!`.

**The fix:**
```rust
fn main() {
    println!("ToyDB v0.1.0");
    println!("Status: Ready");
    println!("Tables: 3");
}
```

Rust even suggested the correct name. Always read the `help:` line.

</details>

---

#### Bug 3: The Tricky One

```rust
fn main() {
    println!("=== Query Result ===");
    println!("SELECT * FROM users");
    println!("Found 3 rows matching "name = Alice"");
    println!("Query time: 0.02s");
}
```

Run this with `cargo run`. This one produces a more confusing error. Read it carefully.

<details>
<summary>Hint 1</summary>

Look at line 4. There are `"` characters inside the string that Rust thinks are the *end* of the string. The problem is the quotes around `name = Alice`.

</details>

<details>
<summary>Hint 2</summary>

To include a double-quote character inside a string, you need to *escape* it with a backslash: `\"`. So `"name = Alice"` inside a string becomes `\"name = Alice\"`.

</details>

<details>
<summary>Solution</summary>

**The error:**
```text
error: expected `,`, found `name`
 --> src/main.rs:4:42
  |
4 |     println!("Found 3 rows matching "name = Alice"");
  |                                       ^^^^ expected `,`
```

**What is wrong:** The `"` before `name` ends the string early. Rust then sees `name = Alice` as code, not text. The double-quotes that are meant to surround the condition are being interpreted as string delimiters.

**The fix:** Escape the inner double-quotes with backslashes:

```rust
fn main() {
    println!("=== Query Result ===");
    println!("SELECT * FROM users");
    println!("Found 3 rows matching \"name = Alice\"");
    println!("Query time: 0.02s");
}
```

Alternatively, you could avoid the issue by using single quotes in the output:

```rust
    println!("Found 3 rows matching 'name = Alice'");
```

This is a common real-world bug. Whenever you need to put special characters inside a string — like `"`, `\`, or `{` — you may need to escape them. You will learn more about this in later chapters.

</details>

---

## What You've Learned

- **Breaking problems into steps** is the first and most important programming skill. Before you write code, you need a plan.
- **Every program is Input, Processing, Output.** Once you identify these three parts, the structure of the program becomes clear.
- **Pseudocode** lets you think about *what* the program should do without worrying about syntax. Write the plan first, then translate to code.
- **Rust's compiler errors are your friend.** They tell you what is wrong, where it is wrong, and often how to fix it. Read the whole message.
- **Debug methodically.** Read the error, isolate the problem, form a hypothesis, test one change at a time. Never guess randomly.

---

## What's Next

You now know how to *think* about programs. In [Chapter 0.4: The Building Blocks](ch00-4-building-blocks.md), you will learn the core Rust tools that turn your thinking into real, working code: **variables** to store data, **functions** to organize logic, and **loops** to repeat actions. By the end of that chapter, you will write a tiny in-memory key-value store from scratch — your first real program that actually *stores and retrieves data*.
