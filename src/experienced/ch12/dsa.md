## DSA in Context: Length-Prefixed Framing

TCP is a byte stream, not a message stream. If you send "HELLO" and then "WORLD", the receiver might read "HELLOW" and then "ORLD" — TCP does not preserve message boundaries. This is the **framing problem**: how does the receiver know where one message ends and the next begins?

### Common framing strategies

**1. Delimiter-based** — use a special byte sequence to mark the end of each message.

```
Message 1: "SELECT * FROM users\n"
Message 2: "INSERT INTO users VALUES (1, 'Alice')\n"
```

HTTP uses `\r\n\r\n` to delimit headers from body. The problem: what if the message itself contains the delimiter? You need escaping, which adds complexity.

**2. Length-prefixed** — send the message length first, then the message.

```
[4 bytes: 0x00000014][20 bytes: SELECT * FROM users]
[4 bytes: 0x00000028][40 bytes: INSERT INTO users...]
```

This is what our protocol uses. The receiver reads exactly 4 bytes (the length), then reads exactly that many bytes (the payload). No escaping needed, no delimiter collisions, and the receiver knows exactly how many bytes to expect.

**3. Fixed-size** — every message is exactly N bytes, padded if shorter.

```
[1024 bytes: "SELECT * FROM users" + 1005 bytes of padding]
```

Simple but wasteful. Used in some low-level protocols where message sizes are predictable.

### Why length-prefixed wins for databases

Database query results vary enormously in size — from 0 bytes (empty result) to gigabytes (full table scan). Length-prefixed framing handles this range naturally:

- 4-byte prefix supports messages up to 4 GB (2^32 bytes)
- No scanning for delimiters (O(1) to find message boundary, vs O(n) for delimiter scanning)
- No escaping overhead
- The receiver can pre-allocate the exact right buffer size

PostgreSQL uses length-prefixed framing. MySQL uses length-prefixed framing. Redis uses delimiter-based (CRLF). HTTP/1.1 uses a mix (headers are delimiter-based, body is length-prefixed via Content-Length).

### The MAX_MESSAGE_SIZE guard

Our protocol limits messages to 16 MB:

```rust
const MAX_MESSAGE_SIZE: u32 = 16 * 1024 * 1024;
```

Without this limit, a malformed or malicious length prefix of `0xFFFFFFFF` (4 GB) would cause the server to allocate 4 GB of memory. The limit is a safety valve: if the length exceeds the maximum, the connection is terminated with an error.

PostgreSQL limits message size to 1 GB. MySQL limits to ~1 GB by default (configurable with `max_allowed_packet`). Our 16 MB is generous for a toy database.

---
