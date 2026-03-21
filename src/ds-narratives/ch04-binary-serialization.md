# Binary Serialization — "Turning structs into bytes"

Your database stores rich values -- integers, strings, booleans, nulls. In memory, these live as Rust enums with nice type safety. But the moment you need to write them to disk or send them over a network, you hit a wall: files and sockets only understand bytes. Raw, flat, unstructured bytes.

You need a protocol. A way to encode `Value::Integer(42)` into a sequence of bytes that you can later decode back into exactly `Value::Integer(42)`. Not `Value::String("42")`. Not `Value::Float(42.0)`. The exact original type and value. One bit wrong and your database corrupts silently.

Let's build a binary serialization format from scratch.

---

## The Naive Way

The simplest idea: just convert everything to strings and store them as text.

```rust
fn main() {
    // Store values as text
    let entries: Vec<String> = vec![
        "integer:42".to_string(),
        "string:hello world".to_string(),
        "bool:true".to_string(),
        "null:".to_string(),
    ];

    // Parse them back
    for entry in &entries {
        if let Some(rest) = entry.strip_prefix("integer:") {
            let n: i64 = rest.parse().unwrap();
            println!("Got integer: {}", n);
        } else if let Some(rest) = entry.strip_prefix("string:") {
            println!("Got string: {}", rest);
        } else if let Some(rest) = entry.strip_prefix("bool:") {
            let b: bool = rest.parse().unwrap();
            println!("Got bool: {}", b);
        } else if entry == "null:" {
            println!("Got null");
        }
    }
}
```

This works until it does not. What if a string contains a colon? What if it contains a newline? How do you know where one value ends and the next begins? Text serialization forces you into an endless game of escaping special characters. And it wastes space -- the integer `42` takes 2 bytes as text but only 8 bytes (fixed) as a binary i64, and more importantly, every integer takes exactly 8 bytes, so you can seek to any position without scanning.

The real cost is parsing. Converting "42" from ASCII to an integer requires multiplication and addition for every digit. Converting 8 raw bytes to an i64 requires one memory copy. At millions of operations per second, that difference matters.

---

## The Insight

Think about how a shipping company packs a container. They do not throw items in loose. Each item gets a label: what type it is, how big it is, and then the item itself. When the container arrives, the receiver reads the label, knows exactly how many bytes to read for the payload, unpacks it, and moves on to the next label.

Binary serialization works the same way. Every value starts with a **type tag** -- a single byte that says "I am an integer" or "I am a string." For fixed-size types like integers and booleans, the decoder already knows how many bytes follow. For variable-size types like strings, we add a **length prefix** -- a number that says how many bytes the payload occupies.

The format for each value:

```text
[type_tag: 1 byte] [length: 4 bytes, only for variable types] [payload: N bytes]
```

This is exactly how real database wire protocols work. PostgreSQL's binary format uses type OIDs and length prefixes. MySQL's binary protocol does the same. We are building the core of what makes database communication possible.

---

## The Build

### The Value Type

First, define the types our database supports:

```rust
#[derive(Debug, Clone, PartialEq)]
enum Value {
    Null,
    Boolean(bool),
    Integer(i64),
    Float(f64),
    String(String),
    Bytes(Vec<u8>),
}
```

### Type Tags

Each variant gets a unique byte tag:

```rust
const TAG_NULL: u8 = 0;
const TAG_BOOLEAN: u8 = 1;
const TAG_INTEGER: u8 = 2;
const TAG_FLOAT: u8 = 3;
const TAG_STRING: u8 = 4;
const TAG_BYTES: u8 = 5;
```

### The Encoder

Encoding converts a `Value` into a `Vec<u8>`. Every encoded value starts with its tag byte, followed by the payload.

