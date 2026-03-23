## Exercise 1: Define the Wire Protocol

**Goal:** Define `Request` and `Response` message types and implement length-prefixed framing for sending and receiving them over TCP.

### Step 1: Create the protocol module

Create `src/protocol.rs` and register it:

```rust
// src/lib.rs -- add this line
pub mod protocol;
```

### Step 2: Define the message types

```rust
// src/protocol.rs

use std::io::{self, Read, Write, BufReader, BufWriter};

/// A request from the client to the server.
///
/// For now, there are only two kinds of requests:
/// - Query: execute some SQL and return results
/// - Disconnect: close the connection gracefully
#[derive(Debug, Clone)]
pub enum Request {
    /// Execute a SQL query and return results.
    Query(String),
    /// Gracefully disconnect.
    Disconnect,
}

/// A response from the server to the client.
///
/// Three possible responses:
/// - Rows: the query produced results (column names + data)
/// - Ok: the query succeeded but produced no results (like INSERT)
/// - Error: something went wrong
#[derive(Debug, Clone)]
pub enum Response {
    /// Query returned rows.
    Rows {
        columns: Vec<String>,
        rows: Vec<Vec<String>>,  // each row is values converted to strings
    },
    /// Query succeeded, no rows returned (INSERT, CREATE TABLE, etc.)
    Ok {
        message: String,
    },
    /// Query failed.
    Error {
        message: String,
    },
}
```

Why `Vec<Vec<String>>` for rows instead of our internal `Row` type? Because the wire protocol should not depend on internal types. The protocol converts all values to strings for transmission. This keeps the protocol module independent from the executor module -- a good separation of concerns.

> **What just happened?**
>
> We defined the language that client and server speak. The client can say "execute this SQL" (Query) or "I am done" (Disconnect). The server can respond with "here are the results" (Rows), "done, no results" (Ok), or "something broke" (Error). These enums are the vocabulary of our protocol.

### Step 3: Implement serialization

We need to convert our messages to bytes for transmission and back. We will use a simple text format that we build manually, without external crates.

```rust
// src/protocol.rs (continued)

impl Request {
    /// Convert this request to bytes for sending over the network.
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

    /// Parse a request from bytes received over the network.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ProtocolError> {
        let s = std::str::from_utf8(bytes)
            .map_err(|_| ProtocolError::InvalidMessage(
                "not valid UTF-8".to_string()
            ))?;

        if s.contains("\"type\":\"disconnect\"") {
            return Ok(Request::Disconnect);
        }

        if let Some(start) = s.find("\"sql\":\"") {
            let sql_start = start + 7; // skip past "sql":"
            let sql = json_unescape(&s[sql_start..])?;
            return Ok(Request::Query(sql));
        }

        Err(ProtocolError::InvalidMessage(
            format!("cannot parse request: {}", s)
        ))
    }
}
```

The `to_bytes` method converts a `Request` into a JSON-like string, then into bytes. The `from_bytes` method does the reverse -- it reads the bytes as a string and figures out which kind of request it is.

The `b"..."` syntax creates a byte slice (`&[u8]`) from a string literal. `.to_vec()` converts it to a `Vec<u8>`.

Now the Response serialization:

```rust
impl Response {
    /// Convert this response to bytes for sending.
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
                format!(
                    "{{\"type\":\"ok\",\"message\":{}}}",
                    json_escape(message),
                )
                .into_bytes()
            }
            Response::Error { message } => {
                format!(
                    "{{\"type\":\"error\",\"message\":{}}}",
                    json_escape(message),
                )
                .into_bytes()
            }
        }
    }

    /// Parse a response from bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ProtocolError> {
        let s = std::str::from_utf8(bytes)
            .map_err(|_| ProtocolError::InvalidMessage(
                "not valid UTF-8".to_string()
            ))?;

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

        Err(ProtocolError::InvalidMessage(
            format!("cannot parse response: {}", s)
        ))
    }
}
```

### Step 4: The error type

```rust
// src/protocol.rs (continued)

/// Errors that can happen during protocol operations.
#[derive(Debug)]
pub enum ProtocolError {
    /// Network I/O error (connection closed, timeout, etc.)
    Io(io::Error),
    /// Message could not be parsed.
    InvalidMessage(String),
    /// Message is too large.
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
            ProtocolError::InvalidMessage(s) => {
                write!(f, "invalid message: {}", s)
            }
            ProtocolError::MessageTooLarge(size) => {
                write!(f, "message too large: {} bytes", size)
            }
        }
    }
}
```

