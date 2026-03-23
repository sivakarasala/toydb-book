## System Design Corner: SQL Parsing in Production

In a system design interview, the parser is one stage in the query processing pipeline. But production SQL parsing has several considerations that our simple parser does not address.

### Prepared statements

When an application runs the same query thousands of times with different parameters:

```sql
SELECT name FROM users WHERE id = 42
SELECT name FROM users WHERE id = 73
SELECT name FROM users WHERE id = 101
```

Parsing each one is wasteful — the structure is identical, only the value changes. **Prepared statements** solve this:

```sql
PREPARE get_user AS SELECT name FROM users WHERE id = $1
EXECUTE get_user(42)
EXECUTE get_user(73)
```

The `PREPARE` step parses once and stores the AST. Each `EXECUTE` substitutes the parameter into the stored AST and skips parsing entirely. This is a major performance win for OLTP workloads where the same queries run millions of times per day.

### Query plan caching

Going further, production databases cache not just the AST but the entire optimized execution plan. PostgreSQL's plan cache stores plans keyed by query string. If the same query arrives again, it skips parsing, planning, and optimization — it jumps straight to execution.

The tradeoff: cached plans may become stale when table statistics change. A plan that was optimal when the table had 100 rows may be terrible when it has 10 million rows. PostgreSQL addresses this with "generic" vs "custom" plans — after a few executions, it compares the generic plan cost against re-planning with current statistics.

### SQL injection and parameterized queries

Parsing is also where SQL injection attacks are prevented. Consider:

```python
# DANGEROUS: string interpolation
query = f"SELECT * FROM users WHERE name = '{user_input}'"
```

If `user_input` is `'; DROP TABLE users; --`, the parser sees:

```sql
SELECT * FROM users WHERE name = ''; DROP TABLE users; --'
```

Three statements. The second one drops the table. **Parameterized queries** prevent this by separating the SQL structure from the data:

```python
# SAFE: parameterized query
query = "SELECT * FROM users WHERE name = $1"
params = [user_input]
```

The parser only sees the template. The parameter value is never parsed as SQL — it is treated as a literal value regardless of its content. This is defense at the architecture level, not at the input validation level.

### Parse error recovery

Production SQL parsers need to handle malformed input gracefully. Our parser returns the first error and stops. PostgreSQL's parser attempts error recovery — it tries to identify where the error is, report a helpful message with a caret pointing to the exact position, and suggest fixes:

```
ERROR:  syntax error at or near "FORM"
LINE 1: SELECT name FORM users
                     ^
HINT:  Did you mean "FROM"?
```

> **Interview talking point:** *"SQL parsing in production goes beyond syntax analysis. Prepared statements amortize parse cost across executions — parse once, execute many. Plan caching extends this to the optimizer, skipping re-planning for repeated queries. Parameterized queries prevent SQL injection at the parsing layer by separating structure from data. And parse error recovery provides actionable diagnostics rather than opaque failures."*

---
