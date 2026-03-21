# TCP Framing — "Where does one message end?"

Your database server is listening on port 5432. A client connects and sends a SQL query: `SELECT * FROM users WHERE id = 42`. The server reads bytes from the TCP socket. It gets `SELECT * FROM users WHERE id = 42`. Easy. But then the client sends two queries back to back: `INSERT INTO users VALUES (1, 'Alice')` followed immediately by `SELECT COUNT(*) FROM users`. The server reads from the socket and gets... `INSERT INTO users VALUES (1, 'Alice')SELECT COUNT(*) FROM users`. One blob of bytes. Where does the first query end? Where does the second begin?

TCP does not tell you. TCP is a **byte stream**, not a message stream. It promises to deliver your bytes in order and without corruption. It makes no promises about preserving message boundaries. Your carefully separated `send()` calls on the client side may arrive as one chunk, or three chunks, or seventeen chunks, depending on network conditions, buffer sizes, and the phase of the moon.

You need a protocol on top of TCP that marks where one message ends and the next begins. That protocol is called **framing**.

---

## The Naive Way

The simplest framing strategy: use a delimiter. End every message with a newline character `\n`. The receiver reads bytes until it sees `\n`, and everything before it is one message:

```rust
fn main() {
    // Simulate a byte stream with newline-delimited messages
    let stream = b"SELECT * FROM users\nINSERT INTO users VALUES (1, 'Alice')\nSELECT COUNT(*)\n";

    let mut messages = Vec::new();
    let mut current = Vec::new();

    for &byte in stream.iter() {
        if byte == b'\n' {
            if !current.is_empty() {
                messages.push(String::from_utf8(current.clone()).unwrap());
                current.clear();
            }
        } else {
            current.push(byte);
        }
    }

    println!("Parsed {} messages:", messages.len());
    for (i, msg) in messages.iter().enumerate() {
        println!("  [{}] {}", i, msg);
    }
}
```

This works for simple text protocols. Redis uses `\r\n` delimiters. HTTP/1.1 uses `\r\n` for headers and a blank line to separate headers from the body. It is easy to debug with telnet.

But delimiter-based framing has a fatal flaw: **the message itself might contain the delimiter**. What if you want to store a JSON blob with embedded newlines? What if the value is binary data -- a JPEG image, a compressed blob, a serialized protobuf -- and it happens to contain the byte `0x0A` (which is `\n`)? The parser sees the `0x0A` in the middle of your binary data, thinks it found a message boundary, and splits your message in half. Data corruption.

```rust
fn main() {
    // Binary data that happens to contain 0x0A (newline)
    let binary_payload: Vec<u8> = vec![0x89, 0x50, 0x4E, 0x47, 0x0A, 0x1A, 0x0A]; // PNG header!
    // The PNG file format literally starts with a \n byte.
    // A newline-delimited parser would split this in the wrong place.

    let contains_newline = binary_payload.contains(&b'\n');
    println!("Binary payload contains newline byte: {}", contains_newline);
    println!("A delimiter-based parser would corrupt this data.");

    // Even with escaping, you need to scan every byte looking for
    // the escape character. And if the escape character itself appears
    // in the data, you need to escape the escape. It gets ugly fast.
    let escaped: Vec<u8> = binary_payload.iter()
        .flat_map(|&b| {
            if b == b'\n' { vec![b'\\', b'n'] }
            else if b == b'\\' { vec![b'\\', b'\\'] }
            else { vec![b] }
        })
        .collect();

    println!("Original size: {} bytes", binary_payload.len());
    println!("Escaped size: {} bytes", escaped.len());
    println!("Escaping adds overhead and complexity.");
}
```

Escaping is a band-aid. It works, but it forces you to scan every byte for escape sequences on both the sending and receiving side. It adds CPU overhead, increases message size unpredictably, and is a fertile breeding ground for bugs. Every protocol that uses escaping eventually regrets it.

---

## The Insight

Think about how a postal service handles packages. They do not look for a special "end of package" marking inside the box. Instead, the shipping label on the outside says how much the package weighs and how big it is. The postal worker reads the label, handles exactly that many pounds and cubic inches, and knows the next package starts right after.

**Length-prefix framing** works the same way. Before every message, you write a fixed-size header that says exactly how many bytes the message contains. The receiver reads the header (always the same size -- say, 4 bytes), extracts the length, then reads exactly that many bytes. Done. The next message starts immediately after.

```
[length: 4 bytes][message: length bytes][length: 4 bytes][message: length bytes]...
```

No scanning. No escaping. No ambiguity. The message can contain any byte value -- newlines, nulls, the length header bytes themselves -- because the receiver never looks at the message content to find boundaries. It trusts the header.