```rust
fn encode(value: &Value) -> Vec<u8> {
    let mut buf = Vec::new();

    match value {
        Value::Null => {
            buf.push(TAG_NULL);
            // No payload -- null is just the tag
        }
        Value::Boolean(b) => {
            buf.push(TAG_BOOLEAN);
            buf.push(if *b { 1 } else { 0 });
        }
        Value::Integer(n) => {
            buf.push(TAG_INTEGER);
            buf.extend_from_slice(&n.to_le_bytes());
        }
        Value::Float(f) => {
            buf.push(TAG_FLOAT);
            buf.extend_from_slice(&f.to_le_bytes());
        }
        Value::String(s) => {
            buf.push(TAG_STRING);
            let bytes = s.as_bytes();
            // Length prefix: 4 bytes, little-endian
            buf.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
            buf.extend_from_slice(bytes);
        }
        Value::Bytes(data) => {
            buf.push(TAG_BYTES);
            buf.extend_from_slice(&(data.len() as u32).to_le_bytes());
            buf.extend_from_slice(data);
        }
    }

    buf
}
```

The key decisions here:

- **Little-endian** byte order (`to_le_bytes()`). We pick one and stick with it. Most modern CPUs are little-endian, so this avoids byte-swapping on common hardware. Network protocols traditionally use big-endian, but disk storage does not have that convention.
- **4-byte length prefix** for variable types. A `u32` supports strings up to 4 GB. That is more than enough for a database value.
- **Fixed sizes** for primitives. An `i64` always emits exactly 8 bytes. A `bool` always emits exactly 1 byte. The decoder never has to guess.

### The Decoder

Decoding reads bytes and reconstructs values. It needs to track its position in the byte stream:

```rust
struct Decoder<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Decoder<'a> {
    fn new(data: &'a [u8]) -> Self {
        Decoder { data, pos: 0 }
    }

    fn read_byte(&mut self) -> Result<u8, String> {
        if self.pos >= self.data.len() {
            return Err("unexpected end of data".to_string());
        }
        let byte = self.data[self.pos];
        self.pos += 1;
        Ok(byte)
    }

    fn read_bytes(&mut self, n: usize) -> Result<&'a [u8], String> {
        if self.pos + n > self.data.len() {
            return Err(format!(
                "need {} bytes but only {} remain",
                n,
                self.data.len() - self.pos
            ));
        }
        let slice = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(slice)
    }

    fn read_u32(&mut self) -> Result<u32, String> {
        let bytes = self.read_bytes(4)?;
        Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
    }

    fn read_i64(&mut self) -> Result<i64, String> {
        let bytes = self.read_bytes(8)?;
        Ok(i64::from_le_bytes(bytes.try_into().unwrap()))
    }

    fn read_f64(&mut self) -> Result<f64, String> {
        let bytes = self.read_bytes(8)?;
        Ok(f64::from_le_bytes(bytes.try_into().unwrap()))
    }

    fn decode_value(&mut self) -> Result<Value, String> {
        let tag = self.read_byte()?;

        match tag {
            TAG_NULL => Ok(Value::Null),
            TAG_BOOLEAN => {
                let b = self.read_byte()?;
                Ok(Value::Boolean(b != 0))
            }
            TAG_INTEGER => {
                let n = self.read_i64()?;
                Ok(Value::Integer(n))
            }
            TAG_FLOAT => {
                let f = self.read_f64()?;
                Ok(Value::Float(f))
            }
            TAG_STRING => {
                let len = self.read_u32()? as usize;
                let bytes = self.read_bytes(len)?;
                let s = std::str::from_utf8(bytes)
                    .map_err(|e| format!("invalid UTF-8: {}", e))?;
                Ok(Value::String(s.to_string()))
            }
            TAG_BYTES => {
                let len = self.read_u32()? as usize;
                let bytes = self.read_bytes(len)?;
                Ok(Value::Bytes(bytes.to_vec()))
            }
            _ => Err(format!("unknown type tag: {}", tag)),
        }
    }
}
```

Notice the error handling. Every read checks bounds first. A corrupted or truncated byte stream returns an error instead of panicking. In a real database, this is essential -- you will encounter corrupted data, and crashing is not an option.

### Encoding Rows

A database row is a sequence of values. We encode a row by encoding each value in order, prefixed with the column count:

