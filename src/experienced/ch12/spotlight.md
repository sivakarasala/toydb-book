## Spotlight: Structs & Networking

Every chapter has one spotlight concept. This chapter's spotlight is **structs and networking** — how Rust handles TCP connections, reads and writes bytes, and structures network messages.

### std::net: TCP in the standard library

Rust's standard library provides everything you need for TCP networking. No external crates required:

```rust
use std::net::{TcpListener, TcpStream};

// Server: listen for connections
let listener = TcpListener::bind("127.0.0.1:4000")?;
for stream in listener.incoming() {
    let stream = stream?;
    println!("New connection from {}", stream.peer_addr()?);
    // handle the connection...
}

// Client: connect to a server
let mut stream = TcpStream::connect("127.0.0.1:4000")?;
// send and receive data...
```

`TcpListener::bind()` creates a listening socket. `listener.incoming()` returns an iterator of incoming connections — each one is a `TcpStream`. On the client side, `TcpStream::connect()` establishes a connection to a server.

A `TcpStream` implements both `Read` and `Write`. You can read bytes from it (data sent by the remote end) and write bytes to it (data you are sending). The same stream handles both directions.

### The Read and Write traits

`Read` and `Write` are Rust's core I/O traits:

```rust
pub trait Read {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize>;
}

pub trait Write {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize>;
    fn flush(&mut self) -> io::Result<()>;
}
```

`Read::read()` fills a buffer with bytes and returns how many were read. `Write::write()` sends bytes and returns how many were written. Both can return fewer bytes than requested — this is not an error, it means "I wrote/read what I could, call me again for the rest."

For convenience, `Read` provides `read_exact()` (blocks until the buffer is full) and `Write` provides `write_all()` (blocks until all bytes are written). These are what we will use for our protocol.

### BufReader and BufWriter: reducing system calls

Every `read()` and `write()` is a system call — the program asks the operating system to transfer data. System calls are expensive (context switch from user space to kernel space). If you send a 4-byte length prefix followed by a 100-byte message, that is 2 system calls. For thousands of messages, the overhead is significant.

`BufReader` and `BufWriter` wrap a `Read`/`Write` and add a user-space buffer. Multiple small reads/writes are batched into fewer, larger system calls:

```rust
use std::io::{BufReader, BufWriter, Read, Write};

// Without buffering: 2 system calls per message
stream.write_all(&length_bytes)?;  // syscall 1
stream.write_all(&message_bytes)?; // syscall 2

// With buffering: writes go to the buffer, flushed as one syscall
let mut writer = BufWriter::new(stream);
writer.write_all(&length_bytes)?;   // goes to buffer
writer.write_all(&message_bytes)?;  // goes to buffer
writer.flush()?;                     // ONE syscall sends both
```

`BufReader` is especially important for reading. TCP delivers data in chunks — a single `read()` might return half a message, or two messages at once. `BufReader` handles partial reads internally, so `read_exact()` always returns the complete requested amount.

### Struct layout for network messages

Network messages need to be converted to and from bytes. The simplest approach is a text-based protocol (like HTTP), but binary protocols are more compact and faster to parse. Our protocol uses a simple format:

```
Message format:
┌──────────────────┬────────────────────────────────┐
│  4 bytes (u32)   │  N bytes (payload)             │
│  Length prefix    │  JSON-encoded message body     │
│  (big-endian)    │                                │
└──────────────────┴────────────────────────────────┘
```

The 4-byte length prefix tells the reader how many bytes to read for the message body. Without it, the reader would not know where one message ends and the next begins — TCP is a byte stream, not a message stream.

We use JSON for the message body because it is human-readable (good for debugging) and trivially implemented with basic string formatting. A production protocol would use a binary format like Protocol Buffers or a custom binary encoding for performance.

> **Coming from JS/Python/Go?**
>
> | Concept | JavaScript | Python | Go | Rust |
> |---------|-----------|--------|-----|------|
> | TCP server | `net.createServer()` | `socket.socket()` | `net.Listen()` | `TcpListener::bind()` |
> | TCP client | `net.connect()` | `socket.connect()` | `net.Dial()` | `TcpStream::connect()` |
> | Read bytes | `socket.on('data')` | `sock.recv()` | `conn.Read()` | `stream.read()` |
> | Write bytes | `socket.write()` | `sock.send()` | `conn.Write()` | `stream.write_all()` |
> | Buffered I/O | Automatic (Node streams) | `io.BufferedReader` | `bufio.Reader` | `BufReader::new()` |
> | Read exact N bytes | Manual accumulation | `sock.recv(n, MSG_WAITALL)` | `io.ReadFull()` | `stream.read_exact()` |
>
> The biggest difference: Node.js and Python use event-driven or async I/O for TCP by default. Rust's `std::net` is synchronous (blocking). A `read()` call blocks the thread until data arrives. This is simpler but limits concurrency — each connection needs its own thread (or you use async, which we cover in Chapter 13). Go takes the same approach (goroutines + blocking I/O) but hides the threading behind its runtime.

---
