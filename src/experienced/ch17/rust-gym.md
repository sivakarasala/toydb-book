## Rust Gym

Time for reps. These drills focus on modules, workspaces, and error propagation — the spotlight concepts for this chapter.

### Drill 1: EXPLAIN ANALYZE (Medium)

Add timing information to query execution. Wrap the executor to measure time spent in each phase, and return it as part of the response when the query starts with `EXPLAIN ANALYZE`.

```rust
use std::time::{Duration, Instant};

/// Execution timing for each phase.
struct QueryTiming {
    lex_time: Duration,
    parse_time: Duration,
    plan_time: Duration,
    optimize_time: Duration,
    execute_time: Duration,
}

impl QueryTiming {
    fn total(&self) -> Duration {
        self.lex_time + self.parse_time + self.plan_time
            + self.optimize_time + self.execute_time
    }

    fn display(&self) -> String {
        format!(
            "Lex: {:?}, Parse: {:?}, Plan: {:?}, Optimize: {:?}, Execute: {:?}, Total: {:?}",
            self.lex_time, self.parse_time, self.plan_time,
            self.optimize_time, self.execute_time, self.total()
        )
    }
}

/// Simulated query phases (replace with real implementations).
fn lex(sql: &str) -> Vec<String> {
    std::thread::sleep(Duration::from_micros(100));
    sql.split_whitespace().map(|s| s.to_string()).collect()
}

fn parse(tokens: Vec<String>) -> String {
    std::thread::sleep(Duration::from_micros(200));
    tokens.join(" ")
}

fn plan(ast: String) -> String {
    std::thread::sleep(Duration::from_micros(50));
    format!("Plan({})", ast)
}

fn optimize(plan: String) -> String {
    std::thread::sleep(Duration::from_micros(30));
    plan // no-op optimization
}

fn execute(plan: String) -> Vec<String> {
    std::thread::sleep(Duration::from_micros(500));
    vec![format!("Result of {}", plan)]
}

fn execute_with_timing(sql: &str) -> (Vec<String>, QueryTiming) {
    // TODO: Execute each phase and measure its duration
    todo!()
}

fn main() {
    let sql = "SELECT * FROM users WHERE id = 1";
    let (results, timing) = execute_with_timing(sql);
    println!("Results: {:?}", results);
    println!("Timing: {}", timing.display());
    assert!(timing.total() > Duration::from_micros(500));
    println!("All checks passed!");
}
```

<details>
<summary>Solution</summary>

```rust
use std::time::{Duration, Instant};

struct QueryTiming {
    lex_time: Duration,
    parse_time: Duration,
    plan_time: Duration,
    optimize_time: Duration,
    execute_time: Duration,
}

impl QueryTiming {
    fn total(&self) -> Duration {
        self.lex_time + self.parse_time + self.plan_time
            + self.optimize_time + self.execute_time
    }

    fn display(&self) -> String {
        format!(
            "Lex: {:?}, Parse: {:?}, Plan: {:?}, Optimize: {:?}, Execute: {:?}, Total: {:?}",
            self.lex_time, self.parse_time, self.plan_time,
            self.optimize_time, self.execute_time, self.total()
        )
    }
}

fn lex(sql: &str) -> Vec<String> {
    std::thread::sleep(Duration::from_micros(100));
    sql.split_whitespace().map(|s| s.to_string()).collect()
}

fn parse(tokens: Vec<String>) -> String {
    std::thread::sleep(Duration::from_micros(200));
    tokens.join(" ")
}

fn plan(ast: String) -> String {
    std::thread::sleep(Duration::from_micros(50));
    format!("Plan({})", ast)
}

fn optimize(p: String) -> String {
    std::thread::sleep(Duration::from_micros(30));
    p
}

fn execute(p: String) -> Vec<String> {
    std::thread::sleep(Duration::from_micros(500));
    vec![format!("Result of {}", p)]
}

fn time<F, T>(f: F) -> (T, Duration)
where
    F: FnOnce() -> T,
{
    let start = Instant::now();
    let result = f();
    let elapsed = start.elapsed();
    (result, elapsed)
}

fn execute_with_timing(sql: &str) -> (Vec<String>, QueryTiming) {
    let (tokens, lex_time) = time(|| lex(sql));
    let (ast, parse_time) = time(|| parse(tokens));
    let (planned, plan_time) = time(|| plan(ast));
    let (optimized, optimize_time) = time(|| optimize(planned));
    let (results, execute_time) = time(|| execute(optimized));

    let timing = QueryTiming {
        lex_time,
        parse_time,
        plan_time,
        optimize_time,
        execute_time,
    };

    (results, timing)
}

fn main() {
    let sql = "SELECT * FROM users WHERE id = 1";
    let (results, timing) = execute_with_timing(sql);
    println!("Results: {:?}", results);
    println!("Timing: {}", timing.display());
    assert!(timing.total() > Duration::from_micros(500));
    println!("All checks passed!");
}
```