This is how almost every serious protocol works:
- **PostgreSQL wire protocol**: 1-byte message type + 4-byte length + payload
- **MySQL wire protocol**: 3-byte length + 1-byte sequence number + payload
- **HTTP/2**: 3-byte length + 1-byte type + 1-byte flags + 4-byte stream ID + payload
- **gRPC**: 1-byte compressed flag + 4-byte length + payload
- **Kafka protocol**: 4-byte length + payload

Let's build a complete frame reader and writer.

---

## The Build

### The Frame Format

We will use a simple format:

```
[payload_length: 4 bytes, big-endian u32][payload: payload_length bytes]
```

Big-endian (network byte order) is the convention for network protocols. The maximum message size is 2^32 - 1 bytes (about 4 GB), which is more than enough for database messages.

### The Frame Writer

The writer takes a message (a slice of bytes) and produces a framed version: the 4-byte length header followed by the payload.

```rust,ignore
use std::io::{self, Write};

/// Write a length-prefixed frame to any Write destination.
fn write_frame(writer: &mut impl Write, payload: &[u8]) -> io::Result<()> {
    let length = payload.len() as u32;
    writer.write_all(&length.to_be_bytes())?; // 4-byte big-endian length
    writer.write_all(payload)?;                // the payload itself
    writer.flush()?;
    Ok(())
}
```

Three lines. That is the entire write side. The simplicity is the point -- length-prefix framing adds almost zero complexity to the sender.

### The Frame Reader: Handling Partial Reads

The read side is where it gets interesting. TCP might deliver your 4-byte header in two separate chunks: first 2 bytes, then 2 bytes. Or it might deliver the header plus half the payload in one chunk. You need to handle **partial reads**.

The core challenge: `read()` on a TCP socket returns "however many bytes are available right now," which might be less than what you asked for. You need to keep reading until you have all the bytes you need.

```rust,ignore
use std::io::{self, Read};

/// Read exactly `n` bytes from a reader, handling partial reads.
/// This is what `Read::read_exact` does, but let's see the logic.
fn read_full(reader: &mut impl Read, buf: &mut [u8]) -> io::Result<()> {
    let mut filled = 0;
    while filled < buf.len() {
        match reader.read(&mut buf[filled..]) {
            Ok(0) => {
                // Connection closed before we got all our bytes
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    format!("connection closed after {} of {} bytes", filled, buf.len()),
                ));
            }
            Ok(n) => {
                filled += n;
            }
            Err(ref e) if e.kind() == io::ErrorKind::Interrupted => {
                // Interrupted by a signal, just retry
                continue;
            }
            Err(e) => return Err(e),
        }
    }
    Ok(())
}
```

The `read()` call might return fewer bytes than requested -- that is normal, not an error. We track how many bytes we have so far (`filled`) and keep reading until we have them all. The only true errors are: connection closed (returned 0 bytes), or an actual I/O error.

Now the frame reader:

```rust,ignore
/// Read one length-prefixed frame. Returns the payload bytes.
fn read_frame(reader: &mut impl Read) -> io::Result<Vec<u8>> {
    // Step 1: Read the 4-byte length header
    let mut header = [0u8; 4];
    read_full(reader, &mut header)?;

    let length = u32::from_be_bytes(header) as usize;

    // Step 2: Validate the length (prevent denial-of-service)
    const MAX_FRAME_SIZE: usize = 16 * 1024 * 1024; // 16 MB
    if length > MAX_FRAME_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("frame too large: {} bytes (max {})", length, MAX_FRAME_SIZE),
        ));
    }

    // Step 3: Read exactly `length` bytes of payload
    let mut payload = vec![0u8; length];
    read_full(reader, &mut payload)?;

    Ok(payload)
}
```

The `MAX_FRAME_SIZE` check is critical for production systems. Without it, a malicious client could send the header `0xFF 0xFF 0xFF 0xFF` (4 GB), causing the server to allocate 4 GB of memory for a single message. This is a classic denial-of-service vector.

### A Complete FrameCodec

Let's wrap everything into a clean struct that manages buffering:

