## What You Built

Let's review what you accomplished in this chapter:

1. **Error type** — You defined a custom `Error` enum with `NotFound` and `Internal` variants. Structured errors are better than string errors because the compiler ensures you handle each case.

2. **Storage trait** — You defined the `Storage` contract with four methods (`set`, `get`, `delete`, `scan`). Any type that implements this trait can be used as a storage engine.

3. **MemoryStorage** — You implemented the `Storage` trait using a `BTreeMap`. This is a working in-memory storage engine that keeps keys in sorted order.

4. **Generic Database** — You wrote `Database<S: Storage>`, a database that works with any storage engine. This is the power of traits + generics: write once, use with many types.

5. **Unit tests** — You wrote seven tests proving your storage engine works correctly.

In Chapter 3, we will build a second storage engine — one that writes to disk. Because it implements the same `Storage` trait, the `Database` code will not change at all. That is the payoff of the design work you did in this chapter.

---
