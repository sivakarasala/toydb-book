## System Design Corner: Layered Architecture

Our database has a classic **layered architecture** — each layer provides services to the layer above and consumes services from the layer below. This is the dominant pattern in systems software.

### The layers

```
┌─────────────────────────────────────┐
│ Layer 7: Client Interface           │  (REPL, wire protocol)
├─────────────────────────────────────┤
│ Layer 6: Query Processing           │  (lexer, parser, planner, optimizer)
├─────────────────────────────────────┤
│ Layer 5: Execution Engine           │  (Volcano-model executor)
├─────────────────────────────────────┤
│ Layer 4: Transaction Management     │  (MVCC, snapshot isolation)
├─────────────────────────────────────┤
│ Layer 3: Consensus                  │  (Raft leader election + log replication)
├─────────────────────────────────────┤
│ Layer 2: Storage Engine             │  (BitCask, in-memory KV)
├─────────────────────────────────────┤
│ Layer 1: Operating System           │  (files, network, memory)
└─────────────────────────────────────┘
```

### Rules of layered architecture

1. **Each layer depends only on the layer directly below it.** The parser does not know about the storage engine. The executor does not know about Raft. This limits the blast radius of changes.

2. **Each layer has a well-defined interface.** The lexer's interface is `&str → Vec<Token>`. The parser's interface is `Vec<Token> → Statement`. These types are the contracts.

3. **Layers can be replaced independently.** Swap the storage engine from in-memory to disk-based? Only the storage layer changes. Replace the optimizer? Only the optimizer changes. Add a new wire protocol? Only the client interface layer changes.

4. **Skip layers carefully.** Sometimes a higher layer needs to bypass an intermediate layer for performance. For example, read queries skip the Raft layer (Layer 3) and go directly from execution (Layer 5) to storage (Layer 2). This is a deliberate design choice — it violates strict layering but is justified by the performance benefit and the fact that reads do not need consensus.

### Real-world examples

| Database | Layers |
|----------|--------|
| PostgreSQL | Client → Parser → Rewriter → Planner → Executor → Access Methods → Buffer Manager → Storage |
| MySQL | Client → Parser → Optimizer → Handler → Storage Engine (InnoDB/MyISAM) |
| CockroachDB | Client → SQL → Distributed SQL → Transaction → Raft → Pebble (storage) |
| SQLite | Client → Parser → Code Generator → Virtual Machine → B-tree → Pager → OS Interface |

Our toydb has the same shape as CockroachDB — SQL over Raft over a storage engine. The difference is scale: CockroachDB adds distributed SQL execution, range-based sharding, and a production-grade storage engine (Pebble, based on LevelDB). But the architectural pattern is identical.

> **Interview talking point:** *"Our database uses a layered architecture with clear boundaries between the SQL frontend (parser, planner, optimizer), the execution engine, the transaction manager (MVCC), the consensus layer (Raft), and the storage engine. Write queries flow through all layers — the SQL is parsed, planned, proposed to Raft, replicated to followers, committed, and then executed against MVCC storage. Read queries skip the consensus layer entirely, going directly from the executor to MVCC storage. This is safe because we only serve reads on the leader, and we use a read lease mechanism to ensure the leader is still authoritative. The error type hierarchy uses Rust's From trait to propagate errors cleanly across layer boundaries, with the server converting all errors into client-facing error messages."*

---
