## Design Insight: Information Hiding

In *A Philosophy of Software Design*, Ousterhout identifies **information hiding** as the most important technique for reducing complexity. The idea is simple: each module should hide its internal implementation details behind a simple interface. Changes to the implementation should not require changes to the callers.

Our client-server boundary is a perfect example. The client knows:

```
1. Connect to an address
2. Send SQL strings
3. Receive results (column names + rows) or errors
```

The client does NOT know:

- That the server uses a Volcano-model executor
- That the optimizer applies constant folding and filter pushdown
- That the parser is a recursive descent parser
- That the lexer uses a state machine
- That storage uses an in-memory HashMap
- That joins use a hash table internally
- That aggregations use accumulators

The entire SQL engine — lexer, parser, planner, optimizer, executor, storage — is hidden behind the wire protocol. Replacing the storage engine with a B-tree-based disk engine would not change the client at all. Replacing the optimizer with a cost-based optimizer would not change the client at all. Adding new SQL features (DISTINCT, HAVING, window functions) would not change the client at all, as long as the wire protocol still sends column names and rows.

This is information hiding at the system boundary level. The protocol is the interface. Everything behind it is implementation detail.

The same principle applies within the server. The executor does not know about the wire protocol. The optimizer does not know about the executor. The parser does not know about the planner. Each module hides its internals and exposes a narrow interface:

```
Lexer:     &str          → Vec<Token>
Parser:    Vec<Token>    → Statement
Planner:   Statement     → Plan
Optimizer: Plan          → Plan
Executor:  Plan          → ResultSet
Server:    SQL string    → Response
```

Each arrow is an information-hiding boundary. Changes on one side do not propagate to the other. This is why you could build each piece independently across 12 chapters — each chapter's work was hidden from the next chapter's code.

> *"The best modules are those that provide powerful functionality yet have simple interfaces."*
> — John Ousterhout

---
