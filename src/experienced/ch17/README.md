# Chapter 17: Integration — SQL over Raft

You have built a SQL parser, a query planner, an optimizer, an executor, an MVCC storage engine, a client-server protocol, and a Raft consensus layer. Each piece works in isolation. But a database is not a collection of parts — it is a system where a user types `INSERT INTO users VALUES (1, 'Alice')` and the data appears on three machines, survives crashes, and is immediately visible to subsequent queries. This chapter connects every layer into a single executable that does exactly that.

The spotlight concept is **module system and workspace** — how Rust organizes code into modules, crates, and workspaces, and how visibility rules (`pub`, `pub(crate)`, private) enforce the boundaries between layers that we have carefully maintained throughout this book.

By the end of this chapter, you will have:

- A clear map of the full request path: SQL string to parsed AST to planned operations to optimized plan to executed results to replicated state to client response
- A `Server` struct that wires together all layers with proper ownership
- Separate read and write paths (writes go through Raft, reads can use local state)
- A Rust workspace with separate crates for storage, SQL, Raft, and server
- Integration tests that execute SQL queries end-to-end through the complete stack
- Error propagation across layer boundaries using Rust's `From` trait and the `?` operator

---