The `time` helper is a generic function that takes any closure, runs it, and returns both the result and the elapsed duration. This is a common pattern for instrumentation — wrap each phase in `time(|| phase())` without changing the phase's implementation. The closure `FnOnce() -> T` captures variables by move, which is why `tokens`, `ast`, etc., are consumed when passed to the next phase.

</details>

### Drill 2: Linearizable Reads (Hard)

Implement a read lease mechanism. The leader tracks the last time a majority of followers confirmed it is still the leader. Reads are only served if the lease has not expired.

```rust
use std::time::{Duration, Instant};

struct ReadLease {
    /// When the lease was last confirmed.
    last_confirmed: Instant,
    /// How long the lease is valid.
    lease_duration: Duration,
    /// Whether this node believes it is the leader.
    is_leader: bool,
}

impl ReadLease {
    fn new(lease_duration: Duration) -> Self {
        // TODO
        todo!()
    }

    /// Called when a majority of followers acknowledge a heartbeat.
    fn confirm(&mut self) {
        // TODO
        todo!()
    }

    /// Check if we can serve reads.
    fn can_serve_read(&self) -> bool {
        // TODO
        todo!()
    }

    /// Called when we lose leadership.
    fn revoke(&mut self) {
        // TODO
        todo!()
    }
}

fn main() {
    let mut lease = ReadLease::new(Duration::from_millis(500));

    // Initially cannot serve reads (no confirmation yet)
    assert!(!lease.can_serve_read());

    // Become leader and confirm
    lease.is_leader = true;
    lease.confirm();
    assert!(lease.can_serve_read());

    // Wait for lease to expire
    std::thread::sleep(Duration::from_millis(600));
    assert!(!lease.can_serve_read());

    // Re-confirm
    lease.confirm();
    assert!(lease.can_serve_read());

    // Lose leadership
    lease.revoke();
    assert!(!lease.can_serve_read());

    println!("All checks passed!");
}
```

<details>
<summary>Solution</summary>

```rust
use std::time::{Duration, Instant};

struct ReadLease {
    last_confirmed: Instant,
    lease_duration: Duration,
    is_leader: bool,
}

impl ReadLease {
    fn new(lease_duration: Duration) -> Self {
        ReadLease {
            // Set to a time far in the past so the initial lease is expired
            last_confirmed: Instant::now() - lease_duration - Duration::from_secs(1),
            lease_duration,
            is_leader: false,
        }
    }

    fn confirm(&mut self) {
        self.last_confirmed = Instant::now();
    }

    fn can_serve_read(&self) -> bool {
        self.is_leader && self.last_confirmed.elapsed() < self.lease_duration
    }

    fn revoke(&mut self) {
        self.is_leader = false;
    }
}

fn main() {
    let mut lease = ReadLease::new(Duration::from_millis(500));

    assert!(!lease.can_serve_read());

    lease.is_leader = true;
    lease.confirm();
    assert!(lease.can_serve_read());

    std::thread::sleep(Duration::from_millis(600));
    assert!(!lease.can_serve_read());

    lease.confirm();
    assert!(lease.can_serve_read());

    lease.revoke();
    assert!(!lease.can_serve_read());

    println!("All checks passed!");
}
```