```rust
fn encode_row(values: &[Value]) -> Vec<u8> {
    let mut buf = Vec::new();
    // Column count as u16 -- supports up to 65,535 columns
    buf.extend_from_slice(&(values.len() as u16).to_le_bytes());
    for value in values {
        buf.extend_from_slice(&encode(value));
    }
    buf
}

fn decode_row(data: &[u8]) -> Result<Vec<Value>, String> {
    let mut decoder = Decoder::new(data);
    let col_count = {
        let bytes = decoder.read_bytes(2)?;
        u16::from_le_bytes(bytes.try_into().unwrap()) as usize
    };

    let mut values = Vec::with_capacity(col_count);
    for _ in 0..col_count {
        values.push(decoder.decode_value()?);
    }

    if decoder.pos != decoder.data.len() {
        return Err(format!(
            "trailing bytes: {} bytes remain after decoding {} columns",
            decoder.data.len() - decoder.pos,
            col_count
        ));
    }

    Ok(values)
}
```

The trailing bytes check is subtle but important. If we decoded all columns but there are leftover bytes, something is wrong -- either the data is corrupted or the column count was wrong. Catching this early prevents silent data corruption.

---

## The Payoff

Here is the full, runnable implementation:

```rust
#[derive(Debug, Clone, PartialEq)]
enum Value {
    Null,
    Boolean(bool),
    Integer(i64),
    Float(f64),
    String(String),
    Bytes(Vec<u8>),
}

const TAG_NULL: u8 = 0;
const TAG_BOOLEAN: u8 = 1;
const TAG_INTEGER: u8 = 2;
const TAG_FLOAT: u8 = 3;
const TAG_STRING: u8 = 4;
const TAG_BYTES: u8 = 5;

fn encode(value: &Value) -> Vec<u8> {
    let mut buf = Vec::new();
    match value {
        Value::Null => { buf.push(TAG_NULL); }
        Value::Boolean(b) => { buf.push(TAG_BOOLEAN); buf.push(if *b { 1 } else { 0 }); }
        Value::Integer(n) => { buf.push(TAG_INTEGER); buf.extend_from_slice(&n.to_le_bytes()); }
        Value::Float(f) => { buf.push(TAG_FLOAT); buf.extend_from_slice(&f.to_le_bytes()); }
        Value::String(s) => {
            buf.push(TAG_STRING);
            let bytes = s.as_bytes();
            buf.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
            buf.extend_from_slice(bytes);
        }
        Value::Bytes(data) => {
            buf.push(TAG_BYTES);
            buf.extend_from_slice(&(data.len() as u32).to_le_bytes());
            buf.extend_from_slice(data);
        }
    }
    buf
}

struct Decoder<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Decoder<'a> {
    fn new(data: &'a [u8]) -> Self { Decoder { data, pos: 0 } }
    fn read_byte(&mut self) -> Result<u8, String> {
        if self.pos >= self.data.len() { return Err("unexpected end of data".into()); }
        let b = self.data[self.pos]; self.pos += 1; Ok(b)
    }
    fn read_bytes(&mut self, n: usize) -> Result<&'a [u8], String> {
        if self.pos + n > self.data.len() {
            return Err(format!("need {} bytes, {} remain", n, self.data.len() - self.pos));
        }
        let s = &self.data[self.pos..self.pos + n]; self.pos += n; Ok(s)
    }
    fn read_u32(&mut self) -> Result<u32, String> {
        let b = self.read_bytes(4)?; Ok(u32::from_le_bytes(b.try_into().unwrap()))
    }
    fn read_i64(&mut self) -> Result<i64, String> {
        let b = self.read_bytes(8)?; Ok(i64::from_le_bytes(b.try_into().unwrap()))
    }
    fn read_f64(&mut self) -> Result<f64, String> {
        let b = self.read_bytes(8)?; Ok(f64::from_le_bytes(b.try_into().unwrap()))
    }
    fn decode_value(&mut self) -> Result<Value, String> {
        let tag = self.read_byte()?;
        match tag {
            TAG_NULL => Ok(Value::Null),
            TAG_BOOLEAN => Ok(Value::Boolean(self.read_byte()? != 0)),
            TAG_INTEGER => Ok(Value::Integer(self.read_i64()?)),
            TAG_FLOAT => Ok(Value::Float(self.read_f64()?)),
            TAG_STRING => {
                let len = self.read_u32()? as usize;
                let bytes = self.read_bytes(len)?;
                let s = std::str::from_utf8(bytes).map_err(|e| format!("bad utf8: {}", e))?;
                Ok(Value::String(s.to_string()))
            }
            TAG_BYTES => {
                let len = self.read_u32()? as usize;
                let bytes = self.read_bytes(len)?;
                Ok(Value::Bytes(bytes.to_vec()))
            }
            _ => Err(format!("unknown tag: {}", tag)),
        }
    }
}

fn encode_row(values: &[Value]) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&(values.len() as u16).to_le_bytes());
    for v in values { buf.extend_from_slice(&encode(v)); }
    buf
}

fn decode_row(data: &[u8]) -> Result<Vec<Value>, String> {
    let mut dec = Decoder::new(data);
    let cols = { let b = dec.read_bytes(2)?; u16::from_le_bytes(b.try_into().unwrap()) as usize };
    let mut vals = Vec::with_capacity(cols);
    for _ in 0..cols { vals.push(dec.decode_value()?); }
    Ok(vals)
}

fn main() {
    // Encode individual values
    let values = vec![
        Value::Integer(42),
        Value::String("hello, database".to_string()),
        Value::Boolean(true),
        Value::Null,
        Value::Float(3.14159),
        Value::Bytes(vec![0xDE, 0xAD, 0xBE, 0xEF]),
    ];

    println!("=== Individual Value Encoding ===");
    for val in &values {
        let encoded = encode(val);
        println!("{:?}", val);
        println!("  encoded: {:?} ({} bytes)", encoded, encoded.len());
        let mut dec = Decoder::new(&encoded);
        let decoded = dec.decode_value().unwrap();
        assert_eq!(*val, decoded);
        println!("  decoded: {:?}", decoded);
    }

    // Encode a full row
    println!("\n=== Row Encoding ===");
    let row = vec![
        Value::Integer(1),
        Value::String("Alice".to_string()),
        Value::Integer(30),
        Value::Boolean(true),
    ];

    let encoded_row = encode_row(&row);
    println!("Row: {:?}", row);
    println!("Encoded: {} bytes", encoded_row.len());
    println!("Raw: {:?}", encoded_row);

    let decoded_row = decode_row(&encoded_row).unwrap();
    assert_eq!(row, decoded_row);
    println!("Decoded: {:?}", decoded_row);

    // Show space efficiency vs text
    println!("\n=== Space Comparison ===");
    let int_val = Value::Integer(1_000_000);
    let binary_size = encode(&int_val).len();
    let text_size = "1000000".len() + "integer:".len(); // naive text format
    println!("Integer 1,000,000:");
    println!("  Binary: {} bytes", binary_size);
    println!("  Text:   {} bytes", text_size);
    println!("  Binary is {:.0}% of text size", (binary_size as f64 / text_size as f64) * 100.0);
}
```