The `From<io::Error>` implementation is important. It tells Rust how to convert an `io::Error` into a `ProtocolError`. This lets us use `?` on I/O operations inside functions that return `Result<_, ProtocolError>` -- the `?` automatically converts the error.

> **What just happened?**
>
> We defined `From<io::Error> for ProtocolError`, which is Rust's conversion trait. When you write `stream.read_exact(&mut buf)?` inside a function returning `Result<_, ProtocolError>`, the `?` operator sees that `read_exact` returns `io::Error` but the function expects `ProtocolError`. It automatically calls `ProtocolError::from(io_error)` to convert. This saves you from writing `.map_err(ProtocolError::Io)?` everywhere.

### Step 5: Implement the framing layer

The framing layer handles length-prefixed messages:

```rust
// src/protocol.rs (continued)

/// Maximum message size: 16 MB.
/// Prevents a malformed length prefix from causing the server
/// to allocate gigabytes of memory.
const MAX_MESSAGE_SIZE: u32 = 16 * 1024 * 1024;

/// Write a length-prefixed message to a stream.
///
/// Format: [4 bytes length (big-endian)] [N bytes payload]
pub fn write_message<W: Write>(
    writer: &mut BufWriter<W>,
    payload: &[u8],
) -> Result<(), ProtocolError> {
    let len = payload.len() as u32;
    if len > MAX_MESSAGE_SIZE {
        return Err(ProtocolError::MessageTooLarge(len as usize));
    }

    // Write the 4-byte length prefix
    writer.write_all(&len.to_be_bytes())?;
    // Write the payload
    writer.write_all(payload)?;
    // Flush to ensure everything is sent
    writer.flush()?;
    Ok(())
}

/// Read a length-prefixed message from a stream.
///
/// Returns None if the connection was closed (the other side hung up).
pub fn read_message<R: Read>(
    reader: &mut BufReader<R>,
) -> Result<Option<Vec<u8>>, ProtocolError> {
    // Read the 4-byte length prefix
    let mut len_bytes = [0u8; 4];
    match reader.read_exact(&mut len_bytes) {
        Ok(()) => {}
        Err(ref e) if e.kind() == io::ErrorKind::UnexpectedEof => {
            return Ok(None);  // Connection closed
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

Let us understand `write_message` step by step:

1. Calculate the payload length and check it is not too large
2. Convert the length to 4 big-endian bytes with `.to_be_bytes()`
3. Write the length bytes to the buffered writer
4. Write the payload bytes
5. Flush to ensure everything is actually sent

And `read_message`:

1. Read exactly 4 bytes (the length prefix)
2. If the read fails with `UnexpectedEof`, the connection was closed -- return `None`
3. Convert the 4 bytes to a `u32` with `from_be_bytes`
4. Check the length is not absurdly large (prevents memory attacks)
5. Allocate a buffer of the right size and read exactly that many bytes

The `vec![0u8; len as usize]` creates a vector of `len` zero bytes. This is the buffer we read the payload into.

> **What just happened?**
>
> We built a framing layer that wraps every message in a 4-byte length prefix. The writer sends: [length][payload]. The reader reads: the length, then exactly that many bytes. This solves the TCP framing problem -- the receiver always knows exactly how many bytes to read for each message. The `BufWriter` batches the length and payload into a single system call when we flush.

### Step 6: JSON helper functions

These functions handle basic JSON string escaping. We build them by hand to avoid depending on an external JSON crate:

```rust
// src/protocol.rs (continued)

/// Wrap a string in quotes and escape special characters for JSON.
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

/// Read a JSON string, handling escape sequences.
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

/// Extract a JSON array of strings.
fn extract_json_string_array(
    json: &str,
    field: &str,
) -> Result<Vec<String>, ProtocolError> {
    let pattern = format!("\"{}\":[", field);
    let start = json.find(&pattern)
        .ok_or_else(|| ProtocolError::InvalidMessage(
            format!("field '{}' not found", field)
        ))?;

    let array_start = start + pattern.len();
    let array_end = json[array_start..].find(']')
        .ok_or_else(|| ProtocolError::InvalidMessage(
            "unterminated array".to_string()
        ))?;

    let array_content = &json[array_start..array_start + array_end];
    if array_content.trim().is_empty() {
        return Ok(vec![]);
    }

    let mut items = Vec::new();
    let mut remaining = array_content;
    while let Some(quote_start) = remaining.find('"') {
        let after_quote = &remaining[quote_start + 1..];
        let value = json_unescape(after_quote)?;
        items.push(value.clone());
        // Skip past the closing quote and any comma
        let skip = quote_start + 1 + value.len() + 1; // opening quote + content + closing quote
        if skip < remaining.len() {
            remaining = &remaining[skip..];
        } else {
            break;
        }
    }
    Ok(items)
}

