## Design Insight: Complexity Is Incremental

In *A Philosophy of Software Design*, Ousterhout warns that complexity does not arrive all at once. It creeps in, one small addition at a time. No single change seems harmful, but the accumulation is devastating:

> *"Complexity is incremental: it is not one particular thing that makes a system complicated, but the accumulation of dozens or hundreds of small things."*

This chapter is a perfect case study. Compare the Chapter 12 server:

```rust,ignore
for stream in listener.incoming() {
    let stream = stream?;
    handle_connection(stream)?;
}
```

With this chapter's server:

```rust,ignore
let db = Arc::new(Mutex::new(Database::new()));
let (shutdown_tx, _) = broadcast::channel::<()>(1);
let semaphore = Arc::new(Semaphore::new(MAX_CONNECTIONS));

loop {
    tokio::select! {
        result = listener.accept() => {
            let (stream, addr) = result?;
            let permit = semaphore.clone().try_acquire_owned()?;
            let db = Arc::clone(&db);
            let mut shutdown_rx = shutdown_tx.subscribe();
            tokio::spawn(async move {
                handle_connection_with_shutdown(stream, db, &mut shutdown_rx).await;
                drop(permit);
            });
        }
        _ = signal::ctrl_c() => {
            let _ = shutdown_tx.send(());
            break;
        }
    }
}
```

Each addition was justified:
- `Arc<Mutex<>>` — needed for shared database access
- `broadcast` channel — needed for shutdown signaling
- `Semaphore` — needed for connection limiting
- `tokio::select!` — needed to listen for both connections and shutdown
- `tokio::spawn` — needed for concurrent connection handling

No single addition was complex. But the accumulation means someone reading this code must understand async/await, `Arc`, `Mutex`, broadcast channels, semaphores, `select!`, and `spawn`. That is a lot of concepts to hold in your head simultaneously.

The lesson: before adding concurrency features, ask whether the simpler version is good enough. A sequential server that handles 100 queries per second might be sufficient for your workload. An async server is necessary at 10,000 queries per second, but the complexity cost is real. Justify it with data, not assumptions.

This principle applies broadly: async adds complexity — each `.await` point is a potential yield point that affects reasoning about state. `Arc<Mutex<>>` adds complexity — you must think about lock ordering and contention. Channels add complexity — you must think about backpressure and deadlocks. Use them when you need them, not because they seem sophisticated.

> *"Complexity is incremental: it is not one particular thing that makes a system complicated, but the accumulation of dozens or hundreds of small things."*
> — John Ousterhout

---
