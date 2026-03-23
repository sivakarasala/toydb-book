## System Design Corner: Wire Protocols

In a system design interview, "how does the client talk to the database?" is a foundational question. The answer is: through a wire protocol — a binary format that defines how requests and responses are encoded on the network.

### PostgreSQL's wire protocol

PostgreSQL uses a message-based protocol. Every message starts with a type byte and a 4-byte length:

```
[type: 1 byte][length: 4 bytes BE][payload: N bytes]

Examples:
  'Q' + length + "SELECT * FROM users\0"    (simple query)
  'D' + length + column_count + col1 + ...   (data row)
  'Z' + length + status                      (ready for query)
```

The server and client take turns sending messages. The protocol is stateful — you must authenticate before querying, and you must send a Sync message after each query batch.

### MySQL's wire protocol

MySQL uses a similar but distinct format with sequence IDs for packet ordering:

```
[length: 3 bytes LE][sequence_id: 1 byte][payload: N bytes]
```

The 3-byte length limits individual packets to 16MB. Larger results are split across multiple packets.

### Design tradeoffs

| Aspect | Text protocol (HTTP/JSON) | Binary protocol (Postgres) |
|--------|--------------------------|---------------------------|
| Debugging | Easy — curl, browser | Hard — need wireshark or custom tools |
| Parsing speed | Slow — text parsing | Fast — fixed offsets |
| Bandwidth | High — verbose | Low — compact |
| Schema evolution | Easy — ignore unknown fields | Hard — must version carefully |
| Streaming | Difficult | Natural — message boundaries |
| Implementation | Any HTTP library | Custom parser needed |

Most new databases (CockroachDB, TiDB) implement the PostgreSQL wire protocol for compatibility — any PostgreSQL client library works out of the box. This is a powerful strategy: instead of building a new ecosystem, piggyback on an existing one.

> **Interview talking point:** *"Our database uses a binary wire protocol with type-tagged messages. Each message starts with a 1-byte type tag and a 4-byte big-endian length, followed by the payload. We chose binary over HTTP/JSON for two reasons: lower latency for high-throughput workloads, and natural support for streaming result sets. For development and debugging, we also support a text-based protocol on a separate port."*

---