/// Extract rows from JSON (simplified parser).
fn extract_json_rows(json: &str) -> Result<Vec<Vec<String>>, ProtocolError> {
    let pattern = "\"rows\":[";
    let start = json.find(pattern)
        .ok_or_else(|| ProtocolError::InvalidMessage(
            "rows field not found".to_string()
        ))?;

    let content = &json[start + pattern.len()..];
    let mut rows = Vec::new();
    let mut remaining = content;

    while let Some(row_start) = remaining.find('[') {
        let row_content = &remaining[row_start + 1..];
        let row_end = row_content.find(']')
            .ok_or_else(|| ProtocolError::InvalidMessage(
                "unterminated row".to_string()
            ))?;

        let row_str = &row_content[..row_end];
        let mut values = Vec::new();

        let mut cell_remaining = row_str;
        while let Some(q) = cell_remaining.find('"') {
            let after = &cell_remaining[q + 1..];
            let val = json_unescape(after)?;
            values.push(val.clone());
            let skip = q + 1 + val.len() + 1;
            if skip < cell_remaining.len() {
                cell_remaining = &cell_remaining[skip..];
            } else {
                break;
            }
        }

        if !values.is_empty() {
            rows.push(values);
        }

        remaining = &row_content[row_end + 1..];

        // Stop at the closing bracket of the rows array
        if remaining.starts_with(']') || remaining.starts_with(",]") {
            break;
        }
    }

    Ok(rows)
}
```

These JSON helpers are intentionally simple. A production database would use a proper serialization library (like `serde`), but building it by hand teaches you how serialization works at the byte level.

### Step 7: Test the protocol

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_roundtrip() {
        // Serialize and deserialize a query request
        let request = Request::Query("SELECT * FROM users".to_string());
        let bytes = request.to_bytes();
        let parsed = Request::from_bytes(&bytes).unwrap();

        match parsed {
            Request::Query(sql) => {
                assert_eq!(sql, "SELECT * FROM users");
            }
            _ => panic!("Expected Query"),
        }
    }

    #[test]
    fn test_disconnect_roundtrip() {
        let request = Request::Disconnect;
        let bytes = request.to_bytes();
        let parsed = Request::from_bytes(&bytes).unwrap();

        match parsed {
            Request::Disconnect => {} // correct
            _ => panic!("Expected Disconnect"),
        }
    }

    #[test]
    fn test_response_ok_roundtrip() {
        let response = Response::Ok {
            message: "1 row inserted".to_string(),
        };
        let bytes = response.to_bytes();
        let parsed = Response::from_bytes(&bytes).unwrap();

        match parsed {
            Response::Ok { message } => {
                assert_eq!(message, "1 row inserted");
            }
            _ => panic!("Expected Ok"),
        }
    }

    #[test]
    fn test_response_error_roundtrip() {
        let response = Response::Error {
            message: "table not found: unicorns".to_string(),
        };
        let bytes = response.to_bytes();
        let parsed = Response::from_bytes(&bytes).unwrap();

        match parsed {
            Response::Error { message } => {
                assert_eq!(message, "table not found: unicorns");
            }
            _ => panic!("Expected Error"),
        }
    }

    #[test]
    fn test_framing_roundtrip() {
        // Test that write_message + read_message preserves data
        let payload = b"Hello, database!";

        // Write to an in-memory buffer
        let mut buffer: Vec<u8> = Vec::new();
        {
            let mut writer = BufWriter::new(&mut buffer);
            write_message(&mut writer, payload).unwrap();
        }

        // Read back from the buffer
        let mut reader = BufReader::new(&buffer[..]);
        let result = read_message(&mut reader).unwrap().unwrap();

        assert_eq!(result, payload);
    }
}
```

