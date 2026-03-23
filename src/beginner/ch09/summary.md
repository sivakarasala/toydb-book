## What We Built

In this chapter, you built a query optimizer that transforms naive plans into efficient ones. Here is what you accomplished:

1. **OptimizerRule trait** -- a clean interface that any optimization rule implements
2. **Optimizer struct** -- holds `Vec<Box<dyn OptimizerRule>>` and applies rules in sequence
3. **Constant folding** -- evaluates expressions like `1 + 1` at plan time, removing always-true/false filters
4. **Filter pushdown** -- moves filters closer to the data source, reducing unnecessary work
5. **Display helpers** -- human-readable plan tree output for debugging

The Rust concepts you learned:

- **Trait objects (`dyn Trait`)** -- erasing concrete types to store different types behind a common interface
- **`Box<dyn Trait>`** -- heap-allocating trait objects because their size is unknown at compile time
- **Dynamic dispatch** -- runtime method lookup through a vtable (two-pointer "fat pointer")
- **Static vs dynamic dispatch** -- `impl Trait` for compile-time resolution, `dyn Trait` for runtime resolution
- **Object safety** -- the rules for which traits can be used as trait objects (no generics, no Self returns)
- **The vtable** -- a compile-time table of function pointers that enables runtime method lookup

The optimizer is the first part of your database that "thinks." The lexer, parser, and planner are mechanical -- they translate SQL into a plan following fixed rules. The optimizer looks at the plan and asks "can I do this faster?" This is the beginning of intelligence in your database engine.

---
