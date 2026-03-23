# Chapter 18: Testing, Benchmarking & Extensions

You have built a distributed SQL database from scratch — seventeen chapters of storage engines, SQL parsing, query execution, MVCC transactions, client-server networking, and Raft consensus. But building software is only half the job. The other half is knowing whether it works, how fast it is, and where it breaks. This final chapter turns inward. You will learn how to test a system this complex with confidence, how to benchmark its performance to find bottlenecks, and where to take it next.

The spotlight concept is **testing and benchmarking** — Rust's built-in test framework, property-based testing with `proptest`, deterministic testing for distributed systems, and performance measurement with `criterion`. Rust makes testing a first-class citizen: tests live next to the code they verify, run with a single command, and the compiler catches entire categories of bugs before a test is ever written.

By the end of this chapter, you will have:

- A comprehensive testing strategy for each layer of the database
- Unit tests using `#[test]` with assertion macros
- Integration tests in the `tests/` directory for end-to-end verification
- Property-based tests using `proptest` that generate random inputs and verify invariants
- Deterministic tests for Raft that control time and network behavior
- Benchmarks using `criterion` to measure and track performance
- A review of everything you built and ideas for extending it further

---
