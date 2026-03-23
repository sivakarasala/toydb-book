# Chapter 1: What Is a Database?

You are about to build a database from scratch. Not a wrapper around SQLite. Not a tutorial that hand-waves over the hard parts. A real, working database — storage engine, SQL parser, query executor, client-server protocol, and distributed consensus — all in Rust. This first chapter starts where every database starts: a place to put data and a way to get it back.

By the end of this chapter, you will have:

- A working key-value store backed by Rust's `HashMap`
- A REPL (read-eval-print loop) that accepts SET, GET, DELETE, LIST, and STATS commands
- Support for multiple value types using Rust's `enum`
- A clear mental model of Rust's type system, variable bindings, and ownership semantics around collections

---
