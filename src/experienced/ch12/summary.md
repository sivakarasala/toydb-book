## What You Built

In this chapter, you:

1. **Defined the wire protocol** — `Request` and `Response` enums with JSON serialization, length-prefixed framing with `write_message` and `read_message`, and a `MAX_MESSAGE_SIZE` safety guard
2. **Built the server** — `TcpListener` for accepting connections, sequential connection handling, request dispatch to the query engine, response serialization
3. **Built the client** — `TcpStream::connect()`, `try_clone()` for bidirectional I/O, `BufReader`/`BufWriter` for efficient buffering, clean `Result`-based error handling
4. **Built the REPL** — interactive prompt, special commands (`\q`, `\h`), semicolon stripping, graceful disconnect on EOF
5. **Practiced networking** — `std::net`, `Read`/`Write` traits, big-endian encoding, the framing problem, `From` for error conversion

Your database is now a service. Start the server, connect with the REPL, type SQL, and see results. It runs as a background process, accepts connections, and executes queries. This is the shape of every database you have ever used — PostgreSQL, MySQL, SQLite (when running in server mode), Redis.

Chapter 13 makes the server concurrent using async I/O with Tokio, so multiple clients can connect and run queries simultaneously.

---

### DS Deep Dive

Our protocol sends all result rows in a single response. PostgreSQL streams rows one at a time, which allows cursors — fetching 100 rows, processing them, then fetching the next 100. This deep dive explores streaming protocols, backpressure, and flow control — how the client tells the server to slow down when it cannot process data fast enough.

**-> [Network Protocols — "The Mail Room"](../../ds-narratives/ch12-tcp-framing.md)**

---

### Reference implementation

The files you built in this chapter correspond to these files in the reference codebase:

| Your file | Reference |
|-----------|-----------|
| `src/protocol.rs` — wire protocol | [`src/server.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/server.rs) — `Message` type and framing |
| `src/server.rs` — TCP server | [`src/server.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/server.rs) — `Server::serve()` |
| `src/client.rs` — TCP client | [`src/client.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/client.rs) — `Client` struct |
| `src/bin/toydb-server.rs` | [`src/bin/toydb.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/bin/toydb.rs) — server binary |
| `src/bin/toydb-repl.rs` | [`src/bin/toysql.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/bin/toysql.rs) — REPL client |
