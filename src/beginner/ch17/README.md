# Chapter 17: Integration -- SQL over Raft

You have built a lot of pieces. A SQL lexer that turns text into tokens. A parser that turns tokens into a syntax tree. A query planner and optimizer. An executor that runs queries against storage. An MVCC storage engine with transactions. A client-server protocol. An async networking layer. A Raft consensus system with leader election, log replication, and durability.

Each piece works in isolation. But a database is not a collection of parts -- it is a system. A user types `INSERT INTO users VALUES (1, 'Alice')` and expects the data to appear on three machines, survive crashes, and be immediately visible to the next `SELECT`. This chapter connects every layer into a single executable that does exactly that.

By the end of this chapter, you will have:

- A clear understanding of how a SQL query flows through every layer
- Rust modules (`mod`, `use`, `pub`) explained from scratch
- A `Server` struct that owns and coordinates all layers
- Separate read and write paths (writes go through Raft, reads use local storage)
- An end-to-end test: INSERT then SELECT through the complete stack

---
