## Design Insight: Strategic vs Tactical Programming

In *A Philosophy of Software Design*, Ousterhout distinguishes between **tactical** and **strategic** programming:

**Tactical:** "Just make it work." The programmer is focused on shipping the current feature. They take shortcuts — copy-paste, hardcoded values, skipped abstractions. Each shortcut is small, but they accumulate into a tangled mess.

**Strategic:** "Make it right, so future work is easier." The programmer invests time in clean abstractions, even when the immediate task does not require them.

Building a proper lexer is a strategic investment. We could have skipped it:

```rust
// Tactical approach: parse SQL with string splitting
fn parse_select(sql: &str) -> Vec<String> {
    let parts: Vec<&str> = sql.split_whitespace().collect();
    // "SELECT" should be first...
    // Column names until "FROM"...
    // Table name after "FROM"...
    // Oh wait, what about "WHERE id = 42"?
    // What about string literals with spaces: 'hello world'?
    // What about parentheses: (a, b, c)?
    // ...this approach collapses.
    todo!()
}
```

The tactical approach works for `SELECT name FROM users` but breaks on anything with spaces in strings, parentheses, or operators without spaces (`id=42`). You would patch each case with more `split` and `trim` calls until the code is unreadable.

The strategic approach — building a proper lexer that handles every token type — takes more time now but pays off in every future chapter. The parser (Chapter 7) will consume tokens, not raw strings. It will never worry about whitespace, escaped quotes, or keyword casing. All of that complexity is handled once, in the lexer, and hidden from everything downstream.

This is the fundamental tradeoff Ousterhout identifies: tactical programming is faster for the current task but slower for the project. Strategic programming is slower for the current task but faster for the project. The total cost of tactical programming increases over time (each patch makes the next one harder). The total cost of strategic programming decreases over time (each abstraction makes the next feature easier).

The lexer is 150 lines of code. It will be used by every SQL feature for the rest of the book — `SELECT`, `INSERT`, `UPDATE`, `DELETE`, `CREATE TABLE`, `JOIN`, and anything else we add. That is a strategic investment.

---