```rust
use std::io::{self, Cursor, Read, Write};

const MAX_FRAME_SIZE: usize = 16 * 1024 * 1024; // 16 MB

struct FrameCodec<S> {
    stream: S,
}

impl<S: Read + Write> FrameCodec<S> {
    fn new(stream: S) -> Self {
        FrameCodec { stream }
    }

    /// Send a message as a length-prefixed frame.
    fn send(&mut self, payload: &[u8]) -> io::Result<()> {
        let length = payload.len() as u32;
        self.stream.write_all(&length.to_be_bytes())?;
        self.stream.write_all(payload)?;
        self.stream.flush()?;
        Ok(())
    }

    /// Receive one frame. Returns the payload.
    fn receive(&mut self) -> io::Result<Vec<u8>> {
        // Read the 4-byte length header
        let mut header = [0u8; 4];
        self.stream.read_exact(&mut header)?;
        let length = u32::from_be_bytes(header) as usize;

        if length > MAX_FRAME_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("frame too large: {} bytes", length),
            ));
        }

        // Read the payload
        let mut payload = vec![0u8; length];
        self.stream.read_exact(&mut payload)?;
        Ok(payload)
    }
}

fn main() -> io::Result<()> {
    // Simulate a TCP connection with an in-memory buffer.
    // In production, this would be a TcpStream.
    let mut buffer = Vec::new();

    // === Sender side ===
    {
        let mut codec = FrameCodec::new(&mut buffer);

        // Send three messages, including one with binary data
        codec.send(b"SELECT * FROM users")?;
        codec.send(b"INSERT INTO users VALUES (1, 'Alice\nBob')")?; // note the \n in the value!

        // Binary payload that would break delimiter-based framing
        let binary = vec![0x00, 0x0A, 0xFF, 0x0D, 0x0A, 0x00];
        codec.send(&binary)?;

        println!("Sent 3 frames ({} bytes total on the wire)", buffer.len());
    }

    // === Receiver side ===
    {
        let mut reader = Cursor::new(&buffer);
        let mut codec = FrameCodec::new(&mut reader);

        let msg1 = codec.receive()?;
        println!("\nFrame 1: {}", String::from_utf8_lossy(&msg1));

        let msg2 = codec.receive()?;
        println!("Frame 2: {}", String::from_utf8_lossy(&msg2));

        let msg3 = codec.receive()?;
        println!("Frame 3: {:?} ({} bytes, binary)", msg3, msg3.len());

        // All three messages decoded correctly, even though:
        // - Frame 2 contains a \n character
        // - Frame 3 contains \n, \r\n, and null bytes
    }

    // Show the wire format
    println!("\n--- Wire format breakdown ---");
    let mut pos = 0;
    let mut frame_num = 1;
    while pos + 4 <= buffer.len() {
        let len = u32::from_be_bytes([buffer[pos], buffer[pos+1], buffer[pos+2], buffer[pos+3]]) as usize;
        println!("Frame {}: header [{:02x} {:02x} {:02x} {:02x}] = {} bytes payload",
                 frame_num, buffer[pos], buffer[pos+1], buffer[pos+2], buffer[pos+3], len);
        pos += 4 + len;
        frame_num += 1;
    }

    Ok(())
}
```

### Typed Messages: Adding a Message Type Header

Database protocols do not just send raw bytes -- they send typed messages. PostgreSQL, for example, has `'Q'` for query, `'D'` for data row, `'C'` for command complete, `'E'` for error. Let's extend our framing to include a message type byte:

```rust,ignore
use std::io::{self, Read, Write};

/// Wire format: [type: 1 byte][length: 4 bytes][payload: length bytes]
/// The length includes the 4 bytes of the length field itself (PostgreSQL convention).

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
enum MessageType {
    Query = b'Q',
    Result = b'R',
    Error = b'E',
    Terminate = b'X',
}

impl MessageType {
    fn from_byte(b: u8) -> Option<Self> {
        match b {
            b'Q' => Some(MessageType::Query),
            b'R' => Some(MessageType::Result),
            b'E' => Some(MessageType::Error),
            b'X' => Some(MessageType::Terminate),
            _ => None,
        }
    }
}

struct TypedMessage {
    msg_type: MessageType,
    payload: Vec<u8>,
}

fn send_typed(writer: &mut impl Write, msg: &TypedMessage) -> io::Result<()> {
    writer.write_all(&[msg.msg_type as u8])?;
    let length = (msg.payload.len() as u32) + 4; // include the length field itself
    writer.write_all(&length.to_be_bytes())?;
    writer.write_all(&msg.payload)?;
    writer.flush()?;
    Ok(())
}

fn receive_typed(reader: &mut impl Read) -> io::Result<TypedMessage> {
    // Read the 1-byte type
    let mut type_buf = [0u8; 1];
    reader.read_exact(&mut type_buf)?;

    let msg_type = MessageType::from_byte(type_buf[0])
        .ok_or_else(|| io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unknown message type: 0x{:02x}", type_buf[0]),
        ))?;

    // Read the 4-byte length
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf)?;
    let length = u32::from_be_bytes(len_buf) as usize;

    // Length includes the 4 bytes of the length field itself
    if length < 4 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "length too small"));
    }
    let payload_len = length - 4;

    let mut payload = vec![0u8; payload_len];
    reader.read_exact(&mut payload)?;

    Ok(TypedMessage { msg_type, payload })
}
```