```
$ cargo test protocol::tests
running 5 tests
test protocol::tests::test_request_roundtrip ... ok
test protocol::tests::test_disconnect_roundtrip ... ok
test protocol::tests::test_response_ok_roundtrip ... ok
test protocol::tests::test_response_error_roundtrip ... ok
test protocol::tests::test_framing_roundtrip ... ok

test result: ok. 5 passed; 0 failed
```

> **Common Mistakes**
>
> 1. **Forgetting to flush**: `BufWriter` buffers data. If you do not call `flush()`, the data might sit in the buffer and never be sent. Always flush after writing a complete message.
>
> 2. **Not handling `UnexpectedEof`**: When the remote side closes the connection, `read_exact` returns an `UnexpectedEof` error. This is not a bug -- it means the connection ended. Treat it as `None` (no more messages).

---

## Exercise 2: Build the Server

**Goal:** Build a TCP server that listens for connections, reads SQL queries, executes them, and sends back results.

### Step 1: The server function

```rust
// src/server.rs

use std::net::TcpListener;
use std::io::{BufReader, BufWriter};
use crate::protocol::{Request, Response, read_message, write_message, ProtocolError};
use crate::executor::{Storage, ScanExecutor, build_executor, collect_all, Value, Row};

/// Start the database server on the given address.
///
/// This function runs forever (until the program is stopped).
/// It accepts connections one at a time and processes queries.
pub fn start_server(addr: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Create the storage and set up some sample data
    let mut storage = Storage::new();
    setup_sample_data(&mut storage);

    // Start listening for connections
    let listener = TcpListener::bind(addr)?;
    println!("ToyDB server listening on {}", addr);

    // Accept connections in a loop
    for stream in listener.incoming() {
        let stream = stream?;
        let peer = stream.peer_addr()?;
        println!("Client connected: {}", peer);

        // Handle this connection
        match handle_connection(stream, &mut storage) {
            Ok(()) => println!("Client {} disconnected", peer),
            Err(e) => println!("Error with client {}: {}", peer, e),
        }
    }

    Ok(())
}

/// Set up some sample tables and data for testing.
fn setup_sample_data(storage: &mut Storage) {
    storage.create_table("users", vec![
        "id".to_string(),
        "name".to_string(),
        "age".to_string(),
    ]);

    let users = vec![
        vec![Value::Integer(1), Value::String("Alice".to_string()), Value::Integer(30)],
        vec![Value::Integer(2), Value::String("Bob".to_string()), Value::Integer(25)],
        vec![Value::Integer(3), Value::String("Carol".to_string()), Value::Integer(35)],
    ];

    for values in users {
        storage.insert_row("users", Row::new(values)).unwrap();
    }
}
```

### Step 2: Handle a single connection

```rust
// src/server.rs (continued)

use std::net::TcpStream;

/// Handle a single client connection.
///
/// Reads requests in a loop, executes them, and sends responses.
/// Returns when the client disconnects.
fn handle_connection(
    stream: TcpStream,
    storage: &mut Storage,
) -> Result<(), ProtocolError> {
    // Wrap the stream in buffered reader and writer.
    // We need to clone the stream because BufReader and BufWriter
    // each need their own reference.
    let reader_stream = stream.try_clone()
        .map_err(|e| ProtocolError::Io(e))?;
    let mut reader = BufReader::new(reader_stream);
    let mut writer = BufWriter::new(stream);

    loop {
        // Read the next request
        let message = match read_message(&mut reader)? {
            Some(bytes) => bytes,
            None => return Ok(()),  // Connection closed
        };

        // Parse the request
        let request = Request::from_bytes(&message)?;

        match request {
            Request::Disconnect => {
                // Send OK and close
                let response = Response::Ok {
                    message: "Goodbye!".to_string(),
                };
                write_message(&mut writer, &response.to_bytes())?;
                return Ok(());
            }

            Request::Query(sql) => {
                // Execute the query and build a response
                let response = execute_query(&sql, storage);
                write_message(&mut writer, &response.to_bytes())?;
            }
        }
    }
}
```

The `stream.try_clone()` creates a second handle to the same TCP connection. We need this because `BufReader` takes ownership of its inner stream, and we also need the stream for `BufWriter`. Cloning the stream handle gives us two independent handles that both refer to the same underlying connection.

### Step 3: Execute a query

This function takes a SQL string, processes it through the full pipeline, and returns a `Response`:

