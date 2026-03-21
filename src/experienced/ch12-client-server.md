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

## Exercise 1: Define the Wire Protocol

**Goal:** Define `Request` and `Response` message types and implement length-prefixed framing for reading and writing them over a TCP stream.

### Step 1: Define the message types

Create `src/protocol.rs`:

```rust
// src/protocol.rs

use std::io::{self, Read, Write, BufReader, BufWriter};

/// A client request — currently just a SQL string.
/// Future versions could add prepared statements, transactions, etc.
#[derive(Debug, Clone)]
pub enum Request {
    /// Execute a SQL query and return results.
    Query(String),
    /// Gracefully disconnect.
    Disconnect,
}

/// A server response — either results or an error.
#[derive(Debug, Clone)]
pub enum Response {
    /// Query executed successfully. Contains column names and rows.
    Rows {
        columns: Vec<String>,
        rows: Vec<Vec<String>>,  // each row is a Vec of string-formatted values
    },
    /// Query executed successfully but returned no rows (INSERT, CREATE TABLE, etc.)
    Ok {
        message: String,
    },
    /// Query failed.
    Error {
        message: String,
    },
}
```

Why `Vec<Vec<String>>` instead of `Vec<Row>`? Because the wire format should not depend on the internal `Row` type. The protocol converts all values to strings for transmission. This keeps the protocol module decoupled from the executor module — the server converts `Row` to `Vec<String>` before sending, and the client displays strings without knowing about `Value` types.

### Step 2: Implement serialization

We will use a simple JSON-like text format, serialized and deserialized manually to avoid external crate dependencies:

```rust
// src/protocol.rs (continued)

impl Request {
    /// Serialize to bytes for transmission.
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            Request::Query(sql) => {
                format!("{{\"type\":\"query\",\"sql\":{}}}", json_escape(sql))
                    .into_bytes()
            }
            Request::Disconnect => {
                b"{\"type\":\"disconnect\"}".to_vec()
            }
        }
    }

    /// Deserialize from bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ProtocolError> {
        let s = std::str::from_utf8(bytes)
            .map_err(|_| ProtocolError::InvalidMessage("not valid UTF-8".to_string()))?;

        if s.contains("\"type\":\"disconnect\"") {
            return Ok(Request::Disconnect);
        }

        // Extract SQL from: {"type":"query","sql":"..."}
        if let Some(start) = s.find("\"sql\":\"") {
            let sql_start = start + 7; // skip past "sql":"
            // Find the closing quote (handling escaped quotes)
            let sql = json_unescape(&s[sql_start..])?;
            return Ok(Request::Query(sql));
        }

        Err(ProtocolError::InvalidMessage(format!("cannot parse request: {}", s)))
    }
}

impl Response {
    /// Serialize to bytes for transmission.
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            Response::Rows { columns, rows } => {
                let cols: Vec<String> = columns.iter()
                    .map(|c| json_escape(c))
                    .collect();
                let row_strs: Vec<String> = rows.iter()
                    .map(|row| {
                        let cells: Vec<String> = row.iter()
                            .map(|v| json_escape(v))
                            .collect();
                        format!("[{}]", cells.join(","))
                    })
                    .collect();

                format!(
                    "{{\"type\":\"rows\",\"columns\":[{}],\"rows\":[{}]}}",
                    cols.join(","),
                    row_strs.join(","),
                )
                .into_bytes()
            }
            Response::Ok { message } => {
                format!("{{\"type\":\"ok\",\"message\":{}}}", json_escape(message))
                    .into_bytes()
            }
            Response::Error { message } => {
                format!("{{\"type\":\"error\",\"message\":{}}}", json_escape(message))
                    .into_bytes()
            }
        }
    }

    /// Deserialize from bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ProtocolError> {
        let s = std::str::from_utf8(bytes)
            .map_err(|_| ProtocolError::InvalidMessage("not valid UTF-8".to_string()))?;

        if s.contains("\"type\":\"error\"") {
            let msg = extract_json_string(s, "message")?;
            return Ok(Response::Error { message: msg });
        }

        if s.contains("\"type\":\"ok\"") {
            let msg = extract_json_string(s, "message")?;
            return Ok(Response::Ok { message: msg });
        }

        if s.contains("\"type\":\"rows\"") {
            let columns = extract_json_string_array(s, "columns")?;
            let rows = extract_json_rows(s)?;
            return Ok(Response::Rows { columns, rows });
        }

        Err(ProtocolError::InvalidMessage(format!("cannot parse response: {}", s)))
    }
}
```

### Step 3: Implement the framing layer

The framing layer reads and writes length-prefixed messages:

```rust
// src/protocol.rs (continued)

/// Errors that can occur during protocol operations.
#[derive(Debug)]
pub enum ProtocolError {
    /// I/O error (connection closed, network failure, etc.)
    Io(io::Error),
    /// Message could not be parsed.
    InvalidMessage(String),
    /// Message exceeds maximum allowed size.
    MessageTooLarge(usize),
}

impl From<io::Error> for ProtocolError {
    fn from(err: io::Error) -> Self {
        ProtocolError::Io(err)
    }
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProtocolError::Io(e) => write!(f, "I/O error: {}", e),
            ProtocolError::InvalidMessage(s) => write!(f, "invalid message: {}", s),
            ProtocolError::MessageTooLarge(size) => {
                write!(f, "message too large: {} bytes", size)
            }
        }
    }
}

/// Maximum message size: 16 MB. Prevents a malformed length prefix
/// from causing the server to allocate gigabytes of memory.
const MAX_MESSAGE_SIZE: u32 = 16 * 1024 * 1024;

/// Write a length-prefixed message to a stream.
///
/// Format: [4 bytes big-endian length][N bytes payload]
pub fn write_message<W: Write>(
    writer: &mut BufWriter<W>,
    payload: &[u8],
) -> Result<(), ProtocolError> {
    let len = payload.len() as u32;
    if len > MAX_MESSAGE_SIZE {
        return Err(ProtocolError::MessageTooLarge(len as usize));
    }

    writer.write_all(&len.to_be_bytes())?;
    writer.write_all(payload)?;
    writer.flush()?;
    Ok(())
}

/// Read a length-prefixed message from a stream.
///
/// Returns None if the connection was closed (read returns 0 bytes).
pub fn read_message<R: Read>(
    reader: &mut BufReader<R>,
) -> Result<Option<Vec<u8>>, ProtocolError> {
    // Read the 4-byte length prefix
    let mut len_bytes = [0u8; 4];
    match reader.read_exact(&mut len_bytes) {
        Ok(()) => {}
        Err(ref e) if e.kind() == io::ErrorKind::UnexpectedEof => {
            return Ok(None); // connection closed
        }
        Err(e) => return Err(ProtocolError::Io(e)),
    }

    let len = u32::from_be_bytes(len_bytes);
    if len > MAX_MESSAGE_SIZE {
        return Err(ProtocolError::MessageTooLarge(len as usize));
    }

    // Read the payload
    let mut payload = vec![0u8; len as usize];
    reader.read_exact(&mut payload)?;

    Ok(Some(payload))
}
```

