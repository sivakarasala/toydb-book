## DSA in Context: Testing Strategies and Coverage

Testing is itself a computer science problem. How do you test a system with infinite possible inputs?

### The testing pyramid

```
         ╱ ╲
        ╱ E2E╲        Few, slow, expensive
       ╱───────╲       Test the full system
      ╱ Integra-╲      Many modules together
     ╱  tion     ╲
    ╱─────────────╲
   ╱  Unit Tests   ╲   Many, fast, cheap
  ╱  (per function) ╲  Test one thing at a time
 ╱───────────────────╲
```

- **Unit tests:** Test individual functions. Fast (milliseconds), deterministic, easy to debug. Our `test_crc32_detects_corruption` is a unit test.
- **Integration tests:** Test multiple components together. Slower (seconds), may need setup/teardown. Our `test_recovery_after_writes` is an integration test.
- **End-to-end tests:** Test the complete system as a user would use it. Slow (seconds to minutes), may be flaky. Our golden SQL tests are end-to-end tests.

The ratio should be many unit tests, fewer integration tests, and few end-to-end tests. Unit tests catch most bugs quickly. Integration tests catch interface mismatches. End-to-end tests catch systemic issues.

### Coverage is not completeness

100% code coverage means every line of code was executed during testing. It does not mean every behavior was tested:

```rust,ignore
fn divide(a: i32, b: i32) -> i32 {
    a / b
}

#[test]
fn test_divide() {
    assert_eq!(divide(10, 2), 5); // 100% coverage!
}
// But divide(10, 0) panics — not covered by any test.
```

Property-based testing addresses this: instead of testing specific inputs, test properties across random inputs. But even property tests cannot test everything — the input space is infinite. The art of testing is choosing the right properties and the right edge cases.

### Mutation testing

A more powerful measure than coverage: **mutation testing**. The idea: automatically mutate your code (change `>` to `>=`, change `+` to `-`, remove a line) and check if any test fails. If a mutation does not cause a test failure, you have a gap in your test suite.

```
Original:  if count > 0 { ... }
Mutation:  if count >= 0 { ... }   ← does any test fail?

If no test fails, your tests do not distinguish between > and >=
for this variable. You need a test where count == 0.
```

Rust has the `cargo-mutants` tool for mutation testing. It is slow (it recompiles and runs tests for every mutation) but very effective at finding testing gaps.

---
