## Design Insight: Information Hiding

> *"The most important technique for achieving simplicity is to design systems so that developers only need to face a small fraction of the overall complexity at any time."*
> — John Ousterhout, *A Philosophy of Software Design*

Consider what each layer hides from the one above it:

| Layer | What it hides |
|-------|---------------|
| **Lexer** | Character-by-character scanning, whitespace handling, string escape sequences, keyword recognition |
| **Parser** | Token lookahead, operator precedence, recursive descent, error recovery |
| **Planner** | Schema lookup, type validation, plan node construction, table existence checks |
| **Optimizer** | Constant folding rules, filter pushdown logic, cost estimation, plan tree transformations |
| **Executor** | Volcano model iteration, hash table construction for joins, accumulator management for aggregations |
| **MVCC** | Version chains, snapshot timestamps, write conflict detection, garbage collection of old versions |
| **Raft** | Leader election timeouts, heartbeat scheduling, log matching, vote counting, term management |
| **Storage** | File format, fsync timing, compaction, block caching, disk I/O scheduling |

Each layer's hidden complexity is substantial — hundreds or thousands of lines of code. But the interface between layers is tiny: a few types and a few functions. This ratio of hidden complexity to interface surface area is what Ousterhout calls the "deep module" principle. The deepest modules provide the most value because they hide the most complexity behind the simplest interface.

The integration layer (this chapter's `Server` struct) is the opposite — a **shallow module**. It has many dependencies but does little work of its own. Its job is wiring, not computation. Shallow modules are fine at the top of the architecture — someone has to connect the pieces — but the deep modules below are where the real value lives.

---
