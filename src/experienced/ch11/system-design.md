## System Design Corner: Distributed Joins

In a single-server database, a hash join reads both tables from local storage. In a distributed database, data is spread across multiple nodes. How do you join tables when rows live on different machines?

### Broadcast join

If one table is small, send a complete copy to every node. Each node joins its local partition of the large table against the complete small table.

```
Node 1: users[1-1000]   + ALL orders → join locally
Node 2: users[1001-2000] + ALL orders → join locally
Node 3: users[2001-3000] + ALL orders → join locally
```

**Cost:** Network transfer of the small table to every node. Good when one table fits in memory on each node.

### Shuffle (repartition) join

Hash both tables by the join key and send rows to the node responsible for that hash value. Now all matching rows are on the same node.

```
Hash partition:
  users with id % 3 == 0 → Node 1    orders with user_id % 3 == 0 → Node 1
  users with id % 3 == 1 → Node 2    orders with user_id % 3 == 1 → Node 2
  users with id % 3 == 2 → Node 3    orders with user_id % 3 == 2 → Node 3

Then: each node does a local hash join
```

**Cost:** Network transfer of both tables (reshuffled). The standard approach for joining two large tables.

### Colocated join

If both tables are already partitioned by the join key (e.g., users and orders are both partitioned by user_id), matching rows are already on the same node. No network transfer needed.

```
Node 1: users[1-1000] + orders[user_id 1-1000] → join locally (no network!)
Node 2: users[1001-2000] + orders[user_id 1001-2000] → join locally
```

**Cost:** Zero network transfer. This is why data partitioning strategy is one of the most important design decisions in distributed databases. CockroachDB, TiDB, and Spanner all optimize for colocated joins.

> **Interview talking point:** *"Our hash join builds a hash table on the smaller table in O(m) and probes with the larger table in O(n), giving O(n+m) overall. For the nested loop join, we use O(n*m) which is fine for small tables or indexed lookups. In a distributed setting, we would choose between broadcast joins (small table sent to all nodes) and shuffle joins (both tables repartitioned by join key). Colocated joins are the fastest — zero network transfer — which is why partitioning strategy matters so much."*

---
