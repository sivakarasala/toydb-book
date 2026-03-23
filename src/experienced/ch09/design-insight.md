## Design Insight: Pass-Through Elimination

In *A Philosophy of Software Design*, Ousterhout warns against pass-through methods — methods that do nothing but forward calls to another method. They add complexity without adding functionality.

Our optimizer rules embody the opposite principle: **each rule eliminates pass-through nodes in the plan tree.** A `Filter(true)` node is a pass-through — it accepts every row and forwards it unchanged. Constant folding detects this and removes the node. A `Filter` above a `Project` is processing rows that might be filtered out — filter pushdown rearranges the tree so rows are eliminated before the expensive projection.

The design insight is broader than optimizer rules:

**Optimizer rules do not create new plan types. They rearrange existing ones.** Each rule has a single responsibility — it knows one pattern and one rewrite. The rule does not know about other rules. The rule does not know about the overall optimization strategy. It just looks for its pattern and applies its rewrite.

This is why the `OptimizerRule` trait is so simple: one method for the name, one method for the transformation. No configuration, no state, no interaction between rules. The `Optimizer` composes them. This is the single-responsibility principle applied to tree transformations.

The same pattern appears throughout software design:

- **Compiler passes:** Each pass (dead code elimination, constant propagation, register allocation) knows one transformation.
- **Unix pipes:** Each tool (grep, sort, uniq, awk) does one thing. Composition creates complex behavior.
- **Middleware in web frameworks:** Each middleware handles one concern (logging, authentication, compression). The framework chains them.

When you find yourself writing a complex transformation, ask: "Can I decompose this into multiple simple transformations applied in sequence?" If each transformation is correct in isolation and the sequence is well-ordered, the composition is correct by construction. This is easier to test, easier to understand, and easier to extend than a monolithic transformation that handles every case.

> *"The best modules are those that provide powerful functionality yet have simple interfaces. A module that has a simple interface is easier to modify without affecting other modules."*
> — John Ousterhout, *A Philosophy of Software Design*

---
