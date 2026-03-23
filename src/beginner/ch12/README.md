# Chapter 12: Client-Server Protocol

Your database exists as a library. To run a query, you write Rust code that calls functions like `build_executor()`, compile it, and read the output. That works for testing, but no one wants to recompile a program every time they need to ask the database a question. A real database runs as a **server** -- a program that starts up, waits for connections, and answers queries from anyone who connects.

Think of a restaurant. You (the client) sit at a table and tell the waiter what you want. The waiter carries your order to the kitchen (the server), which prepares the food and sends it back. You do not need to know how to cook -- you just need to know how to order.

This chapter builds the networking layer that turns your library into a service. You will define a wire protocol (how messages are formatted for the network), build a server that listens for connections and executes queries, build a client that connects and sends SQL, and wrap the client in a REPL (read-eval-print loop) so you can interact with your database like you would with `psql` or `mysql`.

By the end of this chapter, you will have:

- A `Request` and `Response` message type
- Length-prefixed binary framing for sending messages over TCP
- A TCP server that accepts connections, parses SQL, executes it, and returns results
- A TCP client that connects, sends queries, and prints results
- A REPL that provides an interactive SQL prompt

---