Every value round-trips perfectly. The integer `42` encodes to 9 bytes (1 tag + 8 payload). The string "hello, database" encodes to 20 bytes (1 tag + 4 length + 15 chars). The null encodes to just 1 byte. No escaping, no parsing, no ambiguity.

---

## Complexity Table

| Operation | Time | Space | Notes |
|-----------|------|-------|-------|
| Encode Null | O(1) | 1 byte | Tag only |
| Encode Boolean | O(1) | 2 bytes | Tag + 1 byte |
| Encode Integer/Float | O(1) | 9 bytes | Tag + 8 bytes |
| Encode String (len n) | O(n) | 5 + n bytes | Tag + 4-byte length + payload |
| Encode Row (k cols) | O(k + total payload) | 2 + sum of column sizes | Column count prefix + encoded values |
| Decode any value | O(1) for fixed, O(n) for variable | Reconstructed value | Single pass, no backtracking |

The encoding is **self-describing**: you can decode a byte stream without knowing the schema in advance. Each value carries its own type tag. This is more flexible than schema-dependent formats (like Protocol Buffers), at the cost of slightly more space per value. For a database that may alter its schema, self-describing formats simplify recovery and debugging.

---

## Where This Shows Up in Our Database

In Chapter 4, we implement serialization for our key-value storage engine. Every `Value` gets encoded to bytes before writing to the log or B-tree pages:

```rust,ignore
// Storage engine writes encoded bytes to disk
pub fn write_row(&mut self, key: &[u8], row: &[Value]) -> Result<()> {
    let encoded = encode_row(row);
    self.storage.set(key, &encoded)?;
    Ok(())
}

// And reads them back
pub fn read_row(&self, key: &[u8]) -> Result<Option<Vec<Value>>> {
    match self.storage.get(key)? {
        Some(data) => Ok(Some(decode_row(&data)?)),
        None => Ok(None),
    }
}
```

Beyond our toy database, binary serialization is fundamental:
- **PostgreSQL** uses a binary wire protocol for client-server communication
- **SQLite** stores values in a compact binary format called "record format"
- **Write-ahead logs** serialize transactions as binary records for crash recovery
- **Network replication** sends binary-encoded rows between database nodes

Every database, at some level, is a machine that converts structured data to bytes and back.

---

## Try It Yourself

### Exercise 1: Null Bitmap

Database rows often contain many null values. Instead of encoding each null as a 1-byte tag, implement a **null bitmap** at the start of the row: one bit per column, where 1 means "this column is null." Only encode non-null values in the payload. Compare the space used for a row with 10 columns where 7 are null.

<details>
<summary>Solution</summary>

```rust
#[derive(Debug, Clone, PartialEq)]
enum Value {
    Null,
    Boolean(bool),
    Integer(i64),
    String(String),
}

const TAG_BOOLEAN: u8 = 1;
const TAG_INTEGER: u8 = 2;
const TAG_STRING: u8 = 4;

fn encode_value(value: &Value) -> Vec<u8> {
    let mut buf = Vec::new();
    match value {
        Value::Null => {} // never called for nulls
        Value::Boolean(b) => { buf.push(TAG_BOOLEAN); buf.push(if *b { 1 } else { 0 }); }
        Value::Integer(n) => { buf.push(TAG_INTEGER); buf.extend_from_slice(&n.to_le_bytes()); }
        Value::String(s) => {
            buf.push(TAG_STRING);
            let bytes = s.as_bytes();
            buf.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
            buf.extend_from_slice(bytes);
        }
    }
    buf
}

fn encode_row_with_bitmap(values: &[Value]) -> Vec<u8> {
    let mut buf = Vec::new();
    let col_count = values.len() as u16;
    buf.extend_from_slice(&col_count.to_le_bytes());

    // Null bitmap: ceil(col_count / 8) bytes
    let bitmap_bytes = (values.len() + 7) / 8;
    let mut bitmap = vec![0u8; bitmap_bytes];
    for (i, val) in values.iter().enumerate() {
        if matches!(val, Value::Null) {
            bitmap[i / 8] |= 1 << (i % 8);
        }
    }
    buf.extend_from_slice(&bitmap);

    // Only encode non-null values
    for val in values {
        if !matches!(val, Value::Null) {
            buf.extend_from_slice(&encode_value(val));
        }
    }
    buf
}

fn encode_row_naive(values: &[Value]) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&(values.len() as u16).to_le_bytes());
    for val in values {
        match val {
            Value::Null => buf.push(0), // 1 byte tag
            other => buf.extend_from_slice(&encode_value(other)),
        }
    }
    buf
}

fn main() {
    // 10 columns, 7 are null
    let row = vec![
        Value::Integer(1),
        Value::Null,
        Value::Null,
        Value::String("Alice".to_string()),
        Value::Null,
        Value::Null,
        Value::Null,
        Value::Null,
        Value::Null,
        Value::Integer(100),
    ];

    let naive = encode_row_naive(&row);
    let bitmap = encode_row_with_bitmap(&row);

    println!("Naive encoding:  {} bytes", naive.len());
    println!("Bitmap encoding: {} bytes", bitmap.len());
    println!("Saved: {} bytes ({:.0}% smaller)",
        naive.len() - bitmap.len(),
        (1.0 - bitmap.len() as f64 / naive.len() as f64) * 100.0
    );
    // The bitmap approach saves 1 byte per null (the tag byte) minus the
    // bitmap overhead (ceil(10/8) = 2 bytes). With 7 nulls:
    // Savings = 7 bytes (null tags) - 2 bytes (bitmap) = 5 bytes saved.
}
```