```rust
// src/server.rs (continued)

/// Execute a SQL query and return a Response.
///
/// This is the full pipeline:
/// 1. Parse the SQL (in a real implementation)
/// 2. Plan the query
/// 3. Optimize the plan
/// 4. Execute and collect results
///
/// For simplicity, we handle a few hardcoded queries here.
/// In a real implementation, this would call the lexer, parser,
/// planner, optimizer, and executor.
fn execute_query(sql: &str, storage: &Storage) -> Response {
    let sql_lower = sql.trim().to_lowercase();

    // Handle SELECT * FROM <table>
    if sql_lower.starts_with("select * from ") {
        let table_name = sql_lower
            .strip_prefix("select * from ")
            .unwrap()
            .trim()
            .trim_end_matches(';');

        match ScanExecutor::new(storage, table_name) {
            Ok(mut scan) => {
                let columns = scan.columns().to_vec();
                let mut rows = Vec::new();

                loop {
                    match scan.next() {
                        Ok(Some(row)) => {
                            let string_row: Vec<String> = row.values.iter()
                                .map(|v| format!("{}", v))
                                .collect();
                            rows.push(string_row);
                        }
                        Ok(None) => break,
                        Err(e) => {
                            return Response::Error {
                                message: format!("{}", e),
                            };
                        }
                    }
                }

                Response::Rows { columns, rows }
            }
            Err(e) => Response::Error {
                message: format!("{}", e),
            },
        }
    } else {
        Response::Error {
            message: format!("unsupported query: {}", sql),
        }
    }
}
```

This is a simplified query handler that only supports `SELECT * FROM <table>`. A full implementation would feed the SQL through the lexer, parser, planner, and optimizer from earlier chapters. The point here is to demonstrate the server-side flow: receive SQL, execute it, format the results, send them back.

> **What just happened?**
>
> The server loop is simple: read a message, parse it as a Request, execute the query, build a Response, send it back. Repeat until the client disconnects. Each step uses the types and functions we defined in the protocol module. The `execute_query` function is the bridge between networking and the database engine.

### Step 4: Register the server module

```rust
// src/lib.rs -- add this
pub mod server;
```

---

## Exercise 3: Build the Client

**Goal:** Build a TCP client that connects to the server, sends SQL queries, and prints results.

### Step 1: The client struct

```rust
// src/client.rs

use std::net::TcpStream;
use std::io::{BufReader, BufWriter};
use crate::protocol::{Request, Response, read_message, write_message, ProtocolError};

/// A database client that connects to a ToyDB server.
pub struct Client {
    reader: BufReader<TcpStream>,
    writer: BufWriter<TcpStream>,
}

impl Client {
    /// Connect to a ToyDB server at the given address.
    ///
    /// Returns an error if the connection cannot be established
    /// (server not running, wrong address, etc.)
    pub fn connect(addr: &str) -> Result<Self, ProtocolError> {
        let stream = TcpStream::connect(addr)
            .map_err(|e| ProtocolError::Io(e))?;

        let reader_stream = stream.try_clone()
            .map_err(|e| ProtocolError::Io(e))?;

        Ok(Client {
            reader: BufReader::new(reader_stream),
            writer: BufWriter::new(stream),
        })
    }

    /// Send a SQL query to the server and return the response.
    pub fn query(&mut self, sql: &str) -> Result<Response, ProtocolError> {
        // Build and send the request
        let request = Request::Query(sql.to_string());
        write_message(&mut self.writer, &request.to_bytes())?;

        // Read the response
        let response_bytes = read_message(&mut self.reader)?
            .ok_or_else(|| ProtocolError::InvalidMessage(
                "server closed connection".to_string()
            ))?;

        Response::from_bytes(&response_bytes)
    }

    /// Send a disconnect message to the server.
    pub fn disconnect(&mut self) -> Result<(), ProtocolError> {
        let request = Request::Disconnect;
        write_message(&mut self.writer, &request.to_bytes())?;

        // Read the goodbye response
        let _ = read_message(&mut self.reader)?;
        Ok(())
    }
}
```

The `Client` struct holds a reader and writer, both wrapping the same TCP connection. The `query` method:

1. Builds a `Request::Query` with the SQL string
2. Serializes it to bytes
3. Sends it using `write_message` (length-prefixed framing)
4. Reads the response using `read_message`
5. Deserializes the bytes back into a `Response`

### Step 2: Pretty-print results

