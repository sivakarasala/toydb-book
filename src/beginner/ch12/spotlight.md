## Spotlight: Structs & Networking

Every chapter has one **spotlight concept** -- the Rust idea we dig into deeply. This chapter's spotlight is **structs and networking** -- how Rust handles TCP connections, reads and writes bytes, and structures network messages.

### What is TCP?

TCP (Transmission Control Protocol) is how computers talk to each other over a network. When your web browser loads a page, it uses TCP. When you query a database, it uses TCP.

Think of TCP like a phone call:
1. One side **listens** for calls (the server)
2. The other side **dials** (the client)
3. Once connected, both sides can **talk** (send data) and **listen** (receive data)
4. Either side can **hang up** (close the connection)

TCP guarantees that data arrives in order and without corruption. If a packet gets lost, TCP automatically resends it. You do not need to worry about lost or reordered data.

### std::net: TCP in Rust's standard library

Rust provides TCP networking in the standard library. No external crates needed:

```rust,ignore
use std::net::{TcpListener, TcpStream};

// SERVER: listen for connections on port 4000
let listener = TcpListener::bind("127.0.0.1:4000")?;
println!("Server listening on port 4000");

for stream in listener.incoming() {
    let stream = stream?;
    println!("New connection from {}", stream.peer_addr()?);
    // handle the connection...
}
```

Let us break this down:

1. **`TcpListener::bind("127.0.0.1:4000")`** -- create a listener on the local machine, port 4000. The `127.0.0.1` address means "this machine only" (localhost). The `?` propagates any error (e.g., port already in use).

2. **`listener.incoming()`** -- returns an iterator of incoming connections. Each item is a `Result<TcpStream, io::Error>`. The iterator never ends -- it waits for the next connection.

3. **`TcpStream`** -- represents one connection. You can read from it (data the client sent) and write to it (data you send back).

On the client side:

```rust,ignore
// CLIENT: connect to the server
let mut stream = TcpStream::connect("127.0.0.1:4000")?;
// now you can read and write bytes on this stream
```

`TcpStream::connect()` establishes a connection to the server. If the server is not running, this returns an error.

### The Read and Write traits

`TcpStream` implements two traits from the standard library: `Read` and `Write`. These are Rust's core I/O traits:

```rust,ignore
pub trait Read {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize>;
}

pub trait Write {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize>;
    fn flush(&mut self) -> io::Result<()>;
}
```

- **`Read::read()`** fills a buffer with bytes from the stream and returns how many bytes were read. It might read fewer bytes than the buffer size -- this is normal, not an error.
- **`Write::write()`** sends bytes and returns how many were actually sent. Again, it might send fewer than requested.
- **`Write::flush()`** ensures all buffered data is actually sent.

For convenience, `Read` provides `read_exact()` (keeps reading until the buffer is completely full) and `Write` provides `write_all()` (keeps writing until all bytes are sent). These are what we will use.

```rust,ignore
use std::io::{Read, Write};

// Read exactly 4 bytes
let mut buf = [0u8; 4];
stream.read_exact(&mut buf)?;

// Write all bytes
stream.write_all(b"Hello!")?;
```

> **What just happened?**
>
> `read_exact` and `write_all` are convenience methods that handle partial reads/writes for us. Without them, we would need to loop and keep track of how many bytes we have read/written so far. With them, we just say "read exactly this many bytes" or "send all these bytes" and Rust handles the details.

### BufReader and BufWriter: reducing overhead

Every `read()` and `write()` call is a **system call** -- the program asks the operating system to transfer data. System calls are expensive because the CPU switches between your program and the OS kernel.

If you send a 4-byte length prefix followed by a 100-byte message, that is 2 system calls. For thousands of messages, the overhead adds up.

`BufReader` and `BufWriter` wrap a `Read`/`Write` and add a buffer. Multiple small reads/writes are batched into fewer, larger system calls:

```rust,ignore
use std::io::{BufReader, BufWriter, Read, Write};

// Without buffering: 2 system calls
stream.write_all(&length_bytes)?;   // syscall 1
stream.write_all(&message_bytes)?;  // syscall 2

// With buffering: writes go to an in-memory buffer
let mut writer = BufWriter::new(stream);
writer.write_all(&length_bytes)?;   // goes to buffer
writer.write_all(&message_bytes)?;  // goes to buffer
writer.flush()?;                     // ONE syscall sends both
```

Think of it like writing letters. Without buffering, you drive to the mailbox for each letter. With buffering, you put letters in a pile and drive to the mailbox once with all of them.

### The framing problem: where does one message end?

TCP is a **byte stream**, not a message stream. If you send "Hello" followed by "World", the receiver might get "HelloWorld" as one chunk, or "Hel" and "loWorld" as two chunks. TCP does not preserve message boundaries.

This is a problem for our database protocol. If the client sends two SQL queries back to back, the server needs to know where the first query ends and the second begins.

The solution is **length-prefixed framing**: before each message, we send 4 bytes that contain the message length. The receiver reads 4 bytes to learn the length, then reads exactly that many bytes to get the message.

```
Message format:
┌──────────────────┬────────────────────────────────┐
│  4 bytes (u32)   │  N bytes (payload)             │
│  Length prefix    │  The actual message            │
│  (big-endian)    │                                │
└──────────────────┴────────────────────────────────┘
```

For example, sending "SELECT * FROM users" (20 bytes):

```
Bytes on the wire:
[0, 0, 0, 20]  [S, E, L, E, C, T, ...]
 ↑ length        ↑ the actual message
```

The receiver reads 4 bytes, interprets them as the number 20, then reads exactly 20 more bytes to get the complete message.

> **What just happened?**
>
> TCP delivers bytes as a continuous stream with no message boundaries. Length-prefixed framing solves this by prepending each message with its size. The receiver always knows exactly how many bytes to read. This is the same approach used by many real protocols -- PostgreSQL's wire protocol uses a similar scheme.

### Big-endian byte order

The 4-byte length is stored in **big-endian** order (most significant byte first). The number 20 becomes `[0, 0, 0, 20]`. The number 256 becomes `[0, 0, 1, 0]`.

Rust makes this easy with `u32::to_be_bytes()` and `u32::from_be_bytes()`:

```rust
fn main() {
    // Number to bytes
    let length: u32 = 20;
    let bytes = length.to_be_bytes();
    println!("{:?}", bytes);  // [0, 0, 0, 20]

    // Bytes to number
    let restored = u32::from_be_bytes([0, 0, 0, 20]);
    println!("{}", restored);  // 20
}
```

Big-endian is a convention. The important thing is that sender and receiver agree. We use big-endian because it is the network standard (also called "network byte order").

---
