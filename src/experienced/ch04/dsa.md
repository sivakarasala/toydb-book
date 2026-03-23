## DSA in Context: Binary Encoding

Serialization is a data structures problem in disguise. Every encoding scheme makes tradeoffs between three competing goals:

### Fixed-length vs variable-length encoding

**Fixed-length:** Every integer is 8 bytes, every float is 8 bytes, regardless of the actual value. The number `1` takes the same space as `9,223,372,036,854,775,807`.

```
Integer(1)    -> [0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]  (8 bytes)
Integer(MAX)  -> [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x7F]  (8 bytes)
```

Advantage: you can seek to the Nth field by calculating `offset = N * field_size`. No parsing needed. This is why SQLite uses fixed-size records in its B-tree pages.

**Variable-length (varint):** Small numbers use fewer bytes. Protocol Buffers and SQLite's record format use this:

```
Integer(1)    -> [0x01]                    (1 byte)
Integer(300)  -> [0xAC, 0x02]             (2 bytes)
Integer(MAX)  -> [0xFF, 0xFF, ..., 0x01]  (10 bytes)
```

Advantage: most real-world numbers are small. Variable-length encoding saves 50-70% of space for typical data. Disadvantage: you must parse sequentially — you cannot jump to the 5th field without reading fields 1-4.

### Endianness

Should the byte `0x01 0x00` mean 1 (little-endian, least significant byte first) or 256 (big-endian, most significant byte first)?

- **Little-endian (LE):** x86, ARM (default), most modern CPUs. Rust's `to_le_bytes()`.
- **Big-endian (BE):** Network byte order, Java's default. Rust's `to_be_bytes()`.

Our format uses little-endian because that matches the CPU's native byte order, avoiding conversion overhead. Network protocols traditionally use big-endian (hence "network byte order"), but modern protocols like Protocol Buffers use variable-length encoding that sidesteps the question entirely.

### Schema evolution

What happens when you add a field to your struct? With bincode's default format, old data cannot be deserialized into the new struct — the byte layout changed. With formats like Protocol Buffers or MessagePack, fields are tagged with numbers, so new fields can be added without breaking old data.

```
// Version 1: { name: String, age: i64 }
// Bincode: [name_len][name_bytes][age_bytes]

// Version 2: { name: String, age: i64, email: String }
// Bincode: [name_len][name_bytes][age_bytes][email_len][email_bytes]
// Old data is missing email_len — deserialization fails!
```

For toydb, schema evolution is not a concern yet — we control both the writer and reader, and we can migrate data. But in Chapter 12, when we design the client-server wire protocol, we will need to think carefully about backwards compatibility.

---
