## Design Insight: Strategic Programming

John Ousterhout draws a distinction between **tactical** and **strategic** programming in *A Philosophy of Software Design*.

A tactical programmer would skip the planner entirely. "Why build a separate plan representation? Just walk the AST and execute directly." This works in the short term and produces fewer lines of code. The SQL executor would contain a big match statement that handles AST nodes and reads/writes data in the same function.

A strategic programmer builds the planner as a separate stage, even though it seems like unnecessary work. The payoff comes later:

1. **Optimization is separate from planning.** In Chapter 9, we will add an optimizer that transforms plans into better plans (e.g., pushing filters closer to scans, eliminating redundant projections). This works because the optimizer receives a `Plan` and returns a `Plan` — it never touches the AST. If planning and execution were combined, adding optimization would require rewriting the executor.

2. **Execution is separate from validation.** The executor in Chapter 10 can assume the plan is valid — tables exist, columns are correct, types match. It does not need error-handling code for schema validation. This makes the executor simpler and faster.

3. **Testing is separated.** We can test the planner without executing anything. We can test the executor with hand-crafted plans without parsing SQL. Each stage is independently testable.

4. **Plan display is free.** Because the plan is a data structure, we can print it (`EXPLAIN`), serialize it, cache it, or send it over the network. If the plan were interleaved with execution, you would need to add instrumentation to an executing system — much harder.

The pipeline is:

```
SQL String → [Lexer] → Tokens → [Parser] → AST → [Planner] → Plan → [Optimizer] → Plan → [Executor] → Results
```

Each arrow is a clean boundary. Each stage has a defined input type and output type. You can replace, test, or optimize each stage independently. This is strategic programming: invest in structure now, and the system becomes easier to extend later.

Ousterhout's principle: *complexity is anything related to the structure of a system that makes it hard to understand and modify.* The planner adds a stage, but it reduces complexity by isolating concerns. The alternative — a monolithic execute-from-AST function — is simpler to write but harder to extend.

---
