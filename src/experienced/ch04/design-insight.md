## Design Insight: Information Hiding

In *A Philosophy of Software Design*, Ousterhout argues that the most important technique for managing complexity is **information hiding** — each module should encapsulate knowledge that is not needed by other modules.

Serde is a masterclass in information hiding. Consider what happens when you write:

```rust
#[derive(Serialize, Deserialize)]
struct Row {
    values: Vec<Value>,
}
```

The caller of `Row::to_bytes()` does not know:
- Whether the format is little-endian or big-endian
- Whether strings are length-prefixed or null-terminated
- Whether the enum discriminant is 1 byte or 4 bytes
- Whether Vec length is u32 or u64
- How nested structs are flattened

All of this is hidden behind the derive macro and the bincode crate. The caller says "serialize this" and gets bytes. The implementation can change (swap bincode for postcard, switch to big-endian) without touching any caller code.

Compare this to the manual encoding from Exercise 4. Every caller must know the format — type tag values, byte offsets, endianness. Change any detail and every call site breaks. The manual format leaks information; the serde format hides it.

This does not mean manual encoding is wrong. Sometimes you need the control. But Ousterhout's insight is that information leakage is the primary source of complexity in software, and you should default to hiding unless you have a specific reason to expose.

In toydb, our `Value::to_bytes()` hides the encoding format. The storage engine calls `to_bytes()` and `from_bytes()` without knowing whether the underlying format is bincode, postcard, or hand-rolled. If we later switch formats for better performance, the storage engine does not change.

---
