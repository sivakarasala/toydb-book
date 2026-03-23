# Learn Rust by Building a Database

A hands-on guide to **Rust**, **DSA**, and **System Design** — build a distributed SQL database from scratch.

## What You'll Build

A distributed SQL database with:
- Pluggable storage engines (in-memory + BitCask)
- MVCC transaction isolation
- SQL lexer, parser, and AST
- Query planner and optimizer
- Iterator-based query executor (Volcano model)
- Client-server protocol with TCP
- Raft consensus for distributed replication
- Leader election and log replication

By the end, you'll have a working database you can interact with:

```sql
toydb> CREATE TABLE users (id INT, name TEXT, age INT)
toydb> INSERT INTO users VALUES (1, 'Alice', 30)
toydb> SELECT name, age FROM users WHERE age > 28 ORDER BY age DESC
```

## Getting Started

### Prerequisites

- **Rust** — Install from [rustup.rs](https://rustup.rs/)
- **mdBook** — For reading the book locally
- **DDIA** (optional but recommended) — *Designing Data-Intensive Applications* by Martin Kleppmann. The book includes a [companion reading guide](src/ddia-companion.md) that maps each chapter to specific DDIA sections.

### Step 1: Clone and open the book

```bash
git clone https://github.com/sivakarasala/toydb-book.git
cd toydb-book
cargo install mdbook
mdbook serve --open
```

The book opens in your browser. Pick your track:
- **Never coded before?** → Start at Part 0: Programming Fundamentals
- **Know another language?** → Start at Chapter 1: What Is a Database?

### Step 2: Try the finished database

Before building anything, see what you're working toward:

```bash
cd code/toydb
cargo run
```

```sql
toydb> CREATE TABLE users (id INT, name TEXT, age INT)
toydb> INSERT INTO users VALUES (1, 'Alice', 30)
toydb> INSERT INTO users VALUES (2, 'Bob', 25)
toydb> INSERT INTO users VALUES (3, 'Charlie', 35)
toydb> SELECT name, age FROM users WHERE age > 28 ORDER BY age DESC
toydb> SELECT COUNT(*) FROM users
toydb> .quit
```

This is the reference implementation — the finished product after all 18 chapters.

### Step 3: Work through chapter exercises

Each chapter has an exercise crate with `todo!()` stubs and tests:

```bash
cd code/ch01

# Run the tests (they will fail — that's expected)
cargo test --bin exercise

# Open src/main.rs and fill in the todo!() stubs
# Run tests again until they pass
cargo test --bin exercise

# Check your work against the solution
cargo test --bin solution
```

Repeat for ch02, ch03, ... ch18.

### Step 4: Build your own database (the cumulative project)

After each chapter exercise, add that layer to your own toydb:

```bash
# After Ch1-2: Create your project
cargo new my-toydb
cd my-toydb

# Add src/storage/mod.rs (Storage trait)
# Add src/storage/memory.rs (BTreeMap engine)
cargo build  # Should compile!
```

The [code/README.md](code/README.md) has a complete table showing which files to add after each chapter.

### Step 5: Read DDIA alongside (optional)

After each chapter, read the corresponding DDIA sections. The [companion guide](src/ddia-companion.md) has exact page numbers:

| After toydb Chapter | Read in DDIA | Pages |
|:---:|:---|:---:|
| 1-2 | Ch 1 (all) + Ch 3 (pp. 69-83) | ~35 |
| 3 | Ch 3 (pp. 72-76, 83-85) | ~10 |
| 5 | Ch 7 (pp. 221-242) | ~20 |
| 14-15 | Ch 5 (pp. 151-175) + Ch 9 (pp. 354-375) | ~45 |

### Step 6: Tackle the capstone

After all 18 chapters, test your skills:

```bash
cd code/capstone

# 8 DSA coding challenges
cargo test --bin c1-exercise   # KV Range Query
cargo test --bin c4-exercise   # Transaction Scheduler (topological sort)
cargo test --bin c7-exercise   # Deadlock Detector (cycle detection)
```

## The Complete Workflow

```
For each chapter (1-18):
  1. Read the chapter in the book
  2. Do the exercise:     cd code/chXX && cargo test --bin exercise
  3. Build your toydb:    add that layer to your cumulative project
  4. Read DDIA:           companion guide has exact sections + pages
  5. DS Deep Dive:        read the linked narrative for that chapter
```

## Two Learning Tracks

| Track | For | Starts At |
|-------|-----|-----------|
| **Beginner** | Never coded before | Part 0: Programming Fundamentals |
| **Experienced** | Programmers learning Rust | Chapter 1: What Is a Database? |

Both tracks build the same database and converge at the capstone chapters.

## Structure

```
toydb-book/
├── src/                          ← Book content (mdBook)
│   ├── part-0-foundations/       ← Beginner: programming fundamentals
│   ├── beginner/                 ← Beginner track (Ch 1-18)
│   ├── experienced/              ← Experienced track (Ch 1-18)
│   ├── ds-narratives/            ← 16 DS Deep Dives (build data structures from scratch)
│   ├── ddia-companion.md         ← DDIA reading guide with page numbers
│   ├── ch18-5-design-reflection.md ← Ousterhout's design philosophy applied
│   └── capstone/                 ← Coding challenges, system design, mock interviews
│
├── code/                         ← Runnable Rust code
│   ├── ch01/ ... ch18/           ← Exercise + solution crates (per chapter)
│   ├── capstone/                 ← 8 DSA challenges (exercise + solution)
│   ├── toydb/                    ← Complete reference database (REPL + all layers)
│   └── README.md                 ← How to work through the code
```

## Reference

Inspired by Erik Grinaker's [toydb](https://github.com/erikgrinaker/toydb) — an educational distributed SQL database.

## License

MIT
