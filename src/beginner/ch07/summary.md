## What We Built

This chapter transformed flat token streams into structured Abstract Syntax Trees. Here is what you learned:

| Concept | What it does | Why it matters |
|---------|-------------|----------------|
| **Box\<T\>** | Heap-allocates a value, stores a pointer | Allows recursive types like trees |
| **Recursive types** | Enums that contain themselves (via Box) | Natural for tree data structures like ASTs |
| **Recursive descent parsing** | Each grammar rule is a function that calls others | Simple, readable parser structure |
| **Operator precedence** | Numbers controlling which operators bind tighter | `2 + 3 * 4` parses correctly as `2 + (3 * 4)` |
| **Option\<T\>** | Represents a value that might not exist | Optional clauses like WHERE |
| **Result\<T, E\>** | Represents success or failure | Error handling in parsing |

The AST is the bridge between human-readable SQL and machine-executable operations. In the next chapter, we will build a query planner that takes this AST and converts it into an execution plan -- the step-by-step instructions for actually running the query.

---
