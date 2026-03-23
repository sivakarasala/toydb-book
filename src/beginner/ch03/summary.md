## What You Built

This chapter covered the most complex code in the book so far:

1. **Append-only log** — Every write goes to the end of the file. No data is ever overwritten. This is crash-safe by design.

2. **In-memory index** — A `HashMap` maps keys to file offsets for O(1) lookups.

3. **CRC32 checksums** — Each record has a checksum to detect corruption.

4. **Startup recovery** — The log file is scanned from beginning to end to rebuild the index.

5. **Tombstones** — Deleted keys are marked with empty-value records.

6. **File I/O** — You learned `OpenOptions`, `BufReader`, `BufWriter`, `seek`, `read_exact`, `write_all`, and `flush`.

7. **Error handling** — You learned the `?` operator, custom error enums, and the `From` trait.

---