</details>

### Exercise 2: Checksum Validation

Add a CRC32-style checksum to the encoded row format. After encoding all values, append a 4-byte checksum computed from all preceding bytes. On decode, verify the checksum before returning the row. Use a simple checksum: XOR all bytes together, repeated into 4 bytes.

<details>
<summary>Solution</summary>

```rust
#[derive(Debug, Clone, PartialEq)]
enum Value {
    Null,
    Integer(i64),
    String(String),
}

const TAG_NULL: u8 = 0;
const TAG_INTEGER: u8 = 2;
const TAG_STRING: u8 = 4;

fn encode(value: &Value) -> Vec<u8> {
    let mut buf = Vec::new();
    match value {
        Value::Null => { buf.push(TAG_NULL); }
        Value::Integer(n) => { buf.push(TAG_INTEGER); buf.extend_from_slice(&n.to_le_bytes()); }
        Value::String(s) => {
            buf.push(TAG_STRING);
            let b = s.as_bytes();
            buf.extend_from_slice(&(b.len() as u32).to_le_bytes());
            buf.extend_from_slice(b);
        }
    }
    buf
}

fn simple_checksum(data: &[u8]) -> [u8; 4] {
    let mut check = [0u8; 4];
    for (i, &byte) in data.iter().enumerate() {
        check[i % 4] ^= byte;
        // Rotate to mix bits better
        check[i % 4] = check[i % 4].wrapping_add(byte.wrapping_mul(31));
    }
    check
}

fn encode_row_checked(values: &[Value]) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&(values.len() as u16).to_le_bytes());
    for v in values {
        buf.extend_from_slice(&encode(v));
    }
    let checksum = simple_checksum(&buf);
    buf.extend_from_slice(&checksum);
    buf
}

fn decode_row_checked(data: &[u8]) -> Result<Vec<Value>, String> {
    if data.len() < 6 { // 2 (col count) + 4 (checksum) minimum
        return Err("data too short".into());
    }

    let payload = &data[..data.len() - 4];
    let stored_checksum = &data[data.len() - 4..];
    let computed = simple_checksum(payload);

    if stored_checksum != computed {
        return Err(format!(
            "checksum mismatch: stored {:?}, computed {:?}",
            stored_checksum, computed
        ));
    }

    // Decode normally (simplified -- reuse your full decoder here)
    let col_count = u16::from_le_bytes(payload[..2].try_into().unwrap()) as usize;
    let mut pos = 2;
    let mut values = Vec::new();

    for _ in 0..col_count {
        let tag = payload[pos]; pos += 1;
        match tag {
            TAG_NULL => values.push(Value::Null),
            TAG_INTEGER => {
                let n = i64::from_le_bytes(payload[pos..pos+8].try_into().unwrap());
                pos += 8;
                values.push(Value::Integer(n));
            }
            TAG_STRING => {
                let len = u32::from_le_bytes(payload[pos..pos+4].try_into().unwrap()) as usize;
                pos += 4;
                let s = std::str::from_utf8(&payload[pos..pos+len]).unwrap().to_string();
                pos += len;
                values.push(Value::String(s));
            }
            _ => return Err(format!("unknown tag: {}", tag)),
        }
    }

    Ok(values)
}

fn main() {
    let row = vec![
        Value::Integer(42),
        Value::String("hello".to_string()),
        Value::Null,
    ];

    let encoded = encode_row_checked(&row);
    println!("Encoded with checksum: {} bytes", encoded.len());

    // Valid decode
    let decoded = decode_row_checked(&encoded).unwrap();
    assert_eq!(row, decoded);
    println!("Decoded OK: {:?}", decoded);

    // Corrupt one byte
    let mut corrupted = encoded.clone();
    corrupted[5] ^= 0xFF;
    match decode_row_checked(&corrupted) {
        Err(e) => println!("Corruption detected: {}", e),
        Ok(_) => println!("ERROR: corruption not detected!"),
    }
}
```