The read lease is a time-bounded assertion: "I was the leader as of time T, and my lease lasts D milliseconds, so I am still the leader until T+D." This is safe because Raft's election timeout is longer than the lease duration — a new leader cannot be elected until the current leader's heartbeats stop, which takes at least one election timeout. By setting the lease duration shorter than the election timeout, we guarantee that the lease expires before a new leader could be elected.

Real systems (etcd, CockroachDB) use this exact mechanism. The tradeoff: clock skew. If the leader's clock runs fast, its lease might expire too early (safe but reduces availability). If a follower's clock runs fast, it might start an election before the leader's lease expires (unsafe if the leader is still serving reads). This is why distributed systems care deeply about clock synchronization (NTP, PTP, or Google's TrueTime).

</details>

### Drill 3: Multi-Statement Transaction (Hard)

Implement a simple transaction that groups multiple SQL statements. All statements succeed or all are rolled back.

```rust
use std::collections::HashMap;

struct SimpleDb {
    data: HashMap<String, String>,
    // Transaction state
    pending: Option<HashMap<String, Option<String>>>, // key -> Some(new_value) or None (delete)
}

impl SimpleDb {
    fn new() -> Self {
        // TODO
        todo!()
    }

    fn begin(&mut self) -> Result<(), String> {
        // TODO
        todo!()
    }

    fn set(&mut self, key: &str, value: &str) -> Result<(), String> {
        // TODO
        todo!()
    }

    fn delete(&mut self, key: &str) -> Result<(), String> {
        // TODO
        todo!()
    }

    fn get(&self, key: &str) -> Option<String> {
        // TODO: should see uncommitted changes within the transaction
        todo!()
    }

    fn commit(&mut self) -> Result<(), String> {
        // TODO
        todo!()
    }

    fn rollback(&mut self) -> Result<(), String> {
        // TODO
        todo!()
    }
}

fn main() {
    let mut db = SimpleDb::new();
    db.data.insert("a".into(), "1".into());
    db.data.insert("b".into(), "2".into());

    // Transaction that commits
    db.begin().unwrap();
    db.set("a", "10").unwrap();
    db.set("c", "3").unwrap();
    assert_eq!(db.get("a"), Some("10".to_string())); // sees uncommitted
    db.commit().unwrap();
    assert_eq!(db.get("a"), Some("10".to_string()));
    assert_eq!(db.get("c"), Some("3".to_string()));

    // Transaction that rolls back
    db.begin().unwrap();
    db.set("a", "999").unwrap();
    db.delete("b").unwrap();
    assert_eq!(db.get("a"), Some("999".to_string())); // sees uncommitted
    assert_eq!(db.get("b"), None); // sees the delete
    db.rollback().unwrap();
    assert_eq!(db.get("a"), Some("10".to_string())); // rolled back
    assert_eq!(db.get("b"), Some("2".to_string()));  // rolled back

    println!("All checks passed!");
}
```

<details>
<summary>Solution</summary>

