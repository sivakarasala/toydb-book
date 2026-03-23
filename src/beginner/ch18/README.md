# Chapter 18: Testing, Benchmarking & Extensions

You built a distributed SQL database from scratch. Seventeen chapters of storage engines, SQL parsing, query execution, MVCC transactions, client-server networking, and Raft consensus. That is a real achievement.

But building software is only half the job. The other half is knowing whether it works, how fast it is, and where it breaks. This chapter turns inward. You will learn how to test each layer with confidence, measure performance to find bottlenecks, and then step back to see the full picture of what you built.

By the end of this chapter, you will have:

- A thorough understanding of Rust's testing framework
- Unit tests using `#[test]` and assertion macros
- Integration tests in the `tests/` directory
- Benchmarks that measure and track performance
- A complete architecture review of your database
- Ideas for extending it further and where to go next

---
