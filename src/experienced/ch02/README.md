# Chapter 2: In-Memory Storage Engine

Every database needs a place to put data. Before you worry about disks, files, or network protocols, you need to answer a simpler question: if someone hands you a key and a value, where do you store them so you can get the value back later? This chapter builds that foundation — a storage engine that lives entirely in memory.

By the end of this chapter, you will have:

- A `Storage` trait that defines the contract every storage engine must fulfill
- A `MemoryStorage` struct backed by a `BTreeMap` with ordered iteration
- A generic `Database<S: Storage>` that works with any engine
- Unit tests proving your engine handles set, get, delete, and range scans

---