### Async Framing

In a real database server, you handle hundreds of connections concurrently. Blocking one thread per connection does not scale. With async Rust (using `tokio`), the framing logic is almost identical, but uses `AsyncRead` and `AsyncWrite`:

```rust,ignore
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

async fn send_frame(stream: &mut TcpStream, payload: &[u8]) -> io::Result<()> {
    let length = payload.len() as u32;
    stream.write_all(&length.to_be_bytes()).await?;
    stream.write_all(payload).await?;
    stream.flush().await?;
    Ok(())
}

async fn receive_frame(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
    let mut header = [0u8; 4];
    stream.read_exact(&mut header).await?;
    let length = u32::from_be_bytes(header) as usize;

    if length > 16 * 1024 * 1024 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "frame too large"));
    }

    let mut payload = vec![0u8; length];
    stream.read_exact(&mut payload).await?;
    Ok(payload)
}
```

The structure is identical -- `read_exact` and `write_all` become their async equivalents, and the function signature gains `async`. The framing logic does not change at all. This is one of the strengths of Rust's async design: the business logic looks the same whether it is blocking or async.

### Comparing Framing Strategies

There are three main approaches to framing. Let's compare them:

```rust
fn main() {
    println!("=== Framing Strategy Comparison ===\n");

    let message = b"Hello, World!\nThis has a newline.";

    // Strategy 1: Delimiter-based (\n)
    // Must escape the delimiter in the message
    let delimiter_frame: Vec<u8> = message.iter()
        .flat_map(|&b| if b == b'\n' { vec![b'\\', b'n'] } else { vec![b] })
        .chain(std::iter::once(b'\n'))
        .collect();
    println!("Delimiter-based:");
    println!("  Overhead: variable (depends on content)");
    println!("  Frame size: {} bytes (message was {})", delimiter_frame.len(), message.len());
    println!("  Handles binary: No (requires escaping)");
    println!("  Used by: Redis, HTTP/1.1 headers, CSV\n");

    // Strategy 2: Length-prefix (4-byte header)
    let length_frame_size = 4 + message.len();
    println!("Length-prefix:");
    println!("  Overhead: fixed 4 bytes");
    println!("  Frame size: {} bytes (message was {})", length_frame_size, message.len());
    println!("  Handles binary: Yes (no scanning needed)");
    println!("  Used by: PostgreSQL, MySQL, HTTP/2, gRPC, Kafka\n");

    // Strategy 3: Fixed-size frames
    let fixed_size = 64; // each frame is exactly 64 bytes
    let num_frames = (message.len() + fixed_size - 1) / fixed_size;
    println!("Fixed-size (64-byte frames):");
    println!("  Overhead: {} bytes padding", fixed_size * num_frames - message.len());
    println!("  Frame size: {} bytes (message was {})", fixed_size * num_frames, message.len());
    println!("  Handles binary: Yes");
    println!("  Used by: Ethernet (1500-byte MTU), disk sectors (512/4096 bytes)");
    println!("  Drawback: wastes space on small messages, fragments large ones");
}
```

---

## The Payoff

Let's build a complete client-server exchange using our frame codec, simulating what happens when a database client sends queries and receives results:

