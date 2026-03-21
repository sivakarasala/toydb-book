# Your First Program — What Is Code?

In the last chapter, you set up your workshop: a terminal, a code editor, and a workspace folder. Now it is time to use them.

By the end of this chapter, you will have installed the Rust programming language, created your first project, and run a program that you wrote. You will also understand — at a real level, not a hand-wavy one — what happened when you ran it.

---

## What Is a Programming Language?

You speak English (or another human language) to communicate with people. But computers do not understand English. Deep down, a computer only understands **binary** — long strings of ones and zeros, like `01001000 01101001`. That is how a computer says "Hi."

Nobody wants to write in ones and zeros. So people invented **programming languages** — languages that humans can read and write, which get translated into binary for the computer. A programming language is the meeting point between your brain and the machine.

Here is how that translation works:

```
You write code          The compiler translates it          The computer runs it
(human-readable)   -->  (machine-readable binary)     -->  (things happen!)
```

There are hundreds of programming languages, each with different strengths. Python is popular for data science. JavaScript runs in web browsers. Swift is used for iPhone apps.

We are going to learn **Rust**.

---

## Why Rust?

You might wonder: if there are hundreds of programming languages, why learn Rust?

Three reasons:

1. **It is fast.** Rust programs run about as fast as programs written in C or C++, which are the languages used to build operating systems, game engines, and web browsers. When we build ToyDB, queries will execute quickly because the language itself adds almost no overhead.

2. **It catches your mistakes early.** Rust has a **compiler** (more on this soon) that checks your code before it runs. If you made a mistake, Rust tells you *before* your program crashes — not after. Many languages let bugs sneak through and only fail when a real user is affected. Rust does not.

3. **It teaches you to think clearly.** Rust requires you to be precise about what your code does. This can feel strict at first, but it builds habits that make you a better programmer in *any* language. Building a database demands this kind of precision — every byte matters when you are storing and retrieving data.

You do not need to understand all of that right now. What matters is this: Rust is a language that respects your time. It works hard to help you write correct programs from the start.

---

## Installing Rust

Let's install Rust on your computer. Open your terminal and run this command:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

That is a long command, so let's break it down:

- **`curl`** is a tool that downloads things from the internet.
- The long URL (`https://sh.rustup.rs`) is a script written by the Rust team that installs Rust.
- The `|` symbol (called a **pipe**) takes the downloaded script and feeds it to `sh`, which runs it.

In short: *"Download the Rust installer and run it."*

When the installer starts, it will ask you a question:

```
1) Proceed with standard installation (default - just press enter)
2) Customize installation
3) Cancel installation
```

Press **Enter** to choose the default. The installer will download and set up everything you need. This may take a minute or two.

When it finishes, you will see:

```
Rust is installed now. Great!
```

Now, close your terminal and open a new one. (This is necessary so your terminal picks up the new Rust tools.) Then verify the installation:

```bash
rustc --version
```

You should see something like:

```
rustc 1.83.0 (90b35a623 2024-11-26)
```

The exact numbers will differ — that is fine. What matters is that you see a version number, not an error.

Also try:

```bash
cargo --version
```

```
cargo 1.83.0 (5ffbef321 2024-10-29)
```

**`rustc`** is the Rust **compiler** — the program that translates your code into something the computer can run. **`cargo`** is Rust's **build tool and package manager** — we will talk about it more at the end of this chapter. For now, just know that if both commands showed version numbers, Rust is installed and ready.

---

## Your First Program

Here is the moment. Let's create and run a Rust program.

In your terminal, navigate to your workspace:

```bash
cd ~/rusty
```

Now use Cargo to create a new project:

```bash
cargo new hello_toydb
```

You should see:

```
    Creating binary (application) `hello_toydb` package
```

Cargo just created a folder called `hello_toydb` with everything you need to get started. Let's go inside:

```bash
cd hello_toydb
```

Now, run the program:

```bash
cargo run
```

You will see some output as Rust compiles your code, and then:

```
   Compiling hello_toydb v0.1.0 (/Users/yourname/rusty/hello_toydb)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.73s
     Running `target/debug/hello_toydb`
Hello, world!
```

There it is. **Hello, world!** Your first program just ran.

You did not write the code yet — Cargo generated a starter program for you. But it is *your* project now, and in a moment you are going to change it. First, let's understand what Cargo created.

---

## Anatomy of a Rust Program

Open your project in VS Code:

```bash
code ~/rusty/hello_toydb
```

In the left sidebar, you will see this file structure:

```
hello_toydb/
├── Cargo.toml
└── src/
    └── main.rs
```

There are two important files. Let's start with the one that matters most.

### `src/main.rs`

Click on `src/main.rs` in VS Code. You will see:

```rust
fn main() {
    println!("Hello, world!");
}
```

Three lines. That is the entire program. Let's go through it piece by piece.

---

**`fn main() {`**

