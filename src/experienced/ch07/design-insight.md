## Design Insight: Obvious Code

In *A Philosophy of Software Design*, Ousterhout argues that code should be **obvious** — a reader should understand what the code does without significant effort. The best code reads like prose.

Our parser methods demonstrate this principle. Look at `parse_select`:

```rust
fn parse_select(&mut self) -> Result<Statement, String> {
    self.expect_keyword(Keyword::Select)?;
    let columns = self.parse_select_columns()?;
    self.expect_keyword(Keyword::From)?;
    let from = self.expect_ident()?;
    let where_clause = if self.match_token(&Token::Keyword(Keyword::Where)) {
        Some(self.parse_expression(0)?)
    } else {
        None
    };
    Ok(Statement::Select { columns, from, where_clause })
}
```

Read it out loud: "Expect SELECT. Parse the columns. Expect FROM. Get the table name. If there is a WHERE, parse the expression; otherwise, None." The code *reads like the grammar it implements*. Someone who has never seen this code can tell exactly what it does, because the method names map directly to the concepts.

Contrast this with a non-obvious parser:

```rust
// Non-obvious: what does this do?
fn parse(&mut self) -> Result<Statement, String> {
    let t = self.next();
    let mut cols = vec![];
    loop {
        let c = self.next();
        if c.is_kw("FROM") { break; }
        if !c.is_comma() { cols.push(c); }
    }
    let tbl = self.next();
    // ...50 more lines of index manipulation
}
```

Both parse SELECT statements. The first is obvious. The second requires careful reading to understand. The difference is not cleverness — it is the investment in naming, helper methods, and decomposition.

Ousterhout's principle applies directly to recursive descent parsers. Each grammar rule becomes a method. Each method's name describes the grammar rule. The code's structure mirrors the grammar's structure. When you add a new SQL feature (like JOIN or GROUP BY), you add a new method with a descriptive name. The parser grows in a way that remains readable.

This is why recursive descent is the most popular parsing technique despite being less powerful than table-driven parsers (like LALR or PEG). It is obvious. You can read it, debug it, and extend it without understanding automata theory.

---
