# Chapter 4: Serialization

In Chapter 3, you built a persistent storage engine that writes bytes to disk and reads them back. But everything is `Vec<u8>` — raw, untyped bytes. Your database can store `"42"` as a string, but it has no idea whether that is a number, a name, or a boolean. A real database needs to understand its data. It needs to encode integers differently from strings, pack structured rows into bytes for storage, and unpack them on read without losing information.

This chapter builds a proper serialization layer. You will add serde and bincode to your project, derive `Serialize` and `Deserialize` on your types, build round-trip tests, and then implement a custom binary format by hand — because understanding what serde does under the hood makes you a better engineer, the same way understanding what the compiler does makes you a better programmer.

By the end of this chapter, you will have:

- A `Value` enum with typed variants (Null, Boolean, Integer, Float, String) that serializes to compact binary
- Round-trip tests proving that encode-then-decode preserves every type exactly
- A `Row` type that packs multiple values into a single byte sequence for storage
- A hand-rolled length-prefixed binary format to compare against serde's magic
- A deep understanding of derive macros, trait derivation, and how serde works

---