- **`fn`** is short for **function**. A function is a named block of instructions. Think of it like a chapter in a recipe: "Chapter: Make the Sauce" groups together all the sauce-making steps.
- **`main`** is the name of this function. The name `main` is special in Rust — it is the **entry point** of your program. When you run a Rust program, the computer starts by looking for a function called `main` and runs whatever is inside it.
- **`()`** after the name is where you would list any inputs the function needs. `main` does not need any inputs, so the parentheses are empty.
- **`{`** is an opening curly brace. It marks the **beginning** of the function's body — the instructions inside it.

---

**`println!("Hello, world!");`**

- **`println!`** is a **macro** that prints text to the terminal. (A macro is like a special function — the `!` at the end is how you know it is a macro, not a regular function. Don't worry about the difference yet.)
- **`"Hello, world!"`** is a **string** — a piece of text. The double quotes tell Rust *"this is text, not code."*
- **`;`** is a **semicolon**. In Rust, most lines of code end with a semicolon. It is like the period at the end of a sentence — it tells Rust *"this instruction is complete."*

---

**`}`**

The closing curly brace marks the **end** of the `main` function. Everything between `{` and `}` is the body of the function — the instructions that run when the function is called.

So the whole program says: *"When this program starts, print the text 'Hello, world!' to the terminal."*

---

## Making It Yours

Let's change the program. Open `src/main.rs` and replace the contents with:

```rust
fn main() {
    println!("Welcome to ToyDB!");
    println!("A database built from scratch, in Rust.");
}
```

Save the file (`Cmd + S` on macOS, `Ctrl + S` on Windows/Linux), then run it again:

```bash
cargo run
```

```
   Compiling hello_toydb v0.1.0 (/Users/yourname/rusty/hello_toydb)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.31s
     Running `target/debug/hello_toydb`
Welcome to ToyDB!
A database built from scratch, in Rust.
```

You just wrote your first Rust program. Two lines of output, each produced by a `println!` statement. Every line of code you write from now on will build on this foundation.

---

## What the Compiler Does

When you type `cargo run`, two things happen behind the scenes:

1. **Compilation:** Rust reads your `.rs` file and translates it into machine code — the binary instructions your computer's processor understands. This step is called **compiling**, and the program that does it is called the **compiler** (`rustc`).

2. **Execution:** Rust runs the compiled program. Your computer executes the machine code, and you see the output in your terminal.

Why not skip the compilation step and just run the code directly? Some languages (like Python and JavaScript) do this — they *interpret* the code line by line. Rust compiles first because:

- **Speed:** Compiled code runs much faster. The compiler has time to optimize your program before it runs.
- **Safety:** The compiler can check your *entire* program for errors before a single line executes. If it finds a mistake, it stops and tells you — nothing runs until the error is fixed.

Think of it like a spellchecker that reads your entire essay before you submit it, versus one that only catches typos as you type each word.

---

## Comments

Sometimes you want to leave a note in your code — a reminder to yourself, or an explanation for someone reading it later. These notes are called **comments**. The compiler ignores them completely.

In Rust, comments start with `//`:

```rust
fn main() {
    // This program introduces ToyDB
    println!("Welcome to ToyDB!");

    // We will build a real database from scratch
    println!("A database built from scratch, in Rust.");
}
```

The `//` tells Rust: *"Everything after this on the same line is a comment. Skip it."*

Comments do not change what the program does. The output is exactly the same whether the comments are there or not. But they make your code easier to understand — especially when you come back to it a week later and have forgotten what you were thinking.

A good rule: do not comment *what* the code does (the code itself shows that). Comment *why*:

```rust
// Print each field on its own line so the output is easy to scan
println!("Table: users");
println!("Rows: 42");
println!("Columns: id, name, email");
```

---

## The `Cargo.toml` File

Open `Cargo.toml` in VS Code. You will see something like this:

```toml
[package]
name = "hello_toydb"
version = "0.1.0"
edition = "2021"

[dependencies]
```

This is your project's **configuration file**. It tells Cargo:

| Field | Meaning |
|-------|---------|
| `name` | The name of your project |
| `version` | The current version number (we will leave this at `0.1.0` for a long time) |
| `edition` | Which edition of Rust to use (`2021` is the current standard) |
| `[dependencies]` | A list of external libraries your project uses (empty for now) |

You do not need to edit this file yet. Just know it exists and what it is for. When we start using external libraries to help build ToyDB, we will add them under `[dependencies]`.

---

## The Build Cycle

Here is the workflow you will follow for the rest of this book:

1. **Edit** your code in VS Code
2. **Save** the file
3. **Run** `cargo run` in the terminal
4. **Read** the output (or the error message)
5. **Repeat**

This cycle — edit, save, run, read — is the heartbeat of programming. You will do it hundreds of times per chapter. It becomes muscle memory very quickly.

> **Tip:** VS Code has a built-in terminal. Press `` Ctrl + ` `` (backtick) to toggle it. This way you can edit code and run it without switching windows.

---

## Exercises

### Exercise 1: Install and Verify Rust

**Goal:** Install Rust and confirm it is working.

**Instructions:**
1. Open your terminal.
2. Run the Rust installer: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
3. Press Enter to accept the default installation.
4. Close your terminal and open a new one.
5. Run `rustc --version` and confirm you see a version number.
6. Run `cargo --version` and confirm you see a version number.

<details>
<summary>Hints</summary>

- If `rustc --version` gives an error like "command not found," try closing and reopening your terminal. The installation adds Rust to your PATH, but only new terminal sessions pick it up.
- On macOS or Linux, you can also run `source $HOME/.cargo/env` to load Rust into your current session without restarting the terminal.

</details>

**What you should see:**

Two version numbers, something like:

```
rustc 1.83.0 (90b35a623 2024-11-26)
cargo 1.83.0 (5ffbef321 2024-10-29)
```

If you see these, Rust is installed and ready.

---

### Exercise 2: Create and Run Your First Project

**Goal:** Create a new Rust project, modify it, and run it.

**Instructions:**
1. Navigate to your workspace: `cd ~/rusty`
2. Create a new project: `cargo new hello_toydb`
3. Move into the project: `cd hello_toydb`
4. Run the default program: `cargo run`
5. Open the project in VS Code: `code .`
6. Edit `src/main.rs` to print:
   ```
   Welcome to ToyDB!
   A database built from scratch, in Rust.
   ```
7. Save and run `cargo run` again.

<details>
<summary>Hints</summary>

- If you get `error: destination ... already exists`, you may have already created this project. Either delete it (`rm -rf hello_toydb`) and try again, or use a different name like `hello_toydb2`.
- You need two `println!` statements — one for each line of output.
- Remember the semicolons at the end of each `println!` line.

</details>

**What you should see:**

After step 7:

```
Welcome to ToyDB!
A database built from scratch, in Rust.
```

---

### Exercise 3: Make It Your Own

**Goal:** Write a program that prints a database status report.

**Instructions:**
1. Edit `src/main.rs` to produce this exact output:

```
=== ToyDB Status ===
Version: 0.1.0
Tables: 0
Rows: 0
Status: Ready
=====================
```

2. Run it with `cargo run` and confirm the output matches.

<details>
<summary>Hints</summary>

- You need six `println!` statements.
- The `===` lines are just decoration — count the characters if you want them to line up, but it does not need to be perfect.
- Each `println!` statement follows the same pattern: `println!("text here");`

</details>

<details>
<summary>Solution</summary>

```rust
fn main() {
    println!("=== ToyDB Status ===");
    println!("Version: 0.1.0");
    println!("Tables: 0");
    println!("Rows: 0");
    println!("Status: Ready");
    println!("=====================");
}
```

Run it:

```bash
cargo run
```

Expected output:

```
=== ToyDB Status ===
Version: 0.1.0
Tables: 0
Rows: 0
Status: Ready
=====================
```

</details>

---

### Exercise 4: Comments Practice

**Goal:** Add comments to your program explaining what each section does.

**Instructions:**
1. Take the program from Exercise 3.
2. Add a comment above the header line explaining it is the header.
3. Add a comment above the status fields explaining they show database statistics.
4. Add a comment above the closing line explaining it is the footer.
5. Run `cargo run` and confirm the output is unchanged.

<details>
<summary>Hints</summary>

- Comments start with `//` and continue to the end of the line.
- Comments do not affect the output at all.
- Place each comment on the line *above* the code it describes.

</details>

<details>
<summary>Solution</summary>

```rust
fn main() {
    // Header
    println!("=== ToyDB Status ===");

    // Database statistics
    println!("Version: 0.1.0");
    println!("Tables: 0");
    println!("Rows: 0");
    println!("Status: Ready");

    // Footer
    println!("=====================");
}
```

The output is exactly the same as before. The compiler skips every line that starts with `//`.

</details>

---

## What You've Learned

| Concept | What it means |
|---------|--------------|
| **Programming language** | A human-readable language that gets translated into machine code |
| **Compiler** (`rustc`) | The program that translates Rust code into binary |
| **Cargo** | Rust's build tool — creates projects, compiles code, runs programs |
| **`cargo new`** | Creates a new Rust project with the standard file structure |
| **`cargo run`** | Compiles and runs your program |
| **`fn main()`** | The entry point of every Rust program |
| **`println!`** | A macro that prints text to the terminal |
| **String** (`"text"`) | Text enclosed in double quotes |
| **Semicolon** (`;`) | Marks the end of a statement |
| **Comments** (`//`) | Notes in your code that the compiler ignores |
| **`Cargo.toml`** | Your project's configuration file |

You now have Rust installed, you know how to create and run projects, and you understand every piece of a basic Rust program. In the next chapter, we will step back from code and learn how to *think* about problems the way a programmer does.