```rust
use std::io::{self, Cursor, Read, Write};

const MAX_FRAME_SIZE: usize = 16 * 1024 * 1024;

struct FrameCodec<S> {
    stream: S,
    frames_sent: usize,
    bytes_sent: usize,
    frames_received: usize,
    bytes_received: usize,
}

impl<S: Read + Write> FrameCodec<S> {
    fn new(stream: S) -> Self {
        FrameCodec {
            stream,
            frames_sent: 0,
            bytes_sent: 0,
            frames_received: 0,
            bytes_received: 0,
        }
    }

    fn send(&mut self, payload: &[u8]) -> io::Result<()> {
        let length = payload.len() as u32;
        self.stream.write_all(&length.to_be_bytes())?;
        self.stream.write_all(payload)?;
        self.stream.flush()?;
        self.frames_sent += 1;
        self.bytes_sent += 4 + payload.len();
        Ok(())
    }

    fn receive(&mut self) -> io::Result<Vec<u8>> {
        let mut header = [0u8; 4];
        self.stream.read_exact(&mut header)?;
        let length = u32::from_be_bytes(header) as usize;

        if length > MAX_FRAME_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("frame too large: {} bytes", length),
            ));
        }

        let mut payload = vec![0u8; length];
        self.stream.read_exact(&mut payload)?;
        self.frames_received += 1;
        self.bytes_received += 4 + payload.len();
        Ok(payload)
    }
}

fn main() -> io::Result<()> {
    let mut wire = Vec::new();

    // Client sends queries
    {
        let mut client = FrameCodec::new(&mut wire);
        client.send(b"SELECT id, name FROM users WHERE active = true")?;
        client.send(b"INSERT INTO users (name) VALUES ('Charlie')")?;
        client.send(b"SELECT COUNT(*) FROM users")?;
        println!("Client sent {} frames, {} bytes on wire",
                 client.frames_sent, client.bytes_sent);
    }

    // Server receives and processes
    {
        let mut reader = Cursor::new(&wire);
        let mut server = FrameCodec::new(&mut reader);

        println!("\nServer processing:");
        loop {
            match server.receive() {
                Ok(payload) => {
                    let query = String::from_utf8_lossy(&payload);
                    println!("  Query {}: {}", server.frames_received, query);
                }
                Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                    println!("  End of stream (client disconnected)");
                    break;
                }
                Err(e) => return Err(e),
            }
        }
        println!("Server received {} frames, {} bytes",
                 server.frames_received, server.bytes_received);
    }

    // Demonstrate that binary data survives framing intact
    println!("\n--- Binary data test ---");
    let mut binary_wire = Vec::new();
    {
        let mut codec = FrameCodec::new(&mut binary_wire);

        // Simulate sending a query result with binary column data
        let binary_row = vec![
            0x00, 0x00, 0x00, 0x2A, // int32: 42
            0x00, 0x05,              // varchar length: 5
            b'A', b'l', b'i', b'c', b'e', // "Alice"
            0x0A, 0x0D, 0x00,        // bytes that would break delimiters
        ];
        codec.send(&binary_row)?;
    }

    {
        let mut reader = Cursor::new(&binary_wire);
        let mut codec = FrameCodec::new(&mut reader);
        let received = codec.receive()?;
        assert_eq!(received.len(), 13);

        let id = i32::from_be_bytes([received[0], received[1], received[2], received[3]]);
        let name_len = u16::from_be_bytes([received[4], received[5]]) as usize;
        let name = String::from_utf8_lossy(&received[6..6 + name_len]);
        println!("Decoded row: id={}, name={}", id, name);
        println!("Binary payload with 0x0A/0x0D/0x00 bytes survived intact!");
    }

    // Overhead analysis
    println!("\n--- Framing overhead ---");
    let payload_sizes = [10, 100, 1_000, 10_000, 100_000];
    println!("{:>10} {:>10} {:>10}", "Payload", "Wire Size", "Overhead");
    for size in &payload_sizes {
        let wire_size = size + 4;
        let overhead_pct = 4.0 / *size as f64 * 100.0;
        println!("{:>10} {:>10} {:>9.2}%", size, wire_size, overhead_pct);
    }
    println!("4-byte header overhead is negligible for typical database messages.");

    Ok(())
}
```

The 4-byte overhead per message is negligible. A typical SQL query is 50-500 bytes; the framing adds less than 8% overhead. For result sets that might be 10 KB or more, the overhead is 0.04%. You get reliable message boundaries for essentially free.

---

## Complexity Table

| Operation | Delimiter-Based | Length-Prefix | Fixed-Size |
|-----------|----------------|---------------|------------|
| Encode | O(n) scan for delimiter | O(1) header + O(n) copy | O(n) copy + padding |
| Decode | O(n) scan for delimiter | O(1) header read + O(n) payload read | O(1) per frame |
| Handles binary | No (requires escaping) | Yes | Yes |
| Space overhead | Variable (escaping) | Fixed 4 bytes | Padding waste |
| Max message size | Unlimited | 4 GB (u32) | Fixed per frame |
| Streaming | Yes (byte by byte) | Need full header first | Yes |
| Debuggability | Easy (text visible) | Need hex dump | Easy (fixed offsets) |
| Implementation | Simple but fragile | Simple and robust | Simple but wasteful |

Length-prefix is the clear winner for database protocols. The only scenario where delimiters win is human-readable text protocols where you want to interact via telnet or netcat (e.g., Redis, SMTP, early HTTP).

---

## Where This Shows Up in Our Database

In Chapter 12, we build the TCP server for our database. The wire protocol uses length-prefix framing:

```rust,ignore
// Our toydb wire protocol:
// Client -> Server: [length: 4 bytes][SQL query: length bytes]
// Server -> Client: [length: 4 bytes][result: length bytes]

pub async fn handle_connection(mut stream: TcpStream) -> Result<()> {
    loop {
        let query = receive_frame(&mut stream).await?;
        let result = execute_query(&query)?;
        send_frame(&mut stream, &result).await?;
    }
}
```

