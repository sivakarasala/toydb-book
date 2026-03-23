## Design Insight: Define Errors Out of Existence

In *A Philosophy of Software Design*, Ousterhout argues that the best error-handling strategy is designing your system so errors cannot occur. MVCC is a perfect example.

Consider the alternative: lock-based concurrency control. Every read locks the data, every write locks the data, and you need to handle:

```
Error: lock timeout after 30s
Error: deadlock detected — aborting transaction A
Error: lock escalation from row to table (unexpected)
Error: lock held by crashed process — orphaned lock
```

Each of these is a runtime error that the application must handle. Deadlocks require retry logic. Timeouts require configuration tuning. Orphaned locks require a cleanup process. The error surface is enormous.

MVCC eliminates most of these errors by design:

- **No lock timeouts** — readers never lock, so they never wait
- **No deadlocks** — there are no locks to deadlock on
- **No orphaned locks** — there are no locks
- **No lock escalation** — there are no locks

The only remaining "error" is the write-write conflict, which is a clean, well-defined condition: "two transactions tried to modify the same key." The fix is equally clean: abort one and retry. One error case instead of five.

This is the power of defining errors out of existence. By choosing MVCC over locks, we did not just pick a "better" concurrency strategy — we removed entire categories of errors from the system. The code is simpler, the error handling is simpler, and the system is more reliable.

The lesson applies broadly: before writing error handling, ask whether you can redesign the API or the data model so the error is impossible. Type systems help (Rust's `Option` prevents null pointer errors), but design choices help more (MVCC prevents deadlock errors).

---
