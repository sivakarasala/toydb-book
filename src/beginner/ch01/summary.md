## What You Built

Let's step back and look at what you accomplished in this chapter:

1. **Variables and types** — You learned about `let`, `mut`, integers, floats, booleans, and the two string types (`&str` and `String`).

2. **Enums** — You created a `Value` enum that can hold five different types of data. This is more powerful than enums in most languages because each variant can carry different data.

3. **HashMap** — You used Rust's hash map to build a key-value store. You learned about `insert`, `get`, `remove`, `Option<T>`, and the difference between owned and borrowed access.

4. **A working REPL** — You built an interactive command-line interface with `loop`, `match`, string parsing, and user input.

This is the foundation everything else builds on. In Chapter 2, we will extract the storage logic into a **trait** (Rust's version of an interface) so we can swap between different storage engines — in-memory, on-disk, distributed — without changing the rest of the code.

---