This is the same pattern used by every database in production:

- **PostgreSQL** uses a type-byte + length-prefix format. The frontend/backend protocol spec defines over 30 message types, each with a 1-byte type tag and 4-byte length.
- **MySQL** uses a 3-byte length + 1-byte sequence number. The 3-byte length limits individual packets to 16 MB; larger payloads are split across multiple packets.
- **Redis** (since RESP3) uses a type-prefix + length format for binary-safe strings, while keeping `\r\n` delimiters for simple strings. The dual approach gives you text debuggability for commands and binary safety for data.
- **HTTP/2** completely replaced HTTP/1.1's text-based framing with binary length-prefixed frames. A 9-byte header (3 bytes length, 1 byte type, 1 byte flags, 4 bytes stream ID) precedes every frame. This is why HTTP/2 supports multiplexing -- multiple logical streams share one TCP connection, each identified by the stream ID in the frame header.

Framing is invisible infrastructure. You never think about it until it breaks -- and with length-prefix framing, it essentially never breaks.

---

## Try It Yourself

### Exercise 1: Checksummed Frames

Add a CRC32 checksum to each frame to detect corruption. The new format: `[length: 4 bytes][payload: length bytes][crc32: 4 bytes]`. The CRC covers the payload bytes. On receive, compute the CRC of the received payload and compare it to the stored CRC. Return an error if they do not match.

<details>
<summary>Solution</summary>

```rust
use std::io::{self, Cursor, Read, Write};

fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFFFFFF;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            if crc & 1 == 1 {
                crc = (crc >> 1) ^ 0xEDB88320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

struct ChecksummedCodec<S> {
    stream: S,
}

impl<S: Read + Write> ChecksummedCodec<S> {
    fn new(stream: S) -> Self {
        ChecksummedCodec { stream }
    }

    fn send(&mut self, payload: &[u8]) -> io::Result<()> {
        let length = payload.len() as u32;
        let checksum = crc32(payload);

        self.stream.write_all(&length.to_be_bytes())?;
        self.stream.write_all(payload)?;
        self.stream.write_all(&checksum.to_be_bytes())?;
        self.stream.flush()?;
        Ok(())
    }

    fn receive(&mut self) -> io::Result<Vec<u8>> {
        let mut header = [0u8; 4];
        self.stream.read_exact(&mut header)?;
        let length = u32::from_be_bytes(header) as usize;

        if length > 16 * 1024 * 1024 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "frame too large"));
        }

        let mut payload = vec![0u8; length];
        self.stream.read_exact(&mut payload)?;

        let mut crc_buf = [0u8; 4];
        self.stream.read_exact(&mut crc_buf)?;
        let stored_crc = u32::from_be_bytes(crc_buf);
        let computed_crc = crc32(&payload);

        if stored_crc != computed_crc {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "CRC mismatch: stored={:#010x}, computed={:#010x}",
                    stored_crc, computed_crc
                ),
            ));
        }

        Ok(payload)
    }
}

fn main() -> io::Result<()> {
    // Test normal operation
    let mut buffer = Vec::new();
    {
        let mut codec = ChecksummedCodec::new(&mut buffer);
        codec.send(b"Hello, checksummed world!")?;
        codec.send(b"Another message")?;
    }

    println!("Wire size: {} bytes", buffer.len());
    println!("Overhead per frame: 8 bytes (4 length + 4 CRC)\n");

    // Receive correctly
    {
        let mut reader = Cursor::new(&buffer);
        let mut codec = ChecksummedCodec::new(&mut reader);
        let msg1 = codec.receive()?;
        println!("Message 1: {}", String::from_utf8_lossy(&msg1));
        let msg2 = codec.receive()?;
        println!("Message 2: {}", String::from_utf8_lossy(&msg2));
    }

    // Now corrupt a byte and detect it
    println!("\n--- Corruption test ---");
    let mut corrupted = buffer.clone();
    corrupted[10] ^= 0xFF; // flip bits in the middle of the payload

    {
        let mut reader = Cursor::new(&corrupted);
        let mut codec = ChecksummedCodec::new(&mut reader);
        match codec.receive() {
            Ok(_) => println!("ERROR: corruption was not detected!"),
            Err(e) => println!("Corruption detected: {}", e),
        }
    }

    Ok(())
}
```

</details>

### Exercise 2: Message Pipeline

A real database client does not wait for a response before sending the next query. It **pipelines** -- sends multiple queries without waiting, then reads all responses in order. Implement a `Pipeline` struct that batches multiple frames into one write call (reducing system call overhead) and reads responses in order.