```rust
// src/client.rs (continued)

/// Print a Response in a nice table format.
pub fn print_response(response: &Response) {
    match response {
        Response::Rows { columns, rows } => {
            if rows.is_empty() {
                println!("(0 rows)");
                return;
            }

            // Calculate column widths
            let mut widths: Vec<usize> = columns.iter()
                .map(|c| c.len())
                .collect();

            for row in rows {
                for (i, val) in row.iter().enumerate() {
                    if i < widths.len() {
                        widths[i] = widths[i].max(val.len());
                    }
                }
            }

            // Print header
            let header: Vec<String> = columns.iter().enumerate()
                .map(|(i, c)| format!("{:width$}", c, width = widths[i]))
                .collect();
            println!(" {} ", header.join(" | "));

            // Print separator
            let sep: Vec<String> = widths.iter()
                .map(|w| "-".repeat(*w))
                .collect();
            println!("-{}-", sep.join("-+-"));

            // Print rows
            for row in rows {
                let cells: Vec<String> = row.iter().enumerate()
                    .map(|(i, v)| {
                        let width = if i < widths.len() { widths[i] } else { v.len() };
                        format!("{:width$}", v, width = width)
                    })
                    .collect();
                println!(" {} ", cells.join(" | "));
            }

            println!("({} rows)", rows.len());
        }

        Response::Ok { message } => {
            println!("{}", message);
        }

        Response::Error { message } => {
            println!("ERROR: {}", message);
        }
    }
}
```

This produces output like:

```
 id | name  | age
----+-------+----
 1  | Alice | 30
 2  | Bob   | 25
 3  | Carol | 35
(3 rows)
```

The formatting uses `format!("{:width$}", value, width = n)` to pad each value to a consistent width. The `width$` syntax tells Rust to use a variable for the format width.

> **What just happened?**
>
> We built a client that speaks the same protocol as our server. It sends length-prefixed messages containing serialized Requests, and reads length-prefixed messages containing serialized Responses. The `print_response` function formats the results as a readable table. The client and server can be on different machines -- they communicate through TCP over the network.

### Step 3: Register the client module

```rust
// src/lib.rs -- add this
pub mod client;
```

---

## Exercise 4: Build the REPL

**Goal:** Build an interactive SQL prompt that lets you type queries and see results, like `psql` or `mysql`.

### Step 1: Understand the REPL pattern

REPL stands for **Read-Eval-Print Loop**:

1. **Read** -- read a line of input from the user
2. **Eval** -- send it to the server for evaluation (execution)
3. **Print** -- print the results
4. **Loop** -- go back to step 1

```rust
// src/bin/repl.rs

use std::io::{self, Write, BufRead};
use toydb::client::{Client, print_response};

fn main() {
    // Parse command-line arguments for the server address
    let addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:4000".to_string());

    // Connect to the server
    println!("Connecting to {}...", addr);
    let mut client = match Client::connect(&addr) {
        Ok(c) => {
            println!("Connected!");
            c
        }
        Err(e) => {
            eprintln!("Failed to connect: {}", e);
            std::process::exit(1);
        }
    };

    // Print a welcome message
    println!("ToyDB interactive SQL prompt");
    println!("Type SQL queries, or 'quit' to exit.");
    println!();

    // The REPL loop
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    loop {
        // Print the prompt
        print!("toydb> ");
        stdout.flush().unwrap();

        // Read a line of input
        let mut line = String::new();
        match stdin.lock().read_line(&mut line) {
            Ok(0) => {
                // EOF (Ctrl-D on Unix, Ctrl-Z on Windows)
                println!();
                break;
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("Error reading input: {}", e);
                break;
            }
        }

        let trimmed = line.trim();

        // Skip empty lines
        if trimmed.is_empty() {
            continue;
        }

        // Handle special commands
        if trimmed.eq_ignore_ascii_case("quit")
            || trimmed.eq_ignore_ascii_case("exit")
            || trimmed.eq_ignore_ascii_case("\\q")
        {
            println!("Goodbye!");
            let _ = client.disconnect();
            break;
        }

        // Send the query to the server
        match client.query(trimmed) {
            Ok(response) => print_response(&response),
            Err(e) => eprintln!("Error: {}", e),
        }

        println!();  // blank line between results
    }
}
```

Let us understand the key parts:

1. **`print!("toydb> ")`** -- prints the prompt without a newline. We use `print!` instead of `println!` because we want the cursor to stay on the same line as the prompt.

