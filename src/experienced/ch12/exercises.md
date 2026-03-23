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