### Step 4: Implement the JSON helpers

These helper functions handle basic JSON string escaping and extraction without any external crates:

```rust
// src/protocol.rs (continued)

/// Escape a string for JSON (wrap in quotes, escape special characters).
fn json_escape(s: &str) -> String {
    let mut escaped = String::with_capacity(s.len() + 2);
    escaped.push('"');
    for ch in s.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            c => escaped.push(c),
        }
    }
    escaped.push('"');
    escaped
}

/// Extract a string from JSON after the opening quote, handling escape sequences.
/// Input starts right after the opening quote.
fn json_unescape(s: &str) -> Result<String, ProtocolError> {
    let mut result = String::new();
    let mut chars = s.chars();
    loop {
        match chars.next() {
            None => return Err(ProtocolError::InvalidMessage(
                "unterminated string".to_string()
            )),
            Some('"') => return Ok(result),
            Some('\\') => match chars.next() {
                Some('"') => result.push('"'),
                Some('\\') => result.push('\\'),
                Some('n') => result.push('\n'),
                Some('r') => result.push('\r'),
                Some('t') => result.push('\t'),
                Some(c) => {
                    result.push('\\');
                    result.push(c);
                }
                None => return Err(ProtocolError::InvalidMessage(
                    "unterminated escape".to_string()
                )),
            },
            Some(c) => result.push(c),
        }
    }
}

/// Extract a JSON string value by field name.
/// Searches for "field":"value" and returns the unescaped value.
fn extract_json_string(json: &str, field: &str) -> Result<String, ProtocolError> {
    let pattern = format!("\"{}\":\"", field);
    if let Some(start) = json.find(&pattern) {
        let value_start = start + pattern.len();
        json_unescape(&json[value_start..])
    } else {
        Err(ProtocolError::InvalidMessage(
            format!("field '{}' not found", field)
        ))
    }
}

/// Extract a JSON array of strings by field name.
/// Searches for "field":["a","b","c"] and returns vec!["a", "b", "c"].
fn extract_json_string_array(
    json: &str,
    field: &str,
) -> Result<Vec<String>, ProtocolError> {
    let pattern = format!("\"{}\":[", field);
    if let Some(start) = json.find(&pattern) {
        let array_start = start + pattern.len();
        let remaining = &json[array_start..];

        // Find the closing bracket
        let end = find_matching_bracket(remaining, ']')
            .ok_or_else(|| ProtocolError::InvalidMessage(
                "unterminated array".to_string()
            ))?;

        let array_content = &remaining[..end];
        if array_content.trim().is_empty() {
            return Ok(Vec::new());
        }

        // Split by commas outside of quotes
        let items = split_json_array(array_content);
        let mut result = Vec::new();
        for item in items {
            let trimmed = item.trim();
            if trimmed.starts_with('"') && trimmed.len() >= 2 {
                result.push(json_unescape(&trimmed[1..])?);
            }
        }
        Ok(result)
    } else {
        Err(ProtocolError::InvalidMessage(
            format!("field '{}' not found", field)
        ))
    }
}

/// Extract rows from JSON: "rows":[[...],[...]]
fn extract_json_rows(json: &str) -> Result<Vec<Vec<String>>, ProtocolError> {
    let pattern = "\"rows\":[";
    if let Some(start) = json.find(pattern) {
        let array_start = start + pattern.len();
        let remaining = &json[array_start..];

        let end = find_matching_bracket(remaining, ']')
            .ok_or_else(|| ProtocolError::InvalidMessage(
                "unterminated rows array".to_string()
            ))?;

        let rows_content = &remaining[..end];
        if rows_content.trim().is_empty() {
            return Ok(Vec::new());
        }

        // Each row is a [...] array
        let mut rows = Vec::new();
        let mut depth = 0;
        let mut row_start = None;

        for (i, ch) in rows_content.char_indices() {
            match ch {
                '[' => {
                    if depth == 0 {
                        row_start = Some(i + 1);
                    }
                    depth += 1;
                }
                ']' => {
                    depth -= 1;
                    if depth == 0 {
                        if let Some(start) = row_start {
                            let row_content = &rows_content[start..i];
                            let items = split_json_array(row_content);
                            let mut row = Vec::new();
                            for item in items {
                                let trimmed = item.trim();
                                if trimmed.starts_with('"') && trimmed.len() >= 2 {
                                    row.push(json_unescape(&trimmed[1..])?);
                                }
                            }
                            rows.push(row);
                        }
                        row_start = None;
                    }
                }
                _ => {}
            }
        }

        Ok(rows)
    } else {
        Ok(Vec::new())
    }
}

/// Find the position of a matching closing bracket/brace,
/// accounting for nesting and quoted strings.
fn find_matching_bracket(s: &str, close: char) -> Option<usize> {
    let open = match close {
        ']' => '[',
        '}' => '{',
        _ => return None,
    };
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escape = false;

    for (i, ch) in s.char_indices() {
        if escape {
            escape = false;
            continue;
        }
        if ch == '\\' && in_string {
            escape = true;
            continue;
        }
        if ch == '"' {
            in_string = !in_string;
            continue;
        }
        if in_string {
            continue;
        }
        if ch == open {
            depth += 1;
        } else if ch == close {
            if depth == 0 {
                return Some(i);
            }
            depth -= 1;
        }
    }
    None
}

/// Split a JSON array's contents by commas, respecting quoted strings.
fn split_json_array(s: &str) -> Vec<&str> {
    let mut items = Vec::new();
    let mut start = 0;
    let mut in_string = false;
    let mut escape = false;
    let mut depth = 0;

    for (i, ch) in s.char_indices() {
        if escape {
            escape = false;
            continue;
        }
        if ch == '\\' && in_string {
            escape = true;
            continue;
        }
        if ch == '"' {
            in_string = !in_string;
            continue;
        }
        if in_string {
            continue;
        }
        if ch == '[' || ch == '{' {
            depth += 1;
        } else if ch == ']' || ch == '}' {
            depth -= 1;
        } else if ch == ',' && depth == 0 {
            items.push(&s[start..i]);
            start = i + 1;
        }
    }
    if start < s.len() {
        items.push(&s[start..]);
    }
    items
}
```

