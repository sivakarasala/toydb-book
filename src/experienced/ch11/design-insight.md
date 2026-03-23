## Design Insight: Complexity Layers

In *A Philosophy of Software Design*, Ousterhout discusses layered systems where each layer adds one concept. Our executor is a textbook example.

Each executor type adds exactly one concept:

| Executor | Concept added |
|----------|--------------|
| ScanExecutor | Read rows from storage |
| FilterExecutor | Evaluate predicate, skip non-matching rows |
| ProjectExecutor | Select/compute columns |
| NestedLoopJoinExecutor | Combine rows from two sources |
| HashJoinExecutor | Combine rows using a hash table |
| AggregateExecutor | Group rows, compute summaries |
| SortExecutor | Order rows by expressions |

These compose into arbitrarily complex queries:

```
SELECT department, COUNT(*), AVG(salary)
FROM employees
JOIN departments ON employees.dept_id = departments.id
WHERE salary > 50000
GROUP BY department
ORDER BY AVG(salary) DESC

Executor tree:
  Sort(AVG(salary) DESC)
    Aggregate(GROUP BY department, COUNT, AVG)
      Filter(salary > 50000)
        HashJoin(dept_id = id)
          Scan(employees)
          Scan(departments)
```

Seven operators, each with a simple interface (`next()` + `columns()`), compose into a query that joins two tables, filters, groups, aggregates, and sorts. No single operator understands the full query. Each one does its one job and passes rows to the next.

This is the power of the Volcano model and of layered design in general. Adding a new SQL feature (DISTINCT, LIMIT, HAVING, window functions) means implementing one new executor type. Existing executors do not change. The plan builder adds one new case. Everything else stays the same.

> *"The best way to reduce complexity is to eliminate it — not to add complexity to manage it."*
> — John Ousterhout

Each executor eliminates one aspect of the query's complexity. The composition of simple operators produces complex behavior, without any single component being complex itself.

---
