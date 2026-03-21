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

## Two Learning Tracks

| Track | For | Starts At |
|-------|-----|-----------|
| **Beginner** | Never coded before | Part 0: Programming Fundamentals |
| **Experienced** | Programmers learning Rust | Chapter 1: What Is a Database? |

Both tracks build the same database and converge at the capstone chapters.

## Reading the Book

```bash
cargo install mdbook
mdbook serve --open
```

## Structure

- `src/part-0-foundations/` — Programming fundamentals (beginner track only)
- `src/beginner/` — Beginner track chapters 1-18
- `src/experienced/` — Experienced track chapters 1-18
- `src/ds-narratives/` — 16 DS Deep Dives: narrative-driven data structures built from scratch
- `src/capstone/` — Coding challenges, system design, mock interviews (shared)
- `src/evolution/` — Living changelog
- `code/ch00/` — Part 0 standalone Rust exercises
- `code/ch01/` through `code/ch18/` — Compilable project snapshots per chapter
- `code/capstone/` — 8 DSA exercises

## Reference

Inspired by Erik Grinaker's [toydb](https://github.com/erikgrinaker/toydb) — an educational distributed SQL database.

## License

MIT