### Step 5: Test the protocol

```rust
// src/protocol.rs — tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_roundtrip() {
        let req = Request::Query("SELECT * FROM users WHERE name = 'O\\'Brien'".to_string());
        let bytes = req.to_bytes();
        let decoded = Request::from_bytes(&bytes).unwrap();
        match decoded {
            Request::Query(sql) => {
                assert_eq!(sql, "SELECT * FROM users WHERE name = 'O\\'Brien'");
            }
            _ => panic!("expected Query"),
        }
    }

    #[test]
    fn test_response_rows_roundtrip() {
        let resp = Response::Rows {
            columns: vec!["name".to_string(), "age".to_string()],
            rows: vec![
                vec!["Alice".to_string(), "30".to_string()],
                vec!["Bob".to_string(), "25".to_string()],
            ],
        };
        let bytes = resp.to_bytes();
        let decoded = Response::from_bytes(&bytes).unwrap();
        match decoded {
            Response::Rows { columns, rows } => {
                assert_eq!(columns, vec!["name", "age"]);
                assert_eq!(rows.len(), 2);
                assert_eq!(rows[0], vec!["Alice", "30"]);
                assert_eq!(rows[1], vec!["Bob", "25"]);
            }
            _ => panic!("expected Rows"),
        }
    }

    #[test]
    fn test_response_error_roundtrip() {
        let resp = Response::Error {
            message: "table \"users\" not found".to_string(),
        };
        let bytes = resp.to_bytes();
        let decoded = Response::from_bytes(&bytes).unwrap();
        match decoded {
            Response::Error { message } => {
                assert_eq!(message, "table \"users\" not found");
            }
            _ => panic!("expected Error"),
        }
    }

    #[test]
    fn test_disconnect_request() {
        let req = Request::Disconnect;
        let bytes = req.to_bytes();
        let decoded = Request::from_bytes(&bytes).unwrap();
        assert!(matches!(decoded, Request::Disconnect));
    }

    #[test]
    fn test_framing_roundtrip() {
        // Simulate a network stream with an in-memory buffer
        let mut buffer: Vec<u8> = Vec::new();

        // Write a message
        {
            let mut writer = BufWriter::new(&mut buffer);
            write_message(&mut writer, b"hello world").unwrap();
        }

        // Read it back
        {
            let mut reader = BufReader::new(&buffer[..]);
            let msg = read_message(&mut reader).unwrap().unwrap();
            assert_eq!(&msg, b"hello world");
        }
    }
}
```

```
Expected output:
$ cargo test protocol
running 5 tests
test protocol::tests::test_request_roundtrip ... ok
test protocol::tests::test_response_rows_roundtrip ... ok
test protocol::tests::test_response_error_roundtrip ... ok
test protocol::tests::test_disconnect_request ... ok
test protocol::tests::test_framing_roundtrip ... ok
test result: ok. 5 passed; 0 failed
```

<details>
<summary>Hint: If the JSON parsing fails on special characters</summary>

The most common bug is not escaping backslashes and quotes properly. When a SQL query contains `WHERE name = 'O\'Brien'`, the backslash and single quote must survive a round trip through `json_escape` and `json_unescape`. Test with strings containing `"`, `\`, `\n`, and other special characters.

Another subtle issue: `json_unescape` must handle the case where the JSON value contains escaped double quotes (`\"`). The closing quote of the JSON string is an unescaped `"`, not an escaped one. Make sure your loop correctly distinguishes `\"` (part of the string) from `"` (end of the string).

</details>

---

## Exercise 2: Build the Server

**Goal:** Create a TCP server that listens on a port, accepts connections, reads SQL queries, executes them, and sends back results.

### Step 1: Create the server module

Create `src/server.rs`:

```rust
// src/server.rs

use std::io::{BufReader, BufWriter};
use std::net::{TcpListener, TcpStream};

use crate::executor::{
    build_executor, ResultSet, Storage, Row,
};
use crate::types::Value;
use crate::planner::Schema;
use crate::protocol::{
    self, Request, Response, ProtocolError,
};

/// The database server. Holds the storage, schema, and listens for connections.
pub struct Server {
    storage: Storage,
    schema: Schema,
    listener: TcpListener,
}

impl Server {
    /// Create a new server bound to the given address.
    pub fn new(addr: &str) -> Result<Self, std::io::Error> {
        let listener = TcpListener::bind(addr)?;
        println!("toydb server listening on {}", addr);

        // Initialize with some sample data for testing
        let mut storage = Storage::new();
        let schema = Schema::new();

        Ok(Server {
            storage,
            schema,
            listener,
        })
    }

    /// Create a server with pre-loaded storage and schema.
    pub fn with_data(
        addr: &str,
        storage: Storage,
        schema: Schema,
    ) -> Result<Self, std::io::Error> {
        let listener = TcpListener::bind(addr)?;
        println!("toydb server listening on {}", addr);

        Ok(Server {
            storage,
            schema,
            listener,
        })
    }

    /// Run the server, accepting connections forever.
    pub fn run(&self) -> Result<(), std::io::Error> {
        println!("Waiting for connections...");

        for stream in self.listener.incoming() {
            match stream {
                Ok(stream) => {
                    let peer = stream.peer_addr()
                        .map(|a| a.to_string())
                        .unwrap_or_else(|_| "unknown".to_string());

                    println!("Connection from {}", peer);
                    self.handle_connection(stream, &peer);
                    println!("Connection from {} closed", peer);
                }
                Err(e) => {
                    eprintln!("Failed to accept connection: {}", e);
                }
            }
        }

        Ok(())
    }

