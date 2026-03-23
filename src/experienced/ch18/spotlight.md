## Spotlight: Testing & Benchmarking

Every chapter has one spotlight concept. This chapter's spotlight is **testing and benchmarking** — how Rust helps you verify correctness and measure performance.

### Unit tests: #[test] and assert macros

Rust's test framework is built into the language. Any function annotated with `#[test]` is a test:

```rust
fn add(a: i32, b: i32) -> i32 {
    a + b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(add(2, 3), 5);
    }

    #[test]
    fn test_add_negative() {
        assert_eq!(add(-1, 1), 0);
    }

    #[test]
    #[should_panic(expected = "overflow")]
    fn test_add_overflow() {
        let _ = add(i32::MAX, 1); // panics in debug mode
    }
}
```

Run them with `cargo test`. The `#[cfg(test)]` attribute means the `tests` module is only compiled when running tests — it does not appear in the release binary.

Key assertion macros:

| Macro | Purpose | Example |
|-------|---------|---------|
| `assert!(expr)` | Assert expression is true | `assert!(result.is_ok())` |
| `assert_eq!(a, b)` | Assert equality (requires `PartialEq` + `Debug`) | `assert_eq!(count, 42)` |
| `assert_ne!(a, b)` | Assert inequality | `assert_ne!(result, "")` |
| `assert!(expr, "msg")` | Assert with custom message | `assert!(x > 0, "x was {}", x)` |

When an assertion fails, Rust prints both the expected and actual values (thanks to `Debug`). This is why most types in your codebase should derive `Debug` — it makes test failures informative.

### Test organization

Rust has three levels of test organization:

**1. Unit tests** — inside the module they test, using `#[cfg(test)]`:

```rust,ignore
// src/sql/lexer.rs

pub fn tokenize(input: &str) -> Vec<Token> { ... }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_select() {
        let tokens = tokenize("SELECT * FROM users");
        assert_eq!(tokens[0], Token::Keyword(Keyword::Select));
    }
}
```

Unit tests can access private functions (they are inside the module). This is intentional — you should test internal logic, not just the public API.

**2. Integration tests** — in the `tests/` directory, each file is a separate crate:

```
tests/
├── sql_integration.rs    // tests SQL end-to-end
├── storage_test.rs       // tests storage engine
└── raft_test.rs          // tests Raft consensus
```

Integration tests can only use the crate's public API. They compile as separate binaries, so they test your crate as an external consumer would use it.

**3. Doc tests** — code examples in documentation comments:

```rust
/// Parses a SQL string into a statement.
///
/// # Examples
///
/// ```
/// let stmt = toydb_sql::parse("SELECT 1 + 1");
/// assert!(stmt.is_ok());
/// ```
pub fn parse(sql: &str) -> Result<Statement, ParseError> { ... }
```

Doc tests run with `cargo test`. They serve double duty: documentation and test. If the example does not compile, the test fails.

### Testing async code

For testing async functions (like our Tokio-based server), use `#[tokio::test]`:

```rust,ignore
#[tokio::test]
async fn test_server_accepts_connection() {
    let server = TestServer::start().await;
    let mut client = Client::connect(&server.addr()).await.unwrap();

    let response = client.query("SELECT 1").await.unwrap();
    assert_eq!(response.rows[0][0], "1");

    server.shutdown().await;
}
```

The `#[tokio::test]` attribute creates a Tokio runtime for the test. Without it, you would need to manually create a runtime with `tokio::runtime::Runtime::new()`.

### Benchmarking with criterion

Rust's built-in benchmarks (`#[bench]`) are unstable (nightly only). The `criterion` crate provides stable, statistical benchmarks:

```rust,ignore
// benches/storage_bench.rs

use criterion::{criterion_group, criterion_main, Criterion};
use toydb_storage::MvccStorage;

fn bench_insert(c: &mut Criterion) {
    c.bench_function("mvcc_insert", |b| {
        let mut storage = MvccStorage::new_in_memory();
        let mut key_counter = 0u64;

        b.iter(|| {
            key_counter += 1;
            let key = format!("key-{}", key_counter);
            storage.set(&key, "value").unwrap();
        });
    });
}

fn bench_get(c: &mut Criterion) {
    let mut storage = MvccStorage::new_in_memory();

    // Pre-populate
    for i in 0..10_000 {
        storage.set(&format!("key-{}", i), "value").unwrap();
    }

    c.bench_function("mvcc_get", |b| {
        let mut i = 0;
        b.iter(|| {
            i = (i + 1) % 10_000;
            storage.get(&format!("key-{}", i)).unwrap();
        });
    });
}

criterion_group!(benches, bench_insert, bench_get);
criterion_main!(benches);
```

Run with `cargo bench`. Criterion runs the benchmark multiple times, computes statistics (mean, standard deviation, confidence intervals), and compares against previous runs to detect regressions.

### Property-based testing with proptest

Traditional tests verify specific examples. Property-based tests verify *properties* — statements that should hold for all inputs:

```rust,ignore
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_lexer_never_panics(input in "\\PC{0,1000}") {
        // The lexer should never panic, regardless of input.
        // It should return Ok or Err, never crash.
        let _ = Lexer::new(&input).tokenize();
    }

    #[test]
    fn test_roundtrip_serialization(
        term in 0u64..1000,
        index in 0u64..1000,
        command in prop::collection::vec(any::<u8>(), 0..100),
    ) {
        let entry = LogEntry { term, index, command: command.clone() };
        let bytes = entry.serialize();
        let recovered = LogEntry::deserialize(&bytes).unwrap();
        assert_eq!(recovered, entry);
    }
}
```

`proptest` generates random inputs and checks that the property holds. If it finds a failing input, it *shrinks* it — finding the smallest input that still fails. This produces minimal, understandable test cases.

> **Coming from JS/Python/Go?**
>
> | Concept | JavaScript | Python | Go | Rust |
> |---------|-----------|--------|-----|------|
> | Unit test | Jest / Mocha | pytest / unittest | `testing` package | `#[test]` (built-in) |
> | Test runner | `npm test` | `pytest` | `go test` | `cargo test` |
> | Assertion | `expect(x).toBe(y)` | `assert x == y` | `t.Errorf(...)` | `assert_eq!(x, y)` |
> | Test file convention | `*.test.js` | `test_*.py` | `*_test.go` | `tests/*.rs` or `#[cfg(test)]` |
> | Property testing | fast-check | hypothesis | rapid | proptest |
> | Benchmarking | benchmark.js | timeit / pytest-benchmark | `testing.B` | criterion |
> | Mocking | jest.mock | unittest.mock | gomock | mockall |
>
> Rust's key advantage: the compiler catches entire categories of bugs that other languages rely on tests to find. Null pointer dereferences, data races, use-after-free, buffer overflows — none of these can occur in safe Rust, so you never need to test for them. Your tests can focus on *logic* bugs instead of *memory* bugs.
>
> The tradeoff: Rust does not have a built-in mocking framework, and its strong type system makes mocking harder. You typically use traits (interfaces) for dependency injection and create test implementations rather than mock objects. This results in more code but more reliable tests.

---
