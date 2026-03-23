# Chapter 12: Client-Server Protocol

Your database exists as a library. To run a query, you write Rust code that calls `execute_query()`, compile it, and read the output. That is fine for testing but useless for production. No one deploys a database by editing Rust source code and recompiling. A real database runs as a **server process** that listens for connections. Clients — a command-line tool, a web application, an analytics script — connect over the network, send SQL, and receive results.

This chapter builds the TCP layer that turns your library into a service. You will define a wire protocol (how messages are framed on a byte stream), build a server that accepts connections and dispatches queries, build a client that sends queries and displays results, and wrap the client in a REPL so you can interact with your database like you would with `psql` or `mysql`.

The spotlight concept is **structs and networking** — `std::net` for TCP connections, `Read` and `Write` traits for byte I/O, `BufReader` and `BufWriter` for efficient buffering, and struct design for network message types.

By the end of this chapter, you will have:

- A `Request` and `Response` message type with length-prefixed binary framing
- A TCP server that listens on a port, accepts connections, executes SQL, and returns results
- A TCP client that connects to the server, sends queries, and prints results
- A REPL that provides an interactive SQL prompt
- A clear understanding of TCP networking, buffering, and protocol design in Rust

---