```rust
use std::collections::HashMap;

struct SimpleDb {
    data: HashMap<String, String>,
    pending: Option<HashMap<String, Option<String>>>,
}

impl SimpleDb {
    fn new() -> Self {
        SimpleDb {
            data: HashMap::new(),
            pending: None,
        }
    }

    fn begin(&mut self) -> Result<(), String> {
        if self.pending.is_some() {
            return Err("transaction already in progress".to_string());
        }
        self.pending = Some(HashMap::new());
        Ok(())
    }

    fn set(&mut self, key: &str, value: &str) -> Result<(), String> {
        let pending = self.pending.as_mut()
            .ok_or("no active transaction")?;
        pending.insert(key.to_string(), Some(value.to_string()));
        Ok(())
    }

    fn delete(&mut self, key: &str) -> Result<(), String> {
        let pending = self.pending.as_mut()
            .ok_or("no active transaction")?;
        pending.insert(key.to_string(), None);
        Ok(())
    }

    fn get(&self, key: &str) -> Option<String> {
        // Check pending changes first (read-your-writes)
        if let Some(pending) = &self.pending {
            if let Some(change) = pending.get(key) {
                return change.clone(); // Some(value) or None (deleted)
            }
        }
        // Fall through to committed data
        self.data.get(key).cloned()
    }

    fn commit(&mut self) -> Result<(), String> {
        let pending = self.pending.take()
            .ok_or("no active transaction")?;

        for (key, value) in pending {
            match value {
                Some(v) => { self.data.insert(key, v); }
                None => { self.data.remove(&key); }
            }
        }

        Ok(())
    }

    fn rollback(&mut self) -> Result<(), String> {
        self.pending.take()
            .ok_or("no active transaction")?;
        Ok(()) // just discard the pending changes
    }
}

fn main() {
    let mut db = SimpleDb::new();
    db.data.insert("a".into(), "1".into());
    db.data.insert("b".into(), "2".into());

    db.begin().unwrap();
    db.set("a", "10").unwrap();
    db.set("c", "3").unwrap();
    assert_eq!(db.get("a"), Some("10".to_string()));
    db.commit().unwrap();
    assert_eq!(db.get("a"), Some("10".to_string()));
    assert_eq!(db.get("c"), Some("3".to_string()));

    db.begin().unwrap();
    db.set("a", "999").unwrap();
    db.delete("b").unwrap();
    assert_eq!(db.get("a"), Some("999".to_string()));
    assert_eq!(db.get("b"), None);
    db.rollback().unwrap();
    assert_eq!(db.get("a"), Some("10".to_string()));
    assert_eq!(db.get("b"), Some("2".to_string()));

    println!("All checks passed!");
}
```

The `pending` field is `Option<HashMap<...>>` — `None` means no active transaction, `Some(...)` means a transaction is in progress. The `Option` replaces a boolean flag + separate buffer, and Rust's pattern matching makes the "no active transaction" error check natural. Commit applies all pending changes to the main data; rollback discards them with `.take()`. The `.take()` method moves the value out of the `Option`, replacing it with `None` — a clean ownership transfer that also resets the transaction state.

</details>

### Drill 4: Graceful Cluster Shutdown (Medium)

Implement a shutdown coordinator that waits for in-flight requests to complete before stopping the server.

```rust
use std::sync::{Arc, atomic::{AtomicBool, AtomicUsize, Ordering}};
use std::time::Duration;

struct ShutdownCoordinator {
    /// Set to true when shutdown is requested.
    shutting_down: Arc<AtomicBool>,
    /// Number of requests currently being processed.
    in_flight: Arc<AtomicUsize>,
}

impl ShutdownCoordinator {
    fn new() -> Self {
        // TODO
        todo!()
    }

    /// Called before processing a request. Returns false if
    /// the server is shutting down (reject the request).
    fn begin_request(&self) -> bool {
        // TODO
        todo!()
    }

    /// Called after a request is complete.
    fn end_request(&self) {
        // TODO
        todo!()
    }

    /// Initiate shutdown. Blocks until all in-flight requests complete
    /// or the timeout expires.
    fn shutdown(&self, timeout: Duration) -> bool {
        // TODO: returns true if clean shutdown, false if timed out
        todo!()
    }

    fn is_shutting_down(&self) -> bool {
        self.shutting_down.load(Ordering::SeqCst)
    }

    fn in_flight_count(&self) -> usize {
        self.in_flight.load(Ordering::SeqCst)
    }
}

fn main() {
    let coord = ShutdownCoordinator::new();

    // Simulate some in-flight requests
    assert!(coord.begin_request());
    assert!(coord.begin_request());
    assert_eq!(coord.in_flight_count(), 2);

    // Start shutdown — new requests should be rejected
    let coord_clone = ShutdownCoordinator {
        shutting_down: coord.shutting_down.clone(),
        in_flight: coord.in_flight.clone(),
    };

    let handle = std::thread::spawn(move || {
        coord_clone.shutdown(Duration::from_secs(5))
    });

    // Give the shutdown thread time to set the flag
    std::thread::sleep(Duration::from_millis(50));

    // New requests should be rejected
    assert!(!coord.begin_request());
    assert!(coord.is_shutting_down());

    // Complete the in-flight requests
    coord.end_request();
    coord.end_request();

    // Shutdown should complete cleanly
    let clean = handle.join().unwrap();
    assert!(clean);

    println!("All checks passed!");
}
```

