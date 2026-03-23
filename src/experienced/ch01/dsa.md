## DSA in Context: Hash Tables

You just built a database on top of a hash table. Here is what is happening underneath.

### How HashMap works

A hash table stores key-value pairs in an array of *buckets*. To find which bucket a key belongs to:

1. **Hash** the key — run it through a hash function to produce a number
2. **Modulo** — take that number modulo the number of buckets to get an index
3. **Look up** — go directly to that bucket

```
Key: "user:1"
     │
     ▼
  hash("user:1") = 7392841028
     │
     ▼
  7392841028 % 16 = 4   (16 buckets)
     │
     ▼
  buckets[4] → ("user:1", "Alice")
```

### Performance characteristics

| Operation | Average case | Worst case |
|-----------|-------------|------------|
| `insert`  | O(1)        | O(n)       |
| `get`     | O(1)        | O(n)       |
| `remove`  | O(1)        | O(n)       |
| `contains_key` | O(1)  | O(n)       |

The worst case happens when many keys hash to the same bucket (*hash collision*). Rust's `HashMap` uses a technique called *Robin Hood hashing* with *SipHash* as the default hash function — chosen for collision resistance rather than raw speed. This makes it safe against hash-flooding denial-of-service attacks, which matters for a database.

### Hash tables vs B-trees in databases

Real databases use both:

- **Hash indexes** — O(1) exact lookups. Perfect for `WHERE id = 42`. Cannot do range queries (`WHERE id > 10 AND id < 50`) because hash order has no relation to key order.
- **B-tree indexes** — O(log n) lookups, but keys are stored in sorted order. Supports range queries, prefix queries, and ordered iteration. This is what most SQL databases use as their default index.

In Chapter 2, we will build a more sophisticated in-memory storage engine. In Chapter 3, we will add persistence — writing data to disk so it survives restarts. The humble HashMap you built today is the conceptual ancestor of every storage engine in this book.

---
