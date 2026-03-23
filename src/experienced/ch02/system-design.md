## System Design Corner: Pluggable Storage Engines

In a system design interview, you might be asked: *"Design a storage engine for a database."* The trait pattern you built in this chapter is the answer to the first question every interviewer asks: *"How do you support multiple backends?"*

### The architecture

```
┌──────────────────────────────────────────┐
│            SQL Engine / API              │
├──────────────────────────────────────────┤
│          Database<S: Storage>            │
├──────────────┬───────────┬───────────────┤
│ MemoryStorage│DiskStorage│DistributedStore│
│  (BTreeMap)  │ (BitCask) │   (Raft)      │
└──────────────┴───────────┴───────────────┘
```

Everything above the `Storage` trait is engine-agnostic. The SQL parser, the query planner, the client protocol — none of them know which engine is active. This is the **pluggable storage engine** pattern, used by MySQL (InnoDB, MyISAM, Memory), MongoDB (WiredTiger, MMAPv1), and many other databases.

### Interview talking points

**Why in-memory first?** It is the simplest correct implementation. You validate the interface, build tests, and get the database logic working before adding the complexity of disk I/O. This is how CockroachDB, TiDB, and other production databases develop their storage layers — memory first, then disk.

**Why a trait instead of an enum?** A trait is open for extension. Adding a new engine means adding a new struct and `impl Storage for NewEngine`. An enum is closed — adding a variant requires changing the enum definition and every `match` that handles it. Traits follow the open-closed principle: open for extension, closed for modification.

**What about performance?** The generic `Database<S: Storage>` uses static dispatch — the compiler generates one version of `Database` per engine type. There is no vtable lookup, no pointer indirection. This is equivalent to writing separate `DatabaseMemory` and `DatabaseDisk` types, but without duplicating any code. In Rust, abstraction does not cost performance.

**What about testing?** The trait enables mock storage engines for testing. You can create a `FailingStorage` that returns errors on every operation to test your database's error handling, or a `SlowStorage` that adds latency to test timeout behavior. The database under test never knows the difference.

> **Interview framing:** *"We define a Storage trait with set, get, delete, and scan. The Database struct is generic over any Storage implementation. This gives us pluggable backends — we start with in-memory for development, add disk persistence for production, and can later add distributed storage. The trait boundary is where we swap engines without touching the database logic."*

---
