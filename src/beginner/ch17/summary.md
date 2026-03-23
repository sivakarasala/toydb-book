## What You Built

In this chapter, you:

1. **Connected every layer** — a SQL string travels from the client through the lexer, parser, planner, optimizer, Raft log, executor, and back as a result
2. **Built the Server struct** — a single struct that owns all the layers and routes reads directly to storage while sending writes through Raft
3. **Designed error propagation** — a unified `DbError` enum with `From` implementations so errors from any layer flow cleanly up with `?`
4. **Implemented startup and recovery** — on boot, the server restores from snapshots, replays the WAL, initializes Raft, and starts accepting connections
5. **Wrote end-to-end tests** — SQL queries that travel through the entire stack and verify correct results, including recovery after restart

Your database is complete. It accepts SQL over TCP, parses and optimizes queries, replicates writes across a cluster, executes against MVCC storage, and returns results to the client.

---

### Key Rust concepts practiced

- **Module system** — `mod`, `pub`, `use`, `pub(crate)` for organizing a multi-layer codebase
- **The `From` trait** — converting between error types so each layer can define its own errors while the server handles them uniformly
- **Integration testing** — tests that exercise the full stack, not just individual functions