<details>
<summary>Solution</summary>

```rust
use std::io::{self, Cursor, Read, Write};

struct Pipeline {
    // Accumulate frames in a write buffer before flushing
    write_buffer: Vec<u8>,
    frames_buffered: usize,
}

impl Pipeline {
    fn new() -> Self {
        Pipeline {
            write_buffer: Vec::new(),
            frames_buffered: 0,
        }
    }

    /// Buffer a frame without sending it yet.
    fn enqueue(&mut self, payload: &[u8]) {
        let length = payload.len() as u32;
        self.write_buffer.extend_from_slice(&length.to_be_bytes());
        self.write_buffer.extend_from_slice(payload);
        self.frames_buffered += 1;
    }

    /// Flush all buffered frames to the writer in one call.
    fn flush_to(&mut self, writer: &mut impl Write) -> io::Result<usize> {
        writer.write_all(&self.write_buffer)?;
        writer.flush()?;
        let sent = self.frames_buffered;
        self.write_buffer.clear();
        self.frames_buffered = 0;
        Ok(sent)
    }

    /// Read N responses from the reader.
    fn read_responses(reader: &mut impl Read, count: usize) -> io::Result<Vec<Vec<u8>>> {
        let mut responses = Vec::with_capacity(count);
        for _ in 0..count {
            let mut header = [0u8; 4];
            reader.read_exact(&mut header)?;
            let length = u32::from_be_bytes(header) as usize;
            let mut payload = vec![0u8; length];
            reader.read_exact(&mut payload)?;
            responses.push(payload);
        }
        Ok(responses)
    }
}

fn main() -> io::Result<()> {
    // Simulate client pipelining 5 queries
    let mut pipeline = Pipeline::new();
    let queries = vec![
        "SELECT 1",
        "SELECT 2",
        "SELECT 3",
        "SELECT NOW()",
        "SELECT VERSION()",
    ];

    for query in &queries {
        pipeline.enqueue(query.as_bytes());
    }

    // One system call to send all 5 queries
    let mut wire = Vec::new();
    let sent = pipeline.flush_to(&mut wire)?;
    println!("Pipelined {} queries in {} bytes (1 system call)", sent, wire.len());

    // Server reads all queries
    let mut reader = Cursor::new(&wire);
    let received = Pipeline::read_responses(&mut reader, sent)?;

    println!("\nServer received:");
    for (i, payload) in received.iter().enumerate() {
        println!("  [{}] {}", i, String::from_utf8_lossy(payload));
    }

    // Now simulate server sending back 5 responses (also pipelined)
    let mut response_wire = Vec::new();
    let mut resp_pipeline = Pipeline::new();
    for i in 0..5 {
        let response = format!("Result for query {}", i);
        resp_pipeline.enqueue(response.as_bytes());
    }
    let resp_sent = resp_pipeline.flush_to(&mut response_wire)?;

    // Client reads all 5 responses
    let mut resp_reader = Cursor::new(&response_wire);
    let responses = Pipeline::read_responses(&mut resp_reader, resp_sent)?;

    println!("\nClient received {} responses:", responses.len());
    for (i, payload) in responses.iter().enumerate() {
        println!("  [{}] {}", i, String::from_utf8_lossy(payload));
    }

    // Without pipelining: 5 round trips = 5 * RTT latency
    // With pipelining: 1 round trip = 1 * RTT latency
    // On a 1ms RTT network: 5ms vs 1ms. On a 100ms WAN: 500ms vs 100ms.
    println!("\nPipelining reduces latency from N*RTT to 1*RTT");

    Ok(())
}
```

</details>

### Exercise 3: Variable-Length Header

Our 4-byte length header wastes 3 bytes on small messages (which are the majority in database protocols -- most queries are under 255 bytes). Implement a **variable-length integer** encoding: if the first byte is < 254, it IS the length (1-byte header). If it is 254, the next 2 bytes are the length (3-byte header). If it is 255, the next 4 bytes are the length (5-byte header). This is similar to how Bitcoin and Protocol Buffers encode lengths.

<details>
<summary>Solution</summary>