2. **`stdout.flush().unwrap()`** -- flushes the prompt to the screen. Without this, the prompt might not appear because `print!` output is buffered.

3. **`stdin.lock().read_line(&mut line)`** -- reads one line of input. `stdin.lock()` gets a locked handle to standard input, which is more efficient than locking for each operation. Returns `Ok(0)` when the user types Ctrl-D (end of file).

4. **`eq_ignore_ascii_case`** -- compares strings ignoring case. So "QUIT", "quit", "Quit" all work.

### Step 2: Build the server binary

```rust
// src/bin/server.rs

use toydb::server::start_server;

fn main() {
    let addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:4000".to_string());

    println!("Starting ToyDB server on {}...", addr);

    if let Err(e) = start_server(&addr) {
        eprintln!("Server error: {}", e);
        std::process::exit(1);
    }
}
```

### Step 3: Try it out

Open two terminal windows:

```
# Terminal 1: Start the server
$ cargo run --bin server
Starting ToyDB server on 127.0.0.1:4000...
ToyDB server listening on 127.0.0.1:4000

# Terminal 2: Start the client
$ cargo run --bin repl
Connecting to 127.0.0.1:4000...
Connected!
ToyDB interactive SQL prompt
Type SQL queries, or 'quit' to exit.

toydb> SELECT * FROM users;
 id | name  | age
----+-------+----
 1  | Alice | 30
 2  | Bob   | 25
 3  | Carol | 35
(3 rows)

toydb> quit
Goodbye!
```

> **What just happened?**
>
> You have a working database client and server. The client sends SQL over TCP, the server executes it and sends results back. The REPL lets you interact with the database interactively. This is exactly how real databases like PostgreSQL work -- `psql` is a REPL that connects to the PostgreSQL server over TCP.

> **Common Mistakes**
>
> 1. **Starting the client before the server**: If the server is not running, `TcpStream::connect` will fail with "connection refused." Always start the server first.
>
> 2. **Forgetting to flush stdout**: After `print!("toydb> ")`, you must call `stdout.flush()`. Otherwise, the prompt might not appear until the next `println!`. This is because `print!` output is line-buffered on most systems.
>
> 3. **Not handling Ctrl-D**: When the user types Ctrl-D, `read_line` returns `Ok(0)`. If you do not check for this, the REPL would loop forever printing empty prompts.

---

## Exercise 5: End-to-End Test (Challenge)

**Goal:** Write a test that starts a server in a background thread, connects a client, sends a query, and verifies the response.

<details>
<summary>Hint 1: Using threads for testing</summary>

```rust
use std::thread;
use std::time::Duration;

#[test]
fn test_client_server() {
    // Start the server in a background thread
    let server_thread = thread::spawn(|| {
        start_server("127.0.0.1:4001").unwrap();
    });

    // Give the server a moment to start
    thread::sleep(Duration::from_millis(100));

    // Connect a client
    let mut client = Client::connect("127.0.0.1:4001").unwrap();

    // Send a query
    let response = client.query("SELECT * FROM users").unwrap();

    // Verify the response
    match response {
        Response::Rows { columns, rows } => {
            assert_eq!(columns.len(), 3);
            assert_eq!(rows.len(), 3);
        }
        _ => panic!("Expected Rows response"),
    }

    // Disconnect
    client.disconnect().unwrap();
}
```

</details>

<details>
<summary>Hint 2: Choosing a port for tests</summary>

Use a different port for each test (e.g., 4001, 4002, ...) to avoid conflicts when tests run in parallel. You can also use port 0, which tells the OS to assign a random available port -- though you then need to find out which port was assigned using `listener.local_addr()`.

</details>

---

## Key Takeaways

1. **TCP is a byte stream, not a message stream.** Length-prefixed framing (4 bytes of length, then the payload) tells the receiver exactly how many bytes to read for each message.

2. **`BufReader` and `BufWriter` reduce system calls.** Wrapping your streams in buffers is almost always the right thing to do. Multiple small writes become one large write.

3. **The `From` trait enables automatic error conversion.** Implementing `From<io::Error> for ProtocolError` lets you use `?` on I/O operations in functions returning `Result<_, ProtocolError>`.

4. **Separate the protocol from the implementation.** The wire protocol (Request/Response types, framing) is independent of how queries are executed. The server converts between protocol types and internal types at the boundary.

5. **The REPL is the user interface.** It is the simplest possible front end: read a line, send it, print the result. But it makes your database feel like a real product.