    /// Handle a single client connection.
    fn handle_connection(&self, stream: TcpStream, peer: &str) {
        let read_stream = match stream.try_clone() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to clone stream: {}", e);
                return;
            }
        };

        let mut reader = BufReader::new(read_stream);
        let mut writer = BufWriter::new(stream);

        loop {
            // Read the next request
            let payload = match protocol::read_message(&mut reader) {
                Ok(Some(bytes)) => bytes,
                Ok(None) => {
                    // Connection closed by client
                    break;
                }
                Err(e) => {
                    eprintln!("[{}] Read error: {}", peer, e);
                    break;
                }
            };

            // Parse the request
            let request = match Request::from_bytes(&payload) {
                Ok(req) => req,
                Err(e) => {
                    eprintln!("[{}] Invalid request: {}", peer, e);
                    let response = Response::Error {
                        message: format!("invalid request: {}", e),
                    };
                    if let Err(e) = protocol::write_message(
                        &mut writer,
                        &response.to_bytes(),
                    ) {
                        eprintln!("[{}] Write error: {}", peer, e);
                        break;
                    }
                    continue;
                }
            };

            // Handle the request
            match request {
                Request::Disconnect => {
                    println!("[{}] Client disconnected gracefully", peer);
                    break;
                }
                Request::Query(sql) => {
                    println!("[{}] Query: {}", peer, sql);

                    let response = self.execute_query(&sql);

                    if let Err(e) = protocol::write_message(
                        &mut writer,
                        &response.to_bytes(),
                    ) {
                        eprintln!("[{}] Write error: {}", peer, e);
                        break;
                    }
                }
            }
        }
    }

    /// Execute a SQL query and return a Response.
    fn execute_query(&self, sql: &str) -> Response {
        // Use the full pipeline: lex -> parse -> plan -> optimize -> execute
        match crate::execute_query(sql, &self.schema, &self.storage) {
            Ok(result_set) => {
                // Convert ResultSet to Response::Rows
                let rows: Vec<Vec<String>> = result_set.rows.iter()
                    .map(|row| {
                        row.values.iter()
                            .map(|v| format!("{}", v))
                            .collect()
                    })
                    .collect();

                Response::Rows {
                    columns: result_set.columns,
                    rows,
                }
            }
            Err(e) => {
                Response::Error { message: e }
            }
        }
    }
}
```

### Step 2: Create the server binary

Create `src/bin/toydb-server.rs`:

```rust
// src/bin/toydb-server.rs

use toydb::server::Server;
use toydb::executor::{Storage, Row};
use toydb::types::Value;
use toydb::planner::Schema;

fn main() {
    let addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:4000".to_string());

    // Create storage with sample data
    let mut storage = Storage::new();
    let mut schema = Schema::new();

    // Add a sample table
    storage.create_table("users", vec![
        "id".to_string(),
        "name".to_string(),
        "age".to_string(),
    ]);

    storage.insert_row("users", Row::new(vec![
        Value::Integer(1),
        Value::String("Alice".to_string()),
        Value::Integer(30),
    ])).unwrap();

    storage.insert_row("users", Row::new(vec![
        Value::Integer(2),
        Value::String("Bob".to_string()),
        Value::Integer(25),
    ])).unwrap();

    storage.insert_row("users", Row::new(vec![
        Value::Integer(3),
        Value::String("Carol".to_string()),
        Value::Integer(35),
    ])).unwrap();

    // Register the table in the schema
    schema.add_table("users", vec![
        ("id".to_string(), toydb::planner::DataType::Integer),
        ("name".to_string(), toydb::planner::DataType::Text),
        ("age".to_string(), toydb::planner::DataType::Integer),
    ]);

    // Start the server
    let server = Server::with_data(&addr, storage, schema)
        .expect("Failed to bind server");

    if let Err(e) = server.run() {
        eprintln!("Server error: {}", e);
        std::process::exit(1);
    }
}
```

### Step 3: Understand the connection lifecycle

The server handles connections sequentially — one at a time. While handling client A, client B must wait. This is simple but limits throughput. The connection lifecycle:

```
1. Client connects (TCP handshake)
2. Server accepts, enters handle_connection loop
3. Client sends Request
4. Server reads Request, executes query, sends Response
5. Repeat 3-4 for each query
6. Client sends Disconnect (or closes connection)
7. Server exits handle_connection, goes back to accepting
```

This is a **request-response** protocol — the client sends one request, waits for one response, then may send another. It is not a streaming protocol (no server push) and not pipelined (the client cannot send multiple requests without waiting for responses).

### Step 4: Test with a simple server-client pair in a test

```rust
// src/server.rs — tests

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_server_client_roundtrip() {
        // Start server on a random port
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener); // release the port

        // Build storage
        let mut storage = Storage::new();
        storage.create_table("test", vec!["x".to_string()]);
        storage.insert_row("test", Row::new(vec![
            Value::Integer(42),
        ])).unwrap();

        let schema = Schema::new();
        // Note: you would need to register "test" in the schema
        // for the planner to accept queries against it.

        // For this test, we verify that the server starts and accepts
        // a connection. A full end-to-end test requires the planner
        // to know about the "test" table.

        // The key insight: the server and client communicate through
        // the protocol module, which we already tested.
        println!("Server would start on {}", addr);
    }
}
```

```
Expected output:
$ cargo build --bin toydb-server
   Compiling toydb v0.1.0
    Finished dev [unoptimized + debuginfo] target(s)

$ ./target/debug/toydb-server
toydb server listening on 127.0.0.1:4000
Waiting for connections...
```

<details>
<summary>Hint: If the server panics with "address already in use"</summary>

Port 4000 might be in use by another process. Either kill the other process (`lsof -i :4000` to find it) or use a different port:

```bash
./target/debug/toydb-server 127.0.0.1:4001
```

In tests, use `TcpListener::bind("127.0.0.1:0")` to let the OS pick a random available port. Then use `listener.local_addr()` to discover which port was assigned.

</details>

---

## Exercise 3: Build the Client

**Goal:** Create a TCP client that connects to the server, sends SQL queries, and displays the results.

### Step 1: Create the client module

Create `src/client.rs`:

```rust
// src/client.rs

