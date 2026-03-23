## System Design Corner: Query Planning in Production

### Rule-based vs cost-based planning

Our planner uses **rule-based planning**: fixed rules determine the plan structure. SELECT always becomes Project -> Filter -> Scan. There is no choice involved.

Production databases use **cost-based planning**. The planner generates multiple candidate plans and estimates the cost (I/O, CPU, memory) of each one, then picks the cheapest. For example, `SELECT * FROM users WHERE id = 42` could be executed as:

- **Plan A:** Full table scan, then filter. Cost: read all N rows.
- **Plan B:** Index lookup on `id`, fetch one row. Cost: read ~1 row.

A cost-based planner would pick Plan B. Our rule-based planner always picks Plan A (full scan), because we have not built indexes yet.

The cost model typically considers:
- **Table statistics** — row count, column cardinality (number of distinct values), data distribution
- **Index availability** — which columns have indexes, index type (B-tree, hash)
- **Join ordering** — for multi-table queries, the order of joins dramatically affects cost
- **Memory budget** — can the intermediate result fit in memory, or do we need to spill to disk?

PostgreSQL's planner, for example, considers over a dozen different join strategies and uses dynamic programming to find the optimal join order for up to ~12 tables.

### Plan caching and prepared statements

Parsing and planning are expensive relative to executing simple queries. For queries that are executed repeatedly with different parameters, databases support **prepared statements**:

```sql
-- Parse and plan once
PREPARE get_user AS SELECT * FROM users WHERE id = $1;

-- Execute many times with different values
EXECUTE get_user(42);
EXECUTE get_user(99);
EXECUTE get_user(7);
```

The database parses and plans the query once, storing the plan. Subsequent executions reuse the cached plan, substituting the parameter values. This saves the cost of lexing, parsing, and planning on every call.

In our database, implementing prepared statements would mean:
1. Parse the SQL once, producing an AST with parameter placeholders.
2. Plan the AST once, producing a plan with parameter slots.
3. On execution, substitute parameter values into the plan and execute.

We will not build this, but understanding the concept explains why separating planning from execution matters — you cannot cache a plan if planning and execution are interleaved.

### Query plan visualization

PostgreSQL's `EXPLAIN` command shows the plan tree. `EXPLAIN ANALYZE` executes the query and shows actual timing alongside estimates:

```
EXPLAIN ANALYZE SELECT name FROM users WHERE age > 18;

Seq Scan on users  (cost=0.00..35.50 rows=1000 width=32) (actual time=0.01..0.15 rows=847 loops=1)
  Filter: (age > 18)
  Rows Removed by Filter: 153
Planning Time: 0.05 ms
Execution Time: 0.25 ms
```

Our `Display` implementation for `Plan` is a simplified version of this. In Chapter 9 (optimizer) and Chapter 10 (executor), we could extend it to show estimated and actual costs.

---
