# Chapter 13: Async Networking with Tokio

Your database server works. Open a terminal, connect, send a SQL query, get a result. It feels like magic. But try something: open a second terminal and connect to the same server at the same time. What happens?

The second client just sits there. Frozen. Waiting. It does not crash or give an error -- it simply hangs until the first client disconnects. Only then does the second client come to life.

This is because our server from Chapter 12 handles one connection at a time. It is like a restaurant with a single waiter who takes your order, walks to the kitchen, waits for the food to cook, brings it back, clears your table -- and only then walks over to the next customer. Every other customer is just sitting there, hungry and ignored.

A real database -- PostgreSQL, MySQL, or anything you would use in production -- handles hundreds or thousands of connections simultaneously. This chapter teaches you how to do that using **async programming** with Tokio, Rust's most popular async runtime.

By the end of this chapter, you will have:

- An understanding of what async programming is and why it matters
- Tokio added to your project and `#[tokio::main]` running your server
- An async TCP server that handles many clients at the same time
- Per-connection tasks spawned with `tokio::spawn`
- Graceful shutdown so the server stops cleanly

---
