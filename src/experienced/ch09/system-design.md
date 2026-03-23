## System Design Corner: Query Optimization in Production

In a system design interview, discussing query optimization shows that you understand why databases are fast, not just that they are.

### Rule-based optimization (RBO) vs cost-based optimization (CBO)

Our optimizer is **rule-based**: it applies fixed transformations regardless of the data. "Push filters down" is always beneficial. "Fold constants" is always beneficial. Rules do not need statistics about the data.

Production databases also use **cost-based optimization**: they estimate the cost of different plans and choose the cheapest one. This requires statistics:

```
Table: users (1,000,000 rows)
  Column: age — min: 0, max: 120, distinct: 100, histogram: [...]
  Column: country — distinct: 195, most common: 'US' (40%), 'CN' (15%), ...

Query: SELECT name FROM users WHERE age > 30 AND country = 'US'

Plan A: Scan(users) → Filter(age > 30 AND country = 'US')
  Estimated cost: scan 1M rows, filter produces ~400K * 0.40 = ~160K rows

Plan B: IndexScan(users.country = 'US') → Filter(age > 30)
  Estimated cost: index lookup produces ~400K rows, filter produces ~280K rows

Plan C: IndexScan(users.age > 30) → Filter(country = 'US')
  Estimated cost: index lookup produces ~700K rows, filter produces ~280K rows

Cheapest: Plan B (if the country index exists and is selective enough)
```

The optimizer estimates how many rows each plan step produces (cardinality estimation), how much I/O each step costs (disk reads for scans, index lookups), and how much CPU each step costs (comparisons, hash computations). It then picks the cheapest plan.

### Join reordering

For queries with multiple joins, the order of joins matters enormously:

```sql
SELECT *
FROM orders o
JOIN customers c ON o.customer_id = c.id
JOIN products p ON o.product_id = p.id
WHERE c.country = 'US' AND p.category = 'Electronics'
```

If there are 10M orders, 1M customers, and 100K products:

- Join orders with customers first: 10M * (cost of looking up each customer) = expensive
- Filter customers to US first (400K), then join with orders: much cheaper
- Filter products to Electronics first (10K), then join: even cheaper

The optimizer considers different join orders and picks the one with the lowest estimated cost. With N tables, there are N! possible join orders — a combinatorial explosion. Production optimizers use dynamic programming to prune the search space.

### PostgreSQL's optimizer pipeline

PostgreSQL's optimizer has several stages:

```
1. Simplification (rule-based)
   - Constant folding (like our ConstantFolding rule)
   - Predicate normalization (convert OR to IN, flatten AND chains)
   - View expansion (inline view definitions)

2. Path generation
   - For each table: sequential scan, index scan (one per index), bitmap scan
   - For each join: nested loop, hash join, merge join
   - Generate all possible access paths

3. Cost estimation
   - Use table statistics (pg_statistic) to estimate row counts
   - Use cost model to estimate I/O and CPU cost per path
   - Account for caching, parallelism, disk vs SSD

4. Plan selection
   - Dynamic programming for join ordering
   - Pick lowest-cost path for each sub-plan
   - Handle subqueries, CTEs, window functions

5. Plan finalization
   - Add Sort nodes if ORDER BY is present
   - Add Limit nodes
   - Add Result nodes for returning data to the client
```

> **Interview talking point:** *"Our database has a rule-based optimizer with constant folding and filter pushdown. In production, I would add cost-based optimization with cardinality estimation to handle join reordering. The optimizer stores rules as trait objects — `Vec<Box<dyn OptimizerRule>>` — so adding new rules does not require modifying the optimizer core. Each rule is a tree transformation that rewrites plan nodes matching specific patterns. We apply rules in sequence and could extend this to fixed-point iteration for rules that interact."*

### Indexes and the optimizer

The optimizer cannot recommend using an index if it does not know indexes exist. In production databases, the optimizer queries the catalog to discover available indexes, then generates index scan plans alongside sequential scan plans. Our toydb does not have indexes yet, but the optimizer framework we built can easily support them: just add an `IndexScan` variant to `Plan` and a rule that converts `Filter(col = val, Scan(table))` into `IndexScan(table, col, val)` when an index exists on `col`.

---
