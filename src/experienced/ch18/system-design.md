## System Design Corner: Observability and Monitoring

Building a database is step one. Running it in production is step two. You need to know what your database is doing — is it healthy? Is it slow? Is it about to run out of disk?

### The three pillars of observability

**1. Metrics** — numerical measurements over time:

```
toydb_query_duration_seconds{type="select"}    0.023
toydb_query_duration_seconds{type="insert"}    0.045
toydb_raft_term                                5
toydb_raft_commit_index                        12847
toydb_storage_keys_total                       5032
toydb_storage_bytes_total                      15728640
toydb_connections_active                       3
```

Metrics answer: "How fast? How many? How much?" They are cheap to collect (a counter increment per operation), cheap to store (a few bytes per data point), and easy to alert on ("if query latency p99 exceeds 100ms, page the on-call engineer").

**2. Logs** — discrete events with context:

```
2024-01-15T10:23:45Z INFO  [server] Client connected from 192.168.1.5:43210
2024-01-15T10:23:45Z INFO  [sql]    Executing: SELECT * FROM users WHERE id = 42
2024-01-15T10:23:45Z DEBUG [plan]   Plan: Scan(users) -> Filter(id=42)
2024-01-15T10:23:46Z INFO  [sql]    Query completed: 1 row, 12ms
2024-01-15T10:23:47Z WARN  [raft]   Heartbeat to node 3 timed out
2024-01-15T10:23:48Z ERROR [raft]   Node 3 unreachable, marking as failed
```

Logs answer: "What happened?" They are the narrative record of the system's behavior. Structured logging (JSON format) makes logs searchable and parseable.

**3. Traces** — the path of a single request through the system:

```
Trace: query-abc123
├─ server.handle_request         2ms
│  ├─ lexer.tokenize             0.1ms
│  ├─ parser.parse               0.3ms
│  ├─ planner.plan               0.2ms
│  ├─ optimizer.optimize         0.05ms
│  ├─ raft.propose               15ms
│  │  ├─ wal.append_sync         3ms
│  │  └─ replicate_to_followers  12ms
│  └─ executor.execute           1ms
│     └─ mvcc.scan               0.8ms
└─ total                         18.65ms
```

Traces answer: "Why is this request slow?" They connect the dots between logs and metrics, showing exactly where time is spent. Distributed tracing (OpenTelemetry) follows requests across multiple services.

### What to monitor in a database

| Metric | Why it matters |
|--------|---------------|
| Query latency (p50, p95, p99) | User experience |
| Queries per second | Capacity planning |
| Error rate | Health |
| Raft term | Leader stability |
| Raft commit index lag | Replication health |
| WAL size | Disk usage, compaction needed |
| Connection count | Load |
| Memory usage | Capacity |
| fsync latency | Disk health |

> **Interview talking point:** *"I would add observability in three layers: Prometheus metrics for query latency histograms, error rates, and Raft health indicators; structured logging with request IDs for debugging specific queries; and distributed tracing with OpenTelemetry spans for each processing stage (lex, parse, plan, optimize, execute, replicate). For alerting, I would set up PagerDuty alerts on p99 latency exceeding SLO, Raft leader instability (frequent elections), and WAL size exceeding the compaction threshold. The metrics endpoint would be a /metrics HTTP handler that Prometheus scrapes every 15 seconds."*

---
