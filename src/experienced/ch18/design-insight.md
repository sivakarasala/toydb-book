## Design Insight: Testing Philosophy

> *"Software testing proves the presence of bugs, never their absence."*
> — Edsger Dijkstra

Ousterhout approaches testing pragmatically in *A Philosophy of Software Design*: tests should reduce the fear of modifying code. If you are afraid to change a module because "something might break," you need more tests for that module. Tests are a safety net that gives you confidence to refactor.

The debate between "test first" (TDD) and "code first" is less important than the outcome: **do you have enough tests to refactor with confidence?** For our database:

- **Storage engine:** Needs extensive tests because bugs cause data loss. Property tests are especially valuable — they explore the input space far more broadly than hand-written tests.
- **SQL parser:** Needs many specific tests because SQL has complex grammar rules. Golden tests are ideal — each test case is a single SQL statement and its expected AST.
- **Raft:** Needs deterministic simulation because real-world failures are hard to reproduce. The fake clock + fake network pattern eliminates timing-dependent test flakiness.
- **Integration:** Needs end-to-end tests that verify the layers work together. These are the most valuable tests in the entire suite — they catch bugs that no unit test would find.

The practical approach: write tests at the level where they are most effective. Do not force unit tests on code that is better tested at the integration level. Do not force integration tests when a unit test is sufficient. Match the testing strategy to the complexity of the code.

One more thing: **the compiler is your first test suite.** Rust's type system catches null pointer dereferences, data races, use-after-free, buffer overflows, and type mismatches at compile time. These are entire categories of bugs that JavaScript, Python, and Go developers must write tests to catch. Your test suite can focus on logic bugs because the compiler handles the rest.

---