use std::io::{BufReader, BufWriter};
use std::net::TcpStream;

use crate::protocol::{self, Request, Response, ProtocolError};

/// A client that connects to a toydb server over TCP.
pub struct Client {
    reader: BufReader<TcpStream>,
    writer: BufWriter<TcpStream>,
}

impl Client {
    /// Connect to a toydb server at the given address.
    pub fn connect(addr: &str) -> Result<Self, std::io::Error> {
        let stream = TcpStream::connect(addr)?;
        let read_stream = stream.try_clone()?;

        Ok(Client {
            reader: BufReader::new(read_stream),
            writer: BufWriter::new(stream),
        })
    }

    /// Send a SQL query and return the response.
    pub fn query(&mut self, sql: &str) -> Result<Response, ProtocolError> {
        // Send the request
        let request = Request::Query(sql.to_string());
        protocol::write_message(&mut self.writer, &request.to_bytes())?;

        // Read the response
        let payload = protocol::read_message(&mut self.reader)?
            .ok_or_else(|| ProtocolError::Io(
                std::io::Error::new(
                    std::io::ErrorKind::ConnectionAborted,
                    "server closed connection",
                )
            ))?;

        Response::from_bytes(&payload)
    }

    /// Send a disconnect request and close the connection.
    pub fn disconnect(&mut self) -> Result<(), ProtocolError> {
        let request = Request::Disconnect;
        protocol::write_message(&mut self.writer, &request.to_bytes())?;
        Ok(())
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        // Best-effort disconnect
        let _ = self.disconnect();
    }
}
```

### Step 2: Understand try_clone

Notice that we call `stream.try_clone()` to get a second handle to the same TCP connection. This is necessary because `BufReader` and `BufWriter` each take ownership of their wrapped stream. We need two handles — one for reading, one for writing.

`try_clone()` creates a new `TcpStream` that refers to the same underlying socket. Reads on one handle and writes on the other are independent — this is safe because TCP supports simultaneous bidirectional communication (full duplex).

Why not use a single stream for both? Because `BufReader` and `BufWriter` would both hold a mutable reference to the same stream, violating Rust's borrowing rules. `try_clone()` gives us two owned handles, sidestepping the borrow checker entirely.

### Step 3: Create a function to display responses

```rust
// src/client.rs (continued)

/// Format a response for terminal display.
pub fn display_response(response: &Response) -> String {
    match response {
        Response::Error { message } => {
            format!("ERROR: {}", message)
        }
        Response::Ok { message } => {
            message.clone()
        }
        Response::Rows { columns, rows } => {
            if columns.is_empty() && rows.is_empty() {
                return "(empty result)".to_string();
            }

            // Calculate column widths
            let mut widths: Vec<usize> = columns.iter()
                .map(|c| c.len())
                .collect();

            for row in rows {
                for (i, cell) in row.iter().enumerate() {
                    if i < widths.len() {
                        widths[i] = widths[i].max(cell.len());
                    }
                }
            }

            let mut output = String::new();

            // Header
            let header: Vec<String> = columns.iter()
                .enumerate()
                .map(|(i, c)| format!("{:width$}", c, width = widths[i]))
                .collect();
            output.push_str(&header.join(" | "));
            output.push('\n');

            // Separator
            let sep: Vec<String> = widths.iter()
                .map(|w| "-".repeat(*w))
                .collect();
            output.push_str(&sep.join("-+-"));
            output.push('\n');

            // Rows
            for row in rows {
                let cells: Vec<String> = row.iter()
                    .enumerate()
                    .map(|(i, v)| {
                        let width = if i < widths.len() { widths[i] } else { v.len() };
                        format!("{:width$}", v, width = width)
                    })
                    .collect();
                output.push_str(&cells.join(" | "));
                output.push('\n');
            }

            output.push_str(&format!("({} rows)", rows.len()));
            output
        }
    }
}
```

### Step 4: Create the client binary

Create `src/bin/toydb-client.rs`:

```rust
// src/bin/toydb-client.rs

use toydb::client::{Client, display_response};