<details>
<summary>Solution</summary>

```rust
use std::sync::{Arc, atomic::{AtomicBool, AtomicUsize, Ordering}};
use std::time::{Duration, Instant};

struct ShutdownCoordinator {
    shutting_down: Arc<AtomicBool>,
    in_flight: Arc<AtomicUsize>,
}

impl ShutdownCoordinator {
    fn new() -> Self {
        ShutdownCoordinator {
            shutting_down: Arc::new(AtomicBool::new(false)),
            in_flight: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn begin_request(&self) -> bool {
        // Check if shutting down BEFORE incrementing
        if self.shutting_down.load(Ordering::SeqCst) {
            return false;
        }
        self.in_flight.fetch_add(1, Ordering::SeqCst);
        // Double-check after incrementing (avoid race with shutdown)
        if self.shutting_down.load(Ordering::SeqCst) {
            self.in_flight.fetch_sub(1, Ordering::SeqCst);
            return false;
        }
        true
    }

    fn end_request(&self) {
        self.in_flight.fetch_sub(1, Ordering::SeqCst);
    }

    fn shutdown(&self, timeout: Duration) -> bool {
        self.shutting_down.store(true, Ordering::SeqCst);

        let start = Instant::now();
        while self.in_flight.load(Ordering::SeqCst) > 0 {
            if start.elapsed() > timeout {
                return false; // timed out
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        true
    }

    fn is_shutting_down(&self) -> bool {
        self.shutting_down.load(Ordering::SeqCst)
    }

    fn in_flight_count(&self) -> usize {
        self.in_flight.load(Ordering::SeqCst)
    }
}

fn main() {
    let coord = ShutdownCoordinator::new();

    assert!(coord.begin_request());
    assert!(coord.begin_request());
    assert_eq!(coord.in_flight_count(), 2);

    let coord_clone = ShutdownCoordinator {
        shutting_down: coord.shutting_down.clone(),
        in_flight: coord.in_flight.clone(),
    };

    let handle = std::thread::spawn(move || {
        coord_clone.shutdown(Duration::from_secs(5))
    });

    std::thread::sleep(Duration::from_millis(50));

    assert!(!coord.begin_request());
    assert!(coord.is_shutting_down());

    coord.end_request();
    coord.end_request();

    let clean = handle.join().unwrap();
    assert!(clean);

    println!("All checks passed!");
}
```

The double-check in `begin_request` is important. Without it, there is a race condition: a request could check `shutting_down` (sees false), then the shutdown thread sets the flag, then the request increments `in_flight`. The shutdown thread would see `in_flight > 0` and wait, but the request was accepted after shutdown started. The double-check closes this race: if shutdown happened between the first check and the increment, the second check catches it and decrements back.

This is a simplified version of the "graceful shutdown" pattern used in production HTTP servers (Hyper, Actix, Axum). The real implementations use `tokio::sync::Notify` or channels instead of polling, but the principle is the same: stop accepting new work, finish existing work, then exit.

</details>

---
