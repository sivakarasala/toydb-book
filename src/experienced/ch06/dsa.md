## DSA in Context: The Lexer as a Finite State Automaton

A lexer is a finite state automaton (FSA) — a machine with a fixed set of states that transitions between them based on input characters. Our lexer has four main states:

```
         ┌─────────┐
    ─────┤  START   │
         └────┬────┘
              │
    ┌─────────┼─────────┬────────────┐
    │         │         │            │
    ▼         ▼         ▼            ▼
┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐
│InIdent │ │InNumber│ │InString│ │  Emit  │
│        │ │        │ │        │ │ single │
└───┬────┘ └───┬────┘ └───┬────┘ └────────┘
    │          │          │
    ▼          ▼          ▼
  Emit       Emit       Emit
  token      token      token
```

**START:** Look at the current character and decide which state to enter.
- Letter or `_` -> InIdent
- Digit -> InNumber
- `'` -> InString
- Operator character -> Emit single token

**InIdent:** Keep reading while the character is alphanumeric or `_`. When a non-identifier character appears, emit the accumulated string as either a Keyword or an Ident token.

**InNumber:** Keep reading while the character is a digit. When a non-digit appears, parse the accumulated string as an i64 and emit a Number token.

**InString:** Keep reading until an unescaped `'` appears. Handle `''` as an escaped quote. Emit the accumulated string as a Str token. If EOF is reached first, emit an error.

### Regular expressions vs hand-written lexers

Many lexer generators (like flex, or Rust's `logos` crate) compile regular expressions into state machines. You write patterns like:

```
SELECT    -> Keyword(Select)
[0-9]+    -> Number(parse)
'[^']*'   -> String(extract)
[a-z_]+   -> Ident
```

The tool generates the state machine automatically. This is faster to write but harder to customize. Hand-written lexers are more work but give you complete control over error messages, edge cases, and performance.

Our SQL lexer is hand-written because:
1. The grammar is small enough to manage manually
2. We want excellent error messages ("`!` at position 5 — did you mean `!=`?")
3. It is a learning exercise — understanding the state machine is the point

### Time complexity

A lexer is O(N) where N is the input length. Each character is examined exactly once (peek is constant time, advance moves forward). There is no backtracking — once we decide we are in the InNumber state, we stay there until a non-digit appears. This is a key property of deterministic finite automata (DFA): each input character causes exactly one state transition.

---
