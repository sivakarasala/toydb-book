## Design Insight: Deep Modules

In *A Philosophy of Software Design*, John Ousterhout introduces the concept of **deep modules** — modules that provide powerful functionality behind a simple interface. The value of a module is the ratio of its functionality to the complexity of its interface. Deep modules have simple interfaces and rich implementations. Shallow modules have complex interfaces that do not hide much.

The `Storage` trait is a deep module. Its interface is four methods:

```rust
fn set(&mut self, key: String, value: Vec<u8>) -> Result<(), Error>;
fn get(&self, key: &str) -> Result<Option<Vec<u8>>, Error>;
fn delete(&mut self, key: &str) -> Result<(), Error>;
fn scan(&self, range: impl RangeBounds<String>) -> Result<Vec<(String, Vec<u8>)>, Error>;
```

Four methods. A caller can learn this interface in two minutes. Behind it, an implementation might manage:

- A B-tree with node splitting, merging, and rebalancing
- Write-ahead logging for crash recovery
- Memory-mapped files for zero-copy reads
- Bloom filters for negative lookups
- Compaction threads that merge sorted runs
- Checksums for data integrity

All of that complexity is hidden. The caller writes `store.get("key")` and gets bytes back. They do not know, need to know, or want to know about B-tree fanout or compaction strategies.

Contrast this with a shallow interface that exposes implementation details:

```rust
// Shallow: leaks implementation details
fn get(&self, key: &str, use_bloom_filter: bool, cache_hint: CachePolicy) -> ...
fn set(&mut self, key: String, value: Vec<u8>, sync: bool, compression: Codec) -> ...
```

Every parameter beyond the essentials (`key`, `value`) is a leak. The caller must understand bloom filters, cache policies, sync semantics, and compression codecs to use the API. The interface is wide, and its complexity grows with every new feature.

Deep modules are the goal. The `Storage` trait stays at four methods even as the implementation grows from 20 lines (MemoryStorage) to thousands (a production disk engine). That is depth.

---
