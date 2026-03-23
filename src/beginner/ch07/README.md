# Chapter 7: SQL Parser — Building the AST

In the last chapter, we built a lexer that breaks SQL text into tokens. If you give it `SELECT name FROM users WHERE age > 18`, it produces a flat list: `[SELECT, name, FROM, users, WHERE, age, >, 18]`. That is progress, but it is not enough. The list tells us what words appear, but not how they relate to each other. Is `age > 18` a condition for filtering? Or is `age` a column we are selecting? The tokens carry no structure.

Think about diagramming a sentence in English class. The sentence "The cat sat on the mat" is just words until you mark "cat" as the subject, "sat" as the verb, and "on the mat" as a prepositional phrase. Diagramming reveals the structure hidden inside a flat string of words. That is exactly what a parser does for SQL.

This chapter transforms the flat token stream into an **Abstract Syntax Tree** (AST) -- a tree-shaped data structure where each node represents a meaningful piece of the SQL statement. `SELECT name FROM users WHERE age > 18` becomes a `Statement::Select` node with a column list `[name]`, a table `users`, and a WHERE clause that is itself a tree: `BinaryOp { left: Column("age"), op: Gt, right: Literal(Integer(18)) }`.

By the end of this chapter, you will have:

- An `Expression` enum that uses `Box` for recursive variants like `BinaryOp` and `UnaryOp`
- A `Statement` enum covering SELECT, INSERT, and CREATE TABLE
- A `Parser` struct with `peek()`, `advance()`, and `expect()` methods that consume tokens
- Operator precedence for correct parsing of WHERE clauses
- A test suite parsing real SQL into verified ASTs

---
