## System Design Corner: Connection Management at Scale

Real database servers manage connections at a level of sophistication far beyond what we have built. Understanding these patterns demonstrates depth in system design interviews.

### Connection pooling

Applications rarely create a new TCP connection per query. They use a **connection pool** — a bounded set of pre-established connections that are borrowed and returned:

```
Application Server                Connection Pool               Database Server
     │                                  │                              │
     │── get_connection() ──────────►   │                              │
     │◄─ Connection #3 ────────────── │                              │
     │                                  │                              │
     │── query("SELECT...") ──────────────────────────────────────►   │
     │◄─ ResultSet ──────────────────────────────────────────────── │
     │                                  │                              │
     │── return_connection(#3) ──────► │                              │
     │                                  │                              │
     │── get_connection() ──────────►   │                              │
     │◄─ Connection #3 (reused) ──── │                              │
```

Connection pools solve several problems:
1. **TCP handshake overhead**: establishing a TCP connection takes 1-3 round trips. Reusing connections amortizes this cost.
2. **Server resource limits**: each connection consumes server memory (buffers, session state, locks). Pooling bounds the maximum.
3. **Authentication overhead**: TLS handshakes and authentication are expensive. Pooling avoids repeating them per query.

### PgBouncer: a connection multiplexer

PgBouncer sits between applications and PostgreSQL, multiplexing many application connections onto fewer database connections:

```
100 app connections ──► PgBouncer ──► 20 PostgreSQL connections
```

Three pooling modes:
- **Session pooling**: one PG connection per client session (least efficient, most compatible)
- **Transaction pooling**: PG connections are released between transactions (good balance)
- **Statement pooling**: PG connections are released between statements (most efficient, but breaks multi-statement transactions)

### Connection lifecycle

Production databases track connections through a lifecycle:

```
CONNECTING ──► AUTHENTICATING ──► IDLE ──► ACTIVE ──► IDLE ──► ... ──► CLOSING
                                   │                    │
                                   │   timeout          │   query timeout
                                   ▼                    ▼
                                CLOSING              CLOSING
```

Timeouts at every stage:
- **Connect timeout**: how long to wait for TCP handshake (typically 5-30s)
- **Authentication timeout**: how long to wait for auth to complete (typically 10s)
- **Idle timeout**: how long an idle connection stays open (typically 5-30 minutes)
- **Query timeout**: how long a single query can run (configurable per query)
- **Statement timeout**: PostgreSQL-specific per-session timeout

### Load shedding

When a server is overloaded, accepting more connections makes things worse — each new connection consumes memory and CPU, slowing down existing queries, causing timeouts, which cause retries, which cause more load. This is a **cascading failure**.

Load shedding means rejecting requests early when the server is overloaded:

```rust,ignore
if active_connections > max_connections {
    // Reject immediately — better than accepting and timing out later
    return Err("server at capacity");
}
```

Our server does this with the semaphore pattern. Production servers use more sophisticated techniques:
- **Adaptive concurrency limiting**: adjust the limit based on response times (Netflix's concurrency-limiter)
- **Circuit breakers**: stop sending requests to a failing backend
- **Priority queues**: serve high-priority queries first, drop low-priority ones under load

> **Interview talking point:** *"Our async server uses Tokio's multi-threaded runtime to handle concurrent connections. Each connection is a lightweight task (~few hundred bytes of state), allowing thousands of concurrent connections on a single machine. We use a semaphore for connection limiting, broadcast channels for graceful shutdown signaling, and Arc<Mutex<>> for shared database access with short critical sections. For production, I would add connection pooling with PgBouncer-style multiplexing, adaptive concurrency limits based on response time percentiles, and connection lifecycle management with timeouts at each stage."*

---