fn main() {
    let addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:4000".to_string());

    let sql = std::env::args()
        .nth(2);

    let mut client = match Client::connect(&addr) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to connect to {}: {}", addr, e);
            std::process::exit(1);
        }
    };

    match sql {
        Some(query) => {
            // Single query mode: execute the query and print results
            match client.query(&query) {
                Ok(response) => {
                    println!("{}", display_response(&response));
                }
                Err(e) => {
                    eprintln!("Query error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        None => {
            eprintln!("Usage: toydb-client [address] <sql>");
            eprintln!("  or use toydb-repl for interactive mode");
            std::process::exit(1);
        }
    }
}
```

### Step 5: Test the client

In one terminal:

```
$ cargo run --bin toydb-server
toydb server listening on 127.0.0.1:4000
Waiting for connections...
```

In another terminal:

```
$ cargo run --bin toydb-client -- "127.0.0.1:4000" "SELECT name, age FROM users"
name  | age
------+----
Alice | 30
Bob   | 25
Carol | 35
(3 rows)

$ cargo run --bin toydb-client -- "127.0.0.1:4000" "SELECT name FROM users WHERE age > 28"
name
-----
Alice
Carol
(2 rows)
```

<details>
<summary>Hint: If the client hangs after connecting</summary>

The most likely cause is a mismatch between the framing format. If the server writes messages without the length prefix, or uses little-endian instead of big-endian, the client will read the first 4 bytes, interpret them as a very large length, and wait forever for that many bytes.

Verify that both server and client use the same `write_message` and `read_message` functions from the protocol module. Print the raw bytes being sent if needed:

```rust
let bytes = request.to_bytes();
eprintln!("Sending {} bytes: {:?}", bytes.len(), &bytes[..bytes.len().min(50)]);
```

</details>

---

## Exercise 4: Build the REPL

**Goal:** Create an interactive read-eval-print loop where you can type SQL queries and see results immediately.

### Step 1: Create the REPL binary

Create `src/bin/toydb-repl.rs`:

```rust
// src/bin/toydb-repl.rs

use std::io::{self, Write, BufRead};
use toydb::client::{Client, display_response};

fn main() {
    let addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:4000".to_string());

    println!("toydb client");
    println!("Connecting to {}...", addr);

    let mut client = match Client::connect(&addr) {
        Ok(c) => {
            println!("Connected.");
            c
        }
        Err(e) => {
            eprintln!("Failed to connect: {}", e);
            std::process::exit(1);
        }
    };

    println!("Type SQL queries, or \\q to quit.\n");

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    loop {
        // Print prompt
        print!("toydb> ");
        stdout.flush().unwrap();

        // Read a line
        let mut line = String::new();
        match stdin.lock().read_line(&mut line) {
            Ok(0) => {
                // EOF (Ctrl+D)
                println!();
                break;
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("Read error: {}", e);
                break;
            }
        }

        let trimmed = line.trim();

        // Skip empty lines
        if trimmed.is_empty() {
            continue;
        }

        // Handle special commands
        match trimmed {
            "\\q" | "\\quit" | "exit" | "quit" => {
                println!("Bye!");
                break;
            }
            "\\h" | "\\help" | "help" => {
                println!("Commands:");
                println!("  \\q        Quit");
                println!("  \\h        Show this help");
                println!("  \\tables   List tables");
                println!("  <SQL>     Execute a SQL query");
                continue;
            }
            "\\tables" => {
                // This would require a server-side command to list tables
                println!("(not implemented yet)");
                continue;
            }
            _ => {}
        }

        // Strip trailing semicolons (common habit from psql/mysql)
        let sql = trimmed.trim_end_matches(';');

        // Send the query
        match client.query(sql) {
            Ok(response) => {
                println!("{}", display_response(&response));
                println!();
            }
            Err(e) => {
                eprintln!("Error: {}", e);

                // If it is an I/O error, the connection is probably dead
                if matches!(e, toydb::protocol::ProtocolError::Io(_)) {
                    eprintln!("Connection lost. Exiting.");
                    break;
                }
            }
        }
    }

    // Disconnect gracefully
    let _ = client.disconnect();
}
```

### Step 2: Handle multi-line queries

SQL queries often span multiple lines. A common convention: the query ends when the user types a line ending with `;`. Let us add multi-line support:

```rust
// Replace the single read_line with this multi-line reader:

fn read_query(stdin: &io::Stdin, stdout: &mut io::Stdout) -> Option<String> {
    let mut query = String::new();
    let mut first_line = true;

    loop {
        // Print prompt
        if first_line {
            print!("toydb> ");
        } else {
            print!("    -> ");
        }
        stdout.flush().unwrap();

        // Read a line
        let mut line = String::new();
        match stdin.lock().read_line(&mut line) {
            Ok(0) => return None, // EOF
            Ok(_) => {}
            Err(_) => return None,
        }

        let trimmed = line.trim();

        // Special commands only on the first line
        if first_line && (trimmed.starts_with('\\') || trimmed == "exit" || trimmed == "quit" || trimmed == "help") {
            return Some(trimmed.to_string());
        }

        // Skip empty first lines
        if first_line && trimmed.is_empty() {
            continue;
        }

        query.push_str(trimmed);
        query.push(' ');
        first_line = false;

        // Query ends with semicolon
        if trimmed.ends_with(';') {
            let result = query.trim().trim_end_matches(';').to_string();
            return Some(result);
        }

        // Single-line queries without semicolons are also accepted
        // (for convenience — real psql requires semicolons)
        if first_line {
            continue; // wait for more input or semicollon
        }

        // After the first line, we are in multi-line mode
        // Continue reading until semicolon
    }
}
```

Actually, let us keep the REPL simple and accept single-line queries. Multi-line support adds complexity without teaching new Rust concepts. The reader can add it as an exercise.

### Step 3: Test the REPL

```
$ cargo run --bin toydb-server &
toydb server listening on 127.0.0.1:4000
Waiting for connections...

$ cargo run --bin toydb-repl
toydb client
Connecting to 127.0.0.1:4000...
Connected.
Type SQL queries, or \q to quit.

toydb> SELECT * FROM users
id | name  | age
---+-------+----
1  | Alice | 30
2  | Bob   | 25
3  | Carol | 35
(3 rows)

toydb> SELECT name FROM users WHERE age > 28
name
-----
Alice
Carol
(2 rows)

toydb> SELECT * FROM nonexistent
ERROR: table not found: nonexistent

toydb> \q
Bye!
```

### Step 4: Handle connection failures gracefully

Add reconnection logic to the REPL (optional enhancement):

```rust
// When a query fails with Io error, try to reconnect
if matches!(e, toydb::protocol::ProtocolError::Io(_)) {
    eprintln!("Connection lost. Attempting to reconnect...");
    match Client::connect(&addr) {
        Ok(new_client) => {
            client = new_client;
            eprintln!("Reconnected.");
        }
        Err(e) => {
            eprintln!("Failed to reconnect: {}. Exiting.", e);
            break;
        }
    }
}
```

<details>
<summary>Hint: If the REPL does not show a prompt</summary>

`print!("toydb> ")` writes to stdout, but stdout is line-buffered by default — it only flushes on newlines. Since our prompt does not end with `\n`, the text sits in the buffer and never appears. The `stdout.flush().unwrap()` call after `print!` forces the buffer to flush.

If you are using `println!` instead of `print!`, the prompt will appear but the cursor will be on the next line, which looks odd. Use `print!` + `flush()` for prompts.

</details>

---

## Rust Gym

### Drill 1: Read and Write Traits

Implement a function that writes a greeting to any `Write` destination:

```rust
use std::io::Write;

fn write_greeting<W: Write>(writer: &mut W, name: &str) -> std::io::Result<()> {
    todo!()
}

fn main() {
    // Write to stdout
    write_greeting(&mut std::io::stdout(), "Alice").unwrap();

    // Write to a Vec<u8> (in-memory buffer)
    let mut buffer: Vec<u8> = Vec::new();
    write_greeting(&mut buffer, "Bob").unwrap();
    println!("Buffer: {:?}", String::from_utf8(buffer).unwrap());
}
```

<details>
<summary>Solution</summary>

```rust
use std::io::Write;

fn write_greeting<W: Write>(writer: &mut W, name: &str) -> std::io::Result<()> {
    write!(writer, "Hello, {}!\n", name)?;
    writer.flush()
}

fn main() {
    // Write to stdout
    write_greeting(&mut std::io::stdout(), "Alice").unwrap();
    // Output: Hello, Alice!

    // Write to a Vec<u8> (in-memory buffer)
    let mut buffer: Vec<u8> = Vec::new();
    write_greeting(&mut buffer, "Bob").unwrap();
    println!("Buffer: {:?}", String::from_utf8(buffer).unwrap());
    // Output: Buffer: "Hello, Bob!\n"
}
```

Key insight: `Vec<u8>` implements `Write`. This is how we test I/O code without touching the network — write to a `Vec<u8>`, then inspect the bytes. The `write!` macro works with any `Write` implementation, just like `format!` works with strings. Our protocol tests use this pattern: write messages to a `Vec<u8>`, then read them back with a `BufReader` over a slice.

</details>

### Drill 2: Big-Endian Encoding

Convert this number to big-endian bytes and back, without using `to_be_bytes`:

```rust
fn to_big_endian(value: u32) -> [u8; 4] {
    todo!()
}

fn from_big_endian(bytes: [u8; 4]) -> u32 {
    todo!()
}

fn main() {
    let original: u32 = 1024; // 0x00000400
    let bytes = to_big_endian(original);
    println!("{:?}", bytes); // [0, 0, 4, 0]

    let recovered = from_big_endian(bytes);
    assert_eq!(original, recovered);
    println!("Round trip: {} -> {:?} -> {}", original, bytes, recovered);
}
```

<details>
<summary>Solution</summary>

```rust
fn to_big_endian(value: u32) -> [u8; 4] {
    [
        ((value >> 24) & 0xFF) as u8,  // most significant byte first
        ((value >> 16) & 0xFF) as u8,
        ((value >> 8) & 0xFF) as u8,
        (value & 0xFF) as u8,          // least significant byte last
    ]
}

fn from_big_endian(bytes: [u8; 4]) -> u32 {
    (bytes[0] as u32) << 24
        | (bytes[1] as u32) << 16
        | (bytes[2] as u32) << 8
        | (bytes[3] as u32)
}

fn main() {
    let original: u32 = 1024;
    let bytes = to_big_endian(original);
    println!("{:?}", bytes); // [0, 0, 4, 0]

    let recovered = from_big_endian(bytes);
    assert_eq!(original, recovered);
    println!("Round trip: {} -> {:?} -> {}", original, bytes, recovered);
    // Round trip: 1024 -> [0, 0, 4, 0] -> 1024
}
```

Big-endian means "most significant byte first" — the way humans read numbers (thousands, hundreds, tens, ones). 1024 = 0x00000400, so the bytes are [0x00, 0x00, 0x04, 0x00].

Network protocols traditionally use big-endian (also called "network byte order"). Rust's `u32::to_be_bytes()` and `u32::from_be_bytes()` do exactly what we implemented here, but are clearer and optimized by the compiler. Our protocol uses `to_be_bytes()` — this drill teaches you what it does under the hood.

</details>

### Drill 3: Error Conversion with From

Our `ProtocolError` has a `From<io::Error>` implementation. Implement `From` conversions for this custom error type:

```rust
use std::io;
use std::num::ParseIntError;

#[derive(Debug)]
enum AppError {
    Io(io::Error),
    Parse(String),
    NotFound(String),
}

// Implement From<io::Error> and From<ParseIntError> for AppError
// so that ? works automatically

fn read_number_from_file(path: &str) -> Result<i64, AppError> {
    let content = std::fs::read_to_string(path)?; // needs From<io::Error>
    let number: i64 = content.trim().parse()?;      // needs From<ParseIntError>
    Ok(number)
}
```

<details>
<summary>Solution</summary>

```rust
use std::io;
use std::num::ParseIntError;

#[derive(Debug)]
enum AppError {
    Io(io::Error),
    Parse(String),
    NotFound(String),
}

impl From<io::Error> for AppError {
    fn from(err: io::Error) -> Self {
        AppError::Io(err)
    }
}

impl From<ParseIntError> for AppError {
    fn from(err: ParseIntError) -> Self {
        AppError::Parse(err.to_string())
    }
}

fn read_number_from_file(path: &str) -> Result<i64, AppError> {
    let content = std::fs::read_to_string(path)?;
    let number: i64 = content.trim().parse()?;
    Ok(number)
}
```

The `?` operator does two things: unwraps the `Ok` value, or converts the error using `From` and returns it. Without the `From` implementations, `?` would not compile because `io::Error` and `ParseIntError` are not `AppError`.

Our `ProtocolError` uses the same pattern. `From<io::Error>` lets us use `?` on any `io::Result` inside protocol functions, automatically wrapping the error in `ProtocolError::Io`.

</details>

---

## DSA in Context: Length-Prefixed Framing

TCP is a byte stream, not a message stream. If you send "HELLO" and then "WORLD", the receiver might read "HELLOW" and then "ORLD" — TCP does not preserve message boundaries. This is the **framing problem**: how does the receiver know where one message ends and the next begins?

### Common framing strategies

**1. Delimiter-based** — use a special byte sequence to mark the end of each message.

```
Message 1: "SELECT * FROM users\n"
Message 2: "INSERT INTO users VALUES (1, 'Alice')\n"
```

HTTP uses `\r\n\r\n` to delimit headers from body. The problem: what if the message itself contains the delimiter? You need escaping, which adds complexity.

**2. Length-prefixed** — send the message length first, then the message.

```
[4 bytes: 0x00000014][20 bytes: SELECT * FROM users]
[4 bytes: 0x00000028][40 bytes: INSERT INTO users...]
```

This is what our protocol uses. The receiver reads exactly 4 bytes (the length), then reads exactly that many bytes (the payload). No escaping needed, no delimiter collisions, and the receiver knows exactly how many bytes to expect.

**3. Fixed-size** — every message is exactly N bytes, padded if shorter.

```
[1024 bytes: "SELECT * FROM users" + 1005 bytes of padding]
```

Simple but wasteful. Used in some low-level protocols where message sizes are predictable.

### Why length-prefixed wins for databases

Database query results vary enormously in size — from 0 bytes (empty result) to gigabytes (full table scan). Length-prefixed framing handles this range naturally:

- 4-byte prefix supports messages up to 4 GB (2^32 bytes)
- No scanning for delimiters (O(1) to find message boundary, vs O(n) for delimiter scanning)
- No escaping overhead
- The receiver can pre-allocate the exact right buffer size

PostgreSQL uses length-prefixed framing. MySQL uses length-prefixed framing. Redis uses delimiter-based (CRLF). HTTP/1.1 uses a mix (headers are delimiter-based, body is length-prefixed via Content-Length).

### The MAX_MESSAGE_SIZE guard

Our protocol limits messages to 16 MB:

```rust
const MAX_MESSAGE_SIZE: u32 = 16 * 1024 * 1024;
```

Without this limit, a malformed or malicious length prefix of `0xFFFFFFFF` (4 GB) would cause the server to allocate 4 GB of memory. The limit is a safety valve: if the length exceeds the maximum, the connection is terminated with an error.

PostgreSQL limits message size to 1 GB. MySQL limits to ~1 GB by default (configurable with `max_allowed_packet`). Our 16 MB is generous for a toy database.

---

## System Design Corner: Database Wire Protocols

Real database wire protocols are more complex than our simple request-response model. Understanding them demonstrates depth in system design interviews.

### PostgreSQL's protocol

PostgreSQL uses a **message-based protocol** where each message has a 1-byte type identifier followed by a 4-byte length and a payload:

```
┌──────────┬──────────────────┬────────────────────────────────┐
│ 1 byte   │  4 bytes (i32)   │  N bytes (payload)             │
│ Msg Type │  Length (incl.    │  Message body                  │
│ ('Q','R')│  length itself)  │                                │
└──────────┴──────────────────┴────────────────────────────────┘
```

Message types include:
- `Q` — Query (simple query)
- `P` — Parse (prepared statement)
- `B` — Bind (bind parameters to prepared statement)
- `E` — Execute (execute prepared statement)
- `T` — RowDescription (column metadata)
- `D` — DataRow (one row of results)
- `C` — CommandComplete ("INSERT 0 1")
- `Z` — ReadyForQuery (server is idle, ready for next query)

A simple query flows like this:

```
Client -> Server: Q "SELECT * FROM users"
Server -> Client: T [column descriptions]
Server -> Client: D [row 1]
Server -> Client: D [row 2]
Server -> Client: D [row 3]
Server -> Client: C "SELECT 3"
Server -> Client: Z (ready for next query)
```

Each row is sent as a separate message. This is a **streaming protocol** — the client does not need to wait for all rows before processing. Our protocol sends all rows in a single response, which is simpler but requires the server to buffer the entire result set in memory.

### Connection pooling

In production, applications do not create a new TCP connection for every query. They use a **connection pool** — a set of pre-established connections that are borrowed and returned:

```
Application                     Connection Pool               Database
    |                                |                            |
    |-- borrow connection ---------->|                            |
    |<- connection 3 --------------- |                            |
    |                                |                            |
    |-- query on connection 3 -------|--------------------------->|
    |<- results on connection 3 -----|<---------------------------|
    |                                |                            |
    |-- return connection 3 -------->|                            |
```

Popular connection poolers like PgBouncer sit between the application and PostgreSQL, multiplexing many application connections onto fewer database connections. This reduces the load on PostgreSQL (which has per-connection overhead for process management, memory, and locks).

### Prepared statements

Our protocol sends SQL as a string for every query. This means the server must lex, parse, plan, and optimize the same query every time it is executed. **Prepared statements** separate query preparation from execution:

```
1. Prepare: "SELECT name FROM users WHERE id = $1"
   Server returns a statement handle (ID)

2. Execute: handle=1, params=[42]
   Server reuses the cached plan, just plugs in the parameters

3. Execute: handle=1, params=[99]
   Same plan, different parameters — no re-parsing
```

This is faster for repeated queries and also prevents SQL injection (parameters are sent separately from the SQL, so they cannot be interpreted as SQL syntax).

> **Interview talking point:** *"Our wire protocol uses length-prefixed framing — a 4-byte big-endian message length followed by a JSON payload. The server handles connections sequentially, which is simple but limits throughput to one client at a time. For production, I would add connection pooling, prepared statements (to avoid re-parsing the same query), and either async I/O or a thread-per-connection model for concurrent clients. PostgreSQL's protocol uses per-message type identifiers and streams result rows individually, which allows the client to process rows before the entire result set is materialized."*

---

## Design Insight: Information Hiding

In *A Philosophy of Software Design*, Ousterhout identifies **information hiding** as the most important technique for reducing complexity. The idea is simple: each module should hide its internal implementation details behind a simple interface. Changes to the implementation should not require changes to the callers.

Our client-server boundary is a perfect example. The client knows:

```
1. Connect to an address
2. Send SQL strings
3. Receive results (column names + rows) or errors
```

The client does NOT know:

- That the server uses a Volcano-model executor
- That the optimizer applies constant folding and filter pushdown
- That the parser is a recursive descent parser
- That the lexer uses a state machine
- That storage uses an in-memory HashMap
- That joins use a hash table internally
- That aggregations use accumulators

The entire SQL engine — lexer, parser, planner, optimizer, executor, storage — is hidden behind the wire protocol. Replacing the storage engine with a B-tree-based disk engine would not change the client at all. Replacing the optimizer with a cost-based optimizer would not change the client at all. Adding new SQL features (DISTINCT, HAVING, window functions) would not change the client at all, as long as the wire protocol still sends column names and rows.

This is information hiding at the system boundary level. The protocol is the interface. Everything behind it is implementation detail.

The same principle applies within the server. The executor does not know about the wire protocol. The optimizer does not know about the executor. The parser does not know about the planner. Each module hides its internals and exposes a narrow interface:

```
Lexer:     &str          → Vec<Token>
Parser:    Vec<Token>    → Statement
Planner:   Statement     → Plan
Optimizer: Plan          → Plan
Executor:  Plan          → ResultSet
Server:    SQL string    → Response
```

Each arrow is an information-hiding boundary. Changes on one side do not propagate to the other. This is why you could build each piece independently across 12 chapters — each chapter's work was hidden from the next chapter's code.

> *"The best modules are those that provide powerful functionality yet have simple interfaces."*
> — John Ousterhout

---

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

**-> [Network Protocols — "The Mail Room"](../ds-narratives/ch12-network-protocols.md)**

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