```rust
use std::io::{self, Cursor, Read, Write};

/// Encode a length as a variable-length integer.
fn encode_varint_length(writer: &mut impl Write, length: usize) -> io::Result<usize> {
    if length < 254 {
        writer.write_all(&[length as u8])?;
        Ok(1)
    } else if length <= 0xFFFF {
        writer.write_all(&[254])?;
        writer.write_all(&(length as u16).to_be_bytes())?;
        Ok(3)
    } else {
        writer.write_all(&[255])?;
        writer.write_all(&(length as u32).to_be_bytes())?;
        Ok(5)
    }
}

/// Decode a variable-length integer from the reader.
fn decode_varint_length(reader: &mut impl Read) -> io::Result<usize> {
    let mut first = [0u8; 1];
    reader.read_exact(&mut first)?;

    match first[0] {
        b if b < 254 => Ok(b as usize),
        254 => {
            let mut buf = [0u8; 2];
            reader.read_exact(&mut buf)?;
            Ok(u16::from_be_bytes(buf) as usize)
        }
        255 => {
            let mut buf = [0u8; 4];
            reader.read_exact(&mut buf)?;
            Ok(u32::from_be_bytes(buf) as usize)
        }
        _ => unreachable!(),
    }
}

fn send_varint_frame(writer: &mut impl Write, payload: &[u8]) -> io::Result<usize> {
    let header_size = encode_varint_length(writer, payload.len())?;
    writer.write_all(payload)?;
    writer.flush()?;
    Ok(header_size + payload.len())
}

fn receive_varint_frame(reader: &mut impl Read) -> io::Result<Vec<u8>> {
    let length = decode_varint_length(reader)?;
    if length > 16 * 1024 * 1024 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "frame too large"));
    }
    let mut payload = vec![0u8; length];
    reader.read_exact(&mut payload)?;
    Ok(payload)
}

fn main() -> io::Result<()> {
    println!("=== Variable-Length Header Comparison ===\n");

    let test_payloads: Vec<Vec<u8>> = vec![
        vec![0u8; 10],      // tiny message
        vec![0u8; 100],     // small message
        vec![0u8; 253],     // max 1-byte header
        vec![0u8; 254],     // needs 3-byte header
        vec![0u8; 1000],    // medium message
        vec![0u8; 65535],   // max 3-byte header
        vec![0u8; 65536],   // needs 5-byte header
        vec![0u8; 100_000], // large message
    ];

    println!("{:>10} {:>12} {:>12} {:>8}", "Payload", "Fixed (4B)", "VarInt", "Saved");
    println!("{}", "-".repeat(44));

    let mut total_fixed = 0usize;
    let mut total_varint = 0usize;

    for payload in &test_payloads {
        let fixed_size = 4 + payload.len();

        // Encode with varint to measure actual header size
        let mut buf = Vec::new();
        let varint_size = send_varint_frame(&mut buf, payload)?;

        let saved = fixed_size as isize - varint_size as isize;
        println!("{:>10} {:>12} {:>12} {:>8}",
                 payload.len(), fixed_size, varint_size, saved);

        total_fixed += fixed_size;
        total_varint += varint_size;
    }

    println!("{}", "-".repeat(44));
    println!("{:>10} {:>12} {:>12} {:>8}",
             "Total", total_fixed, total_varint,
             total_fixed as isize - total_varint as isize);

    // Verify round-trip
    println!("\n--- Round-trip verification ---");
    let mut wire = Vec::new();
    send_varint_frame(&mut wire, b"Hello")?;
    send_varint_frame(&mut wire, b"World")?;
    send_varint_frame(&mut wire, &vec![0x42; 300])?;

    let mut reader = Cursor::new(&wire);
    let msg1 = receive_varint_frame(&mut reader)?;
    let msg2 = receive_varint_frame(&mut reader)?;
    let msg3 = receive_varint_frame(&mut reader)?;

    println!("Frame 1: {} bytes (\"{}\")", msg1.len(), String::from_utf8_lossy(&msg1));
    println!("Frame 2: {} bytes (\"{}\")", msg2.len(), String::from_utf8_lossy(&msg2));
    println!("Frame 3: {} bytes (all 0x42? {})", msg3.len(), msg3.iter().all(|&b| b == 0x42));

    Ok(())
}
```

</details>

---

## Recap

TCP is a byte stream. It delivers your bytes in order but does not preserve message boundaries. Framing is the layer that tells the receiver where one message ends and the next begins. Delimiter-based framing (scanning for `\n`) is simple but breaks on binary data. Fixed-size framing wastes space. Length-prefix framing -- a fixed-size header that encodes the payload length -- is the standard solution used by virtually every production database protocol.

The implementation is trivial: write 4 bytes (the length), then the payload. Read 4 bytes, parse the length, read that many bytes. The subtlety is handling partial reads -- TCP may deliver fewer bytes than you asked for, so you need to loop until you have the full header and the full payload.

With this single technique, you can build a wire protocol that handles arbitrary binary data, supports message pipelining, and scales to thousands of concurrent connections. It is the invisible foundation that every database client library depends on.
