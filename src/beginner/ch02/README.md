# Chapter 2: In-Memory Storage Engine

In Chapter 1, you built a database with a `HashMap` and a REPL. It works, but it has a design problem: the `Database` struct is locked into using a `HashMap`. What if you later want to store data on disk instead of in memory? You would have to rewrite the entire `Database` struct, the REPL, and everything that touches it.

Real databases solve this problem by separating *what* the storage does (store a value, get a value, delete a value) from *how* it does it. The "what" is defined in a contract. The "how" is implemented by different storage engines. You can swap engines without changing the rest of the code.

In Rust, that contract is called a **trait**.

By the end of this chapter, you will have:

- A `Storage` trait that defines the contract every storage engine must follow
- A `MemoryStorage` struct backed by a `BTreeMap` (like HashMap but sorted)
- A generic `Database<S: Storage>` that works with *any* engine that implements the trait
- Unit tests proving your engine handles set, get, delete, and scan operations
- A clear understanding of traits and generics — two of Rust's most important features

---
