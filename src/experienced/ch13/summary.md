## What You Built

In this chapter, you:

1. **Converted the server to async** — replaced `std::net` with `tokio::net`, added `.await` to I/O calls, used `#[tokio::main]` for the runtime
2. **Spawned concurrent tasks** — `tokio::spawn` for per-connection handling, `async move` for ownership transfer, `Arc::clone` for shared state
3. **Shared the database safely** — `Arc<Mutex<Database>>` for concurrent access, short critical sections, mutex guard scoping
4. **Implemented graceful shutdown** — `tokio::signal::ctrl_c()`, broadcast channels for shutdown notification, `tokio::select!` for multiplexing
5. **Added connection management** — connection counting with `AtomicUsize`, connection limits with `Semaphore`, server metrics, load shedding

Your database is now a concurrent service. Multiple clients connect simultaneously, queries execute concurrently, and the server shuts down gracefully. This is the architecture of every modern database server.

Chapter 14 introduces distributed consensus with Raft — making your database fault-tolerant by replicating it across multiple servers. If one server crashes, the others continue serving queries.

---

### DS Deep Dive

Async I/O is built on operating system primitives — `epoll` (Linux), `kqueue` (macOS), `IOCP` (Windows). These event notification systems tell the runtime which sockets are ready for reading or writing, without blocking. This deep dive explores how Tokio's reactor translates OS events into Rust futures, how the task scheduler decides which future to poll next, and why async Rust compiles to state machines with zero runtime overhead.

**-> [Async Runtimes — "The Air Traffic Controller"](../ds-narratives/ch13-async-runtimes.md)**

---

### Reference implementation

The files you built in this chapter correspond to these files in the reference codebase:

| Your file | Reference |
|-----------|-----------|
| `src/async_protocol.rs` — async wire protocol | [`src/server.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/server.rs) — async message framing |
| `src/bin/toydb-async-server.rs` — async TCP server | [`src/server.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/server.rs) — `Server::serve()` with Tokio |
| Connection management — semaphore, metrics | [`src/server.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/server.rs) — connection lifecycle |
| Graceful shutdown — broadcast, select | [`src/server.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/server.rs) — shutdown handling |
