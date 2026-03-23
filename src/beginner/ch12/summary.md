## What We Built

In this chapter, you built the networking layer that turns your database library into a service:

1. **Wire protocol** -- `Request` and `Response` enums, serialized to/from JSON-like bytes
2. **Length-prefixed framing** -- 4-byte length prefix solves TCP's message boundary problem
3. **TCP server** -- listens on a port, accepts connections, executes queries, sends results
4. **TCP client** -- connects to the server, sends SQL, receives and displays results
5. **REPL** -- interactive prompt for typing queries and seeing results

The Rust concepts you learned:

- **`std::net::TcpListener` and `TcpStream`** -- Rust's built-in TCP networking
- **`Read` and `Write` traits** -- the core I/O interface for reading and writing bytes
- **`read_exact()` and `write_all()`** -- convenience methods for complete reads/writes
- **`BufReader` and `BufWriter`** -- buffered I/O to reduce system call overhead
- **`From` trait for error conversion** -- enabling `?` to automatically convert `io::Error` to `ProtocolError`
- **`u32::to_be_bytes()` and `from_be_bytes()`** -- converting between numbers and byte arrays
- **`try_clone()`** -- creating a second handle to a TcpStream for simultaneous reading and writing
- **Binary framing** -- length-prefixed messages to solve TCP's streaming nature

Your database is no longer just a library. It is a service that anyone can connect to over the network. The pipeline is complete: client sends SQL over TCP -> server parses, plans, optimizes, executes -> server sends results back over TCP -> client displays them.

---
