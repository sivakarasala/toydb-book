## What We Built: The Complete Architecture

Let us step back and review what you have built across 18 chapters:

```
┌─────────────────────────────────────────────────────────────────┐
│                        toydb                                     │
│                                                                  │
│  Ch 12-13: Client/Server         Ch 6-11: SQL Engine             │
│  ┌──────────────────────┐       ┌──────────────────────────────┐ │
│  │ TCP Server (async)   │       │ Lexer     → Tokens           │ │
│  │ Wire Protocol        │       │ Parser    → AST              │ │
│  │ REPL Client          │       │ Planner   → Plan             │ │
│  └──────────┬───────────┘       │ Optimizer → Optimized Plan   │ │
│             │                   │ Executor  → Results          │ │
│             │                   └──────────────┬───────────────┘ │
│             │                                  │                 │
│             └────────────┬─────────────────────┘                 │
│                          │                                       │
│  Ch 14-16: Raft          │         Ch 1-5: Storage               │
│  ┌──────────────────┐    │        ┌─────────────────────────────┐│
│  │ Leader Election  │    │        │ Key-Value (HashMap)         ││
│  │ Log Replication  │◄───┤        │ BitCask (append-only disk)  ││
│  │ WAL + Recovery   │    │        │ MVCC (multi-version)        ││
│  │ Snapshots        │    └───────>│ Serialization               ││
│  └──────────────────┘             └─────────────────────────────┘│
│                                                                  │
│  Ch 17: Integration     Ch 18: Testing                           │
│  ┌──────────────────┐   ┌──────────────────────────────────────┐ │
│  │ Server struct     │   │ Unit + Integration + Property tests  │ │
│  │ Error propagation│   │ Deterministic distributed testing    │ │
│  │ Config + startup │   │ Benchmarking with criterion          │ │
│  │ Read/write paths │   │ Golden test suite                    │ │
│  └──────────────────┘   └──────────────────────────────────────┘ │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### Chapter-by-chapter summary

| Chapter | What you built | Spotlight Rust concept |
|---------|---------------|----------------------|
| 1 | Key-value store with REPL | Variables, types, HashMap |
| 2 | In-memory storage engine | Traits, generics |
| 3 | BitCask disk storage | File I/O, serialization |
| 4 | Binary serialization | Bytes, endianness, encoding |
| 5 | MVCC transactions | Lifetimes, borrows |
| 6 | SQL lexer | Enums, pattern matching |
| 7 | SQL parser | Recursion, Box, tree structures |
| 8 | Query planner | From AST to execution plan |
| 9 | Query optimizer | Tree transformations |
| 10 | Query executor | Iterators, Volcano model |
| 11 | SQL features (JOIN, GROUP BY) | Collections, closures |
| 12 | Client-server protocol | Structs, networking |
| 13 | Async networking | async/await, Tokio |
| 14 | Raft leader election | State machines, timers |
| 15 | Raft log replication | Channels, message passing |
| 16 | Raft durability | Ownership, persistence |
| 17 | Full integration | Module system, workspace |
| 18 | Testing and benchmarks | Testing, benchmarking |

---

## What You Built

In this chapter, you:

1. **Mastered Rust's testing framework** — `#[test]`, `#[cfg(test)]`, assertion macros, `#[should_panic]`, test modules, integration tests in `tests/`, doc tests
2. **Wrote property-based tests** — `proptest` strategies that generate random valid SQL and verify parser invariants across thousands of inputs
3. **Built deterministic distributed tests** — fake clock, fake network, test cluster, chaos engine with seeded randomness for reproducible failure scenarios
4. **Benchmarked storage engines** — `criterion` for statistical rigor, `black_box` to prevent dead code elimination, throughput measurements for Memory vs BitCask
5. **Created a golden test suite** — SQL scripts with expected output files, automatic diff on failure, easy-to-add test cases
6. **Reviewed the complete system** — all 18 chapters, all layers, the full architecture of a distributed SQL database built from scratch in Rust

---

## A Final Word

You started with a `HashMap` and a REPL. You ended with a distributed SQL database that parses queries, optimizes execution plans, provides transactional isolation, replicates data across a cluster for fault tolerance, persists state to survive crashes, and serves clients over a network.

This is not a toy. It is a real database — small and incomplete compared to PostgreSQL, but architecturally identical to CockroachDB, TiDB, and YugabyteDB. The same layers, the same patterns, the same tradeoffs. The difference is scale, not kind.

More importantly, you learned Rust by building something real. Not by reading about ownership in isolation, but by discovering why ownership matters when you need exclusive access to a WAL file. Not by memorizing trait syntax, but by defining a `Storage` trait so the executor does not need to know about BitCask. Not by studying lifetimes in the abstract, but by building MVCC where lifetimes determine which versions a transaction can see.

The code is yours. Extend it. Break it. Rewrite it. Use it as a reference when you encounter these patterns in production systems. And the next time someone asks "how does a database work?" — you know, because you built one.

---

### DS Deep Dive

Our testing chapter scratches the surface of distributed systems testing. The gold standard is the approach used by FoundationDB: deterministic simulation that models every source of non-determinism (time, network, disk, thread scheduling) and runs millions of simulated hours of cluster operation in minutes of wall-clock time. This deep dive explores the "simulation testing" paradigm, how it compares to TLA+ model checking, and why FoundationDB credits it with catching bugs that no amount of traditional testing would find.
