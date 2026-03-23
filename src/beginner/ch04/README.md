# Chapter 4: Serialization — Turning Structs Into Bytes

Your database can store key-value pairs on disk. But look at what it actually stores: raw bytes. Just `Vec<u8>` — an untyped blob. Your database has no idea whether the bytes `[52, 50]` represent the number 42, the string "42", or something else entirely. It is like a filing cabinet that accepts any piece of paper but cannot tell you whether it contains a number, a name, or a shopping list.

A real database needs to understand its data. It needs to know that `42` is a number (so it can do math), that `"Alice"` is a string (so it can search text), and that `true` is a boolean (so it can filter results). This chapter builds that understanding.

You will create a typed `Value` system and learn how to convert Rust structs into bytes (serialization) and back again (deserialization). Along the way, you will discover one of Rust's most powerful features: derive macros that write code for you.

By the end of this chapter, you will have:

- A `Value` enum with typed variants (Null, Boolean, Integer, Float, String) that serializes to compact binary
- Round-trip tests proving that encode-then-decode preserves every type exactly
- A `Row` type that packs multiple values into a single byte sequence for storage
- A hand-rolled binary format to compare against the automatic approach
- A deep understanding of derive macros, serde, and how serialization works under the hood

---
