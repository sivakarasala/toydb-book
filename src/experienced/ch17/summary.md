## What You Built

In this chapter, you:

1. **Connected all layers** — SQL string to tokens to AST to plan to optimized plan to replicated log entry to executed result to client response, through 9 distinct processing stages
2. **Built the Server struct** — a single owner for all layers, routing reads directly to storage and writes through Raft consensus
3. **Designed error propagation** — a unified `DbError` type with `From` implementations for all layer errors, enabling clean `?`-based propagation
4. **Implemented configuration and startup** — recovery from disk, snapshot restoration, WAL replay, Raft initialization, connection acceptance
5. **Wrote integration tests** — end-to-end SQL queries through the complete stack, including recovery tests that verify data survives server restarts
6. **Practiced Rust's module system** — `pub`/`pub(crate)`/private visibility, `use` imports, `mod` declarations, workspace organization, re-exports

Your database is complete. It accepts SQL over TCP, parses it, plans it, replicates writes through Raft, executes queries against MVCC storage, and returns results to the client. Data survives crashes. Writes are replicated to multiple nodes. Reads are served from the leader's local storage.

Chapter 18 adds rigor: testing strategies for each layer, benchmarking to measure performance, and ideas for extending your database with new features.

---

### DS Deep Dive

Our integration uses a synchronous request-response model — the client sends a query and blocks until the response arrives. Production databases support pipelining (send multiple queries before reading any responses), streaming (send results row by row as they are produced), and multiplexing (interleave multiple queries on the same connection). This deep dive explores these advanced protocol patterns, their impact on throughput and latency, and how they interact with connection pooling.