</details>

### Exercise 3: Nested Values (Arrays)

Extend the serialization to support `Value::Array(Vec<Value>)`. An array is encoded as: tag byte, element count (u32), then each element encoded recursively. Encode and decode a row containing `[1, "two", [3, 4]]` -- an array with a nested array inside.

<details>
<summary>Solution</summary>

```rust
#[derive(Debug, Clone, PartialEq)]
enum Value {
    Null,
    Integer(i64),
    String(String),
    Array(Vec<Value>),
}

const TAG_NULL: u8 = 0;
const TAG_INTEGER: u8 = 2;
const TAG_STRING: u8 = 4;
const TAG_ARRAY: u8 = 6;

fn encode(value: &Value) -> Vec<u8> {
    let mut buf = Vec::new();
    match value {
        Value::Null => { buf.push(TAG_NULL); }
        Value::Integer(n) => {
            buf.push(TAG_INTEGER);
            buf.extend_from_slice(&n.to_le_bytes());
        }
        Value::String(s) => {
            buf.push(TAG_STRING);
            let b = s.as_bytes();
            buf.extend_from_slice(&(b.len() as u32).to_le_bytes());
            buf.extend_from_slice(b);
        }
        Value::Array(elements) => {
            buf.push(TAG_ARRAY);
            buf.extend_from_slice(&(elements.len() as u32).to_le_bytes());
            for elem in elements {
                buf.extend_from_slice(&encode(elem)); // recursive!
            }
        }
    }
    buf
}

struct Decoder<'a> { data: &'a [u8], pos: usize }

impl<'a> Decoder<'a> {
    fn new(data: &'a [u8]) -> Self { Decoder { data, pos: 0 } }

    fn read_byte(&mut self) -> u8 {
        let b = self.data[self.pos]; self.pos += 1; b
    }

    fn read_bytes(&mut self, n: usize) -> &'a [u8] {
        let s = &self.data[self.pos..self.pos + n]; self.pos += n; s
    }

    fn decode(&mut self) -> Value {
        let tag = self.read_byte();
        match tag {
            TAG_NULL => Value::Null,
            TAG_INTEGER => {
                let b = self.read_bytes(8);
                Value::Integer(i64::from_le_bytes(b.try_into().unwrap()))
            }
            TAG_STRING => {
                let len = u32::from_le_bytes(
                    self.read_bytes(4).try_into().unwrap()
                ) as usize;
                let b = self.read_bytes(len);
                Value::String(std::str::from_utf8(b).unwrap().to_string())
            }
            TAG_ARRAY => {
                let count = u32::from_le_bytes(
                    self.read_bytes(4).try_into().unwrap()
                ) as usize;
                let mut elems = Vec::with_capacity(count);
                for _ in 0..count {
                    elems.push(self.decode()); // recursive!
                }
                Value::Array(elems)
            }
            _ => panic!("unknown tag: {}", tag),
        }
    }
}

fn main() {
    // [1, "two", [3, 4]]
    let nested = Value::Array(vec![
        Value::Integer(1),
        Value::String("two".to_string()),
        Value::Array(vec![
            Value::Integer(3),
            Value::Integer(4),
        ]),
    ]);

    let encoded = encode(&nested);
    println!("Nested array encoded to {} bytes", encoded.len());
    println!("Raw bytes: {:?}", encoded);

    let mut dec = Decoder::new(&encoded);
    let decoded = dec.decode();
    assert_eq!(nested, decoded);
    println!("Round-trip successful: {:?}", decoded);

    // Verify the structure: tag(1) + count(4) +
    //   int(9) + string(1+4+3=8) + array(1+4+int(9)+int(9)=23) = 45 bytes
    println!("Expected: 1 + 4 + 9 + 8 + 23 = 45 bytes");
}
```

</details>

---

## Recap

Binary serialization is the foundation of every storage engine. We built a format with three principles: type tags identify variants, fixed-size encoding for primitives, and length prefixes for variable data. The decoder reads one tag, dispatches to the right parser, reads exactly the right number of bytes, and moves on. No scanning, no escaping, no ambiguity. Every byte has a purpose and every value round-trips perfectly.
