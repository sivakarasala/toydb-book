## What We Built

This chapter transformed ASTs into validated execution plans. Here is what you learned:

| Concept | What it does | Why it matters |
|---------|-------------|----------------|
| **Iterators** | Produce a sequence of values one at a time | Process collections without manual loops |
| **Iterator adapters** | Transform iterators: `.map()`, `.filter()`, `.collect()` | Chain transformations cleanly |
| **Closures** | Anonymous functions that capture surrounding variables | Customize iterator behavior |
| **Lifetimes (`'a`)** | Track how long references are valid | Share data safely without copying |
| **Option/Result with iterators** | `.collect::<Result<Vec<_>, _>>()` | Stop at first error in a list |

The plan tree is the bridge between "what the user asked" (AST) and "how to get it" (execution). In the next chapter, we will build an optimizer that rewrites plans to be more efficient.

---
