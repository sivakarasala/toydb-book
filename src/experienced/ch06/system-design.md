## System Design Corner: Language Processing Pipelines

In a system design interview, explaining how a database processes a SQL query shows deep understanding. The pipeline has five stages:

```
SQL string
    │
    ▼
┌────────┐     "SELECT name FROM users WHERE id = 42"
│ Lexer  │
└───┬────┘
    │          [SELECT, name, FROM, users, WHERE, id, =, 42]
    ▼
┌────────┐
│ Parser │
└───┬────┘
    │          AST: Select { columns: [name], table: users, where: id = 42 }
    ▼
┌────────────┐
│ Optimizer  │
└───┬────────┘
    │          Plan: IndexScan(users.id = 42) -> Project(name)
    ▼
┌────────────┐
│ Executor   │
└───┬────────┘
    │          Result: [("Alice",)]
    ▼
   Client
```

**Lexer** (this chapter): Characters to tokens. O(N) where N is query length. Cannot fail on valid SQL syntax — only on invalid characters or unterminated strings.

**Parser** (Chapter 7): Tokens to Abstract Syntax Tree (AST). O(N) where N is token count. Detects syntax errors like `SELECT FROM` (missing column list) or `WHERE AND` (missing left operand).

**Optimizer** (Chapter 9): AST to execution plan. Chooses between table scan and index lookup, reorders joins, pushes filters down. This is where query performance is determined — a bad optimizer turns a 10ms query into a 10-minute query.

**Executor** (Chapter 10): Runs the plan against the storage engine. Returns rows. May use iterators (Volcano model) or batch processing (vectorized execution).

Each stage is a separate module with a clean interface. The lexer does not know about tables. The parser does not know about indexes. The optimizer does not know about disk I/O. This separation of concerns is what makes databases tractable — you can reason about each stage independently.

> **Interview talking point:** *"Our query processing pipeline has four stages: lexer, parser, optimizer, and executor. The lexer converts SQL text to tokens in O(N) time. The parser builds an AST and catches syntax errors. The optimizer generates an execution plan — choosing between sequential scans, index lookups, and join strategies based on statistics. The executor runs the plan using the Volcano iterator model, where each operator pulls rows from its children on demand. This lazy evaluation means we never materialize intermediate results larger than necessary."*

---
