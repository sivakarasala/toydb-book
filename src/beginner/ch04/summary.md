## What You Built

In this chapter, you:

1. **Added serde and bincode** -- the foundation of Rust's serialization ecosystem, with derive macros that generate encoding/decoding code at compile time
2. **Built a typed Value enum** -- five variants covering every SQL data type your database needs, with safe extraction methods
3. **Wrote round-trip tests** -- proving that every `Value` variant survives the encode-decode cycle, including edge cases like empty strings, extreme numbers, and corrupted input
4. **Created a Row and Table abstraction** -- moving from raw key-value pairs to structured data with named columns and typed values
5. **Implemented a custom binary format** -- understanding what serde does under the hood, and learning when manual encoding is the right choice

Your database now understands its data. It knows the difference between the integer `42` and the string `"42"`. It can store structured rows with multiple columns. And it can serialize everything to compact binary for storage or transmission.

But the database still has a critical limitation: if two users read and write at the same time, they will see inconsistent data. One user's write might appear halfway through another user's read. Chapter 5 introduces MVCC -- Multi-Version Concurrency Control -- the mechanism that gives every reader a consistent snapshot, even while writers are modifying data.

---

## Exercises

**Exercise 4.1: Add a `Bytes` variant to Value**

Add a `Bytes(Vec<u8>)` variant to the `Value` enum for storing raw binary data (like images or files). Implement both serde and manual encoding/decoding for it.

<details>
<summary>Hint</summary>

Add the variant to the enum, add a new type tag (e.g., `0x05`) in `encode_manual`, and add a new arm in `decode_manual`. Serde handles the new variant automatically -- just add it to the enum and re-derive.

</details>

**Exercise 4.2: Size comparison report**

Write a test that creates a row with 10 different values and compares the size of serde/bincode encoding vs manual encoding. Print a table showing the size of each individual value in both formats.

<details>
<summary>Hint</summary>

```rust,ignore
for val in &values {
    let serde_size = val.to_bytes().unwrap().len();
    let manual_size = val.encode_manual().len();
    println!("{:>20} | serde: {:>3} | manual: {:>3}", val, serde_size, manual_size);
}
```

</details>

**Exercise 4.3: Add a `DataType` enum**

Create a `DataType` enum with variants `Boolean`, `Integer`, `Float`, `String`. Add a method `Value::data_type(&self) -> Option<DataType>` that returns `None` for `Null` and the appropriate `DataType` for everything else. This will be useful when we add column type checking in later chapters.

<details>
<summary>Hint</summary>

```rust,ignore
#[derive(Debug, Clone, PartialEq)]
pub enum DataType {
    Boolean,
    Integer,
    Float,
    String,
}

impl Value {
    pub fn data_type(&self) -> Option<DataType> {
        match self {
            Value::Null => None,
            Value::Boolean(_) => Some(DataType::Boolean),
            // ...
        }
    }
}
```

</details>

---

## Key Takeaways

- **Serialization** converts structured data to bytes. Deserialization converts bytes back to structured data.
- **Serde** is Rust's serialization framework. It separates data model (your structs) from format (JSON, bincode, etc.).
- **Derive macros** auto-generate trait implementations at compile time. `#[derive(Serialize, Deserialize)]` replaces hundreds of lines of manual encoding code.
- **Bincode** is a compact binary format ideal for database storage. JSON is better for human-readable output.
- **Round-trip tests** are the most important serialization tests: encode, decode, verify equality.
- **Every derive requires all fields to support the trait.** If one field does not implement `Clone`, you cannot derive `Clone` on the struct.
- **Manual encoding** gives you byte-level control at the cost of more code and more bugs. Use it when you need wire protocol stability or minimal dependencies.

---

### Reference implementation

The files you built in this chapter correspond to these files in the reference codebase:

| Your file | Reference |
|-----------|-----------|
| `src/value.rs` | [`src/sql/types.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/sql/types.rs) -- `Value` enum with serialization |
| `src/table.rs` | [`src/sql/engine/local.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/sql/engine/local.rs) -- table operations |
| Manual encoding | [`src/encoding.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/encoding.rs) -- custom key encoding for ordered storage |
| Round-trip tests | Tests within each module |
