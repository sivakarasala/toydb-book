## Spotlight: Testing & Benchmarking

Every chapter has one **spotlight concept**. This chapter's spotlight is **testing and benchmarking** -- how Rust helps you verify correctness and measure performance.

### Why testing matters

Imagine you change the query optimizer to be faster. You run a quick test -- SELECT works. You deploy. Three days later, a user reports that DELETE queries silently skip rows. The optimizer change broke a code path you did not think to test.

Tests prevent this. A well-tested database has hundreds of tests that run in seconds. Every code change runs all of them. If the optimizer change breaks DELETE, the test fails immediately, before anyone is affected.

Rust makes testing especially powerful because the compiler already catches entire categories of bugs: null pointer dereferences, data races, use-after-free, buffer overflows. None of these can happen in safe Rust. So your tests can focus on **logic bugs** -- the kind the compiler cannot catch.

### Unit tests: `#[test]`

Rust's test framework is built into the language. You do not need to install a separate testing library. Any function annotated with `#[test]` is a test:

```rust
fn add(a: i32, b: i32) -> i32 {
    a + b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_positive() {
        assert_eq!(add(2, 3), 5);
    }

    #[test]
    fn test_add_negative() {
        assert_eq!(add(-1, 1), 0);
    }

    #[test]
    fn test_add_zero() {
        assert_eq!(add(0, 0), 0);
    }
}
```

Run all tests with:

```
$ cargo test
running 3 tests
test tests::test_add_positive ... ok
test tests::test_add_negative ... ok
test tests::test_add_zero ... ok

test result: ok. 3 passed; 0 failed
```

> **Programming Concept: `#[cfg(test)]`**
>
> The `#[cfg(test)]` attribute tells the compiler: "only include this module when running tests." The test code is not included in your release binary. This means tests add zero overhead to the compiled database -- they exist only in the test build.
>
> `use super::*;` imports everything from the parent module (where `add` is defined) so the tests can call the function.

### Assertion macros

Rust provides several assertion macros for tests:

| Macro | What it checks | Example |
|-------|---------------|---------|
| `assert!(expr)` | Expression is true | `assert!(result.is_ok())` |
| `assert_eq!(a, b)` | a equals b | `assert_eq!(count, 42)` |
| `assert_ne!(a, b)` | a does not equal b | `assert_ne!(name, "")` |
| `assert!(expr, "msg")` | True, with custom message | `assert!(x > 0, "x was {}", x)` |

When an assertion fails, Rust prints both the expected and actual values:

```
thread 'tests::test_add' panicked at 'assertion failed: `(left == right)`
  left: `6`,
 right: `5`'
```

This is why most types in your codebase should derive `Debug` -- it makes test failure messages informative. Without `Debug`, Rust cannot print the values, and you get a much less helpful error.

### Testing for panics

Sometimes you want to verify that a function panics (crashes) on bad input:

```rust,ignore
#[test]
#[should_panic(expected = "index out of bounds")]
fn test_invalid_log_index() {
    let log = RaftLog::new();
    let _entry = log.get(999).unwrap();  // should panic
}
```

The `#[should_panic]` attribute says: "This test passes if the function panics." The `expected` parameter checks that the panic message contains the given text.

### Testing for errors

More often, you want to verify that a function returns an error (not a panic):

```rust,ignore
#[test]
fn test_parse_invalid_sql() {
    let result = Parser::parse("SELECTT * FROMM users");
    assert!(result.is_err());

    let error_message = result.unwrap_err().to_string();
    assert!(
        error_message.contains("unexpected"),
        "Expected error about unexpected token, got: {}",
        error_message
    );
}
```

> **What Just Happened?**
>
> We covered three ways to test edge cases:
> 1. `assert!(result.is_ok())` -- verify success
> 2. `assert!(result.is_err())` -- verify failure
> 3. `#[should_panic]` -- verify that bad input causes a panic
>
> In production code, you should prefer returning `Err` over panicking. Panics crash the entire program. Errors can be handled gracefully.

---
