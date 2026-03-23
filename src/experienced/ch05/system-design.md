## System Design Corner: Transaction Isolation Levels

In a system design interview, you should know the four standard isolation levels and which anomalies each prevents.

### The anomaly hierarchy

| Anomaly | Description | Example |
|---------|-------------|---------|
| **Dirty read** | Reading uncommitted data | T1 writes x=5 (not committed), T2 reads x=5 |
| **Non-repeatable read** | Same query, different results | T1 reads x=10, T2 commits x=20, T1 reads x=20 |
| **Phantom read** | New rows appear between queries | T1 counts 5 users, T2 inserts a user, T1 counts 6 |
| **Lost update** | Two transactions overwrite each other | T1 reads x=10, T2 reads x=10, T1 writes x=11, T2 writes x=11 (should be 12) |

### The four isolation levels

| Level | Dirty Read | Non-repeatable Read | Phantom Read | Lost Update |
|-------|-----------|-------------------|-------------|-------------|
| Read Uncommitted | Possible | Possible | Possible | Possible |
| Read Committed | Prevented | Possible | Possible | Possible |
| Repeatable Read | Prevented | Prevented | Possible | Depends |
| Serializable | Prevented | Prevented | Prevented | Prevented |

### MVCC in real databases

**PostgreSQL** uses MVCC for all isolation levels. Its "Repeatable Read" is actually snapshot isolation (which is between Repeatable Read and Serializable in the hierarchy). Versions are stored in the same table as the data — old versions are called "dead tuples" and cleaned up by VACUUM.

**MySQL (InnoDB)** uses MVCC for reads and locks for writes. At Repeatable Read, it takes "gap locks" to prevent phantoms. This is stricter than PostgreSQL's Repeatable Read but has more lock contention.

**Our toydb** implements snapshot isolation — equivalent to PostgreSQL's Repeatable Read. Reads always see a consistent snapshot. Write conflicts are detected at commit time (in the full implementation).

> **Interview talking point:** *"Our database uses MVCC with snapshot isolation. Each transaction gets a consistent snapshot at begin time and sees no changes from concurrent transactions. Writers do not block readers, and readers do not block writers. Write-write conflicts are detected at commit time — if two transactions modify the same key, the second to commit is aborted and retried. This gives us serializable-equivalent behavior for most workloads without the overhead of full serializable isolation."*

---
