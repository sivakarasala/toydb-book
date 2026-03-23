## Design Insight: Pull Complexity Downward

In *A Philosophy of Software Design*, John Ousterhout advises: **"Pull complexity downward."** When a module has unavoidable complexity, push it into the implementation rather than leaking it to callers.

Look at the `LogStorage` API from the caller's perspective:

```rust
let mut store = LogStorage::new("data.log")?;
store.set("name", b"ToyDB")?;
let value = store.get("name")?;
store.delete("name")?;
```

Four lines. Simple. The caller knows nothing about:

- Binary record formats (CRC, key_len, value_len, payload)
- File seeking and buffered I/O
- Tombstone-based deletes
- Index rebuilding on startup
- Crash recovery and truncated record handling
- fsync for durability

All of that complexity is **pulled downward** into the `LogStorage` implementation. The caller's mental model is just "set, get, delete" — the same interface as a `HashMap`. Whether the data lives in memory or on disk, uses checksums or not, syncs every write or batches them — these are implementation details that the caller should never need to think about.

This is why we defined a `Storage` trait with a minimal interface. The trait is the contract: "you give me keys and values, I store them and give them back." The how is entirely the responsibility of the implementation. Tomorrow you could swap `LogStorage` for a `RocksDBStorage` or a `PostgresStorage`, and as long as it implements the `Storage` trait, every caller works unchanged.

The temptation is to do the opposite — expose configuration knobs, require callers to call `fsync()` manually, force them to handle partial writes. This pushes complexity **upward** and makes every caller deal with the same hard problems. Pull it down. Handle it once, correctly, inside the module. Let callers focus on their own problems.

---
