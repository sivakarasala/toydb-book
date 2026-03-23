## System Design Corner: Database Wire Protocols

Real database wire protocols are more complex than our simple request-response model. Understanding them demonstrates depth in system design interviews.

### PostgreSQL's protocol

PostgreSQL uses a **message-based protocol** where each message has a 1-byte type identifier followed by a 4-byte length and a payload:

```
┌──────────┬──────────────────┬────────────────────────────────┐
│ 1 byte   │  4 bytes (i32)   │  N bytes (payload)             │
│ Msg Type │  Length (incl.    │  Message body                  │
│ ('Q','R')│  length itself)  │                                │
└──────────┴──────────────────┴────────────────────────────────┘
```

Message types include:
- `Q` — Query (simple query)
- `P` — Parse (prepared statement)
- `B` — Bind (bind parameters to prepared statement)
- `E` — Execute (execute prepared statement)
- `T` — RowDescription (column metadata)
- `D` — DataRow (one row of results)
- `C` — CommandComplete ("INSERT 0 1")
- `Z` — ReadyForQuery (server is idle, ready for next query)

A simple query flows like this:

```
Client -> Server: Q "SELECT * FROM users"
Server -> Client: T [column descriptions]
Server -> Client: D [row 1]
Server -> Client: D [row 2]
Server -> Client: D [row 3]
Server -> Client: C "SELECT 3"
Server -> Client: Z (ready for next query)
```

Each row is sent as a separate message. This is a **streaming protocol** — the client does not need to wait for all rows before processing. Our protocol sends all rows in a single response, which is simpler but requires the server to buffer the entire result set in memory.

### Connection pooling

In production, applications do not create a new TCP connection for every query. They use a **connection pool** — a set of pre-established connections that are borrowed and returned:

```
Application                     Connection Pool               Database
    |                                |                            |
    |-- borrow connection ---------->|                            |
    |<- connection 3 --------------- |                            |
    |                                |                            |
    |-- query on connection 3 -------|--------------------------->|
    |<- results on connection 3 -----|<---------------------------|
    |                                |                            |
    |-- return connection 3 -------->|                            |
```

Popular connection poolers like PgBouncer sit between the application and PostgreSQL, multiplexing many application connections onto fewer database connections. This reduces the load on PostgreSQL (which has per-connection overhead for process management, memory, and locks).

### Prepared statements

Our protocol sends SQL as a string for every query. This means the server must lex, parse, plan, and optimize the same query every time it is executed. **Prepared statements** separate query preparation from execution:

```
1. Prepare: "SELECT name FROM users WHERE id = $1"
   Server returns a statement handle (ID)

2. Execute: handle=1, params=[42]
   Server reuses the cached plan, just plugs in the parameters

3. Execute: handle=1, params=[99]
   Same plan, different parameters — no re-parsing
```

This is faster for repeated queries and also prevents SQL injection (parameters are sent separately from the SQL, so they cannot be interpreted as SQL syntax).

> **Interview talking point:** *"Our wire protocol uses length-prefixed framing — a 4-byte big-endian message length followed by a JSON payload. The server handles connections sequentially, which is simple but limits throughput to one client at a time. For production, I would add connection pooling, prepared statements (to avoid re-parsing the same query), and either async I/O or a thread-per-connection model for concurrent clients. PostgreSQL's protocol uses per-message type identifiers and streams result rows individually, which allows the client to process rows before the entire result set is materialized."*

---
