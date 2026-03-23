## Rust Gym

### Drill 1: Exhaustive State Machine

Implement a traffic light state machine where the compiler ensures all transitions are handled:

```rust,ignore
#[derive(Debug, Clone, PartialEq)]
enum TrafficLight {
    Red,
    Yellow,
    Green,
}

impl TrafficLight {
    /// Advance to the next state.
    fn next(&self) -> TrafficLight {
        todo!()
    }

    /// How long this light stays on (in seconds).
    fn duration(&self) -> u64 {
        todo!()
    }

    /// Can vehicles proceed?
    fn can_go(&self) -> bool {
        todo!()
    }
}

fn main() {
    let mut light = TrafficLight::Red;
    for _ in 0..6 {
        println!("{:?}: duration={}s, can_go={}", light, light.duration(), light.can_go());
        light = light.next();
    }
}
```

Expected output:
```
Red: duration=30s, can_go=false
Green: duration=25s, can_go=true
Yellow: duration=5s, can_go=false
Red: duration=30s, can_go=false
Green: duration=25s, can_go=true
Yellow: duration=5s, can_go=false
```

<details>
<summary>Solution</summary>

```rust
#[derive(Debug, Clone, PartialEq)]
enum TrafficLight {
    Red,
    Yellow,
    Green,
}

impl TrafficLight {
    fn next(&self) -> TrafficLight {
        match self {
            TrafficLight::Red => TrafficLight::Green,
            TrafficLight::Green => TrafficLight::Yellow,
            TrafficLight::Yellow => TrafficLight::Red,
        }
    }

    fn duration(&self) -> u64 {
        match self {
            TrafficLight::Red => 30,
            TrafficLight::Green => 25,
            TrafficLight::Yellow => 5,
        }
    }

    fn can_go(&self) -> bool {
        match self {
            TrafficLight::Green => true,
            TrafficLight::Red | TrafficLight::Yellow => false,
        }
    }
}

fn main() {
    let mut light = TrafficLight::Red;
    for _ in 0..6 {
        println!("{:?}: duration={}s, can_go={}", light, light.duration(), light.can_go());
        light = light.next();
    }
}
```

Key insight: if you add a new variant (say `TrafficLight::FlashingYellow`), every `match` expression becomes a compile error until you handle the new state. Try it — add a variant and watch the compiler tell you exactly which functions need updating. This is why Rust enums are ideal for state machines.

</details>

### Drill 2: Enum with Data

Implement a connection state machine where each state carries different data:

```rust,ignore
#[derive(Debug)]
enum ConnectionState {
    Disconnected,
    Connecting { address: String, attempt: u32 },
    Connected { address: String, latency_ms: u64 },
    Error { message: String, retries_left: u32 },
}

impl ConnectionState {
    /// Attempt to connect.
    fn connect(address: &str) -> Self {
        todo!()
    }

    /// Connection succeeded.
    fn on_connected(self, latency_ms: u64) -> Self {
        todo!()
    }

    /// Connection failed.
    fn on_error(self, message: &str) -> Self {
        todo!()
    }

    /// Is the connection usable?
    fn is_connected(&self) -> bool {
        todo!()
    }

    /// Get a status string for display.
    fn status(&self) -> String {
        todo!()
    }
}

fn main() {
    let state = ConnectionState::connect("127.0.0.1:4000");
    println!("{}", state.status());

    let state = state.on_connected(5);
    println!("{}", state.status());

    let state = state.on_error("connection reset");
    println!("{}", state.status());
}
```

<details>
<summary>Solution</summary>

```rust
#[derive(Debug)]
enum ConnectionState {
    Disconnected,
    Connecting { address: String, attempt: u32 },
    Connected { address: String, latency_ms: u64 },
    Error { message: String, retries_left: u32 },
}

impl ConnectionState {
    fn connect(address: &str) -> Self {
        ConnectionState::Connecting {
            address: address.to_string(),
            attempt: 1,
        }
    }

    fn on_connected(self, latency_ms: u64) -> Self {
        match self {
            ConnectionState::Connecting { address, .. } => {
                ConnectionState::Connected { address, latency_ms }
            }
            other => other, // ignore if not connecting
        }
    }

    fn on_error(self, message: &str) -> Self {
        match self {
            ConnectionState::Connecting { address, attempt } => {
                if attempt < 3 {
                    ConnectionState::Connecting {
                        address,
                        attempt: attempt + 1,
                    }
                } else {
                    ConnectionState::Error {
                        message: message.to_string(),
                        retries_left: 0,
                    }
                }
            }
            ConnectionState::Connected { .. } => {
                ConnectionState::Error {
                    message: message.to_string(),
                    retries_left: 3,
                }
            }
            other => other,
        }
    }

    fn is_connected(&self) -> bool {
        matches!(self, ConnectionState::Connected { .. })
    }

    fn status(&self) -> String {
        match self {
            ConnectionState::Disconnected => "disconnected".to_string(),
            ConnectionState::Connecting { address, attempt } => {
                format!("connecting to {} (attempt {})", address, attempt)
            }
            ConnectionState::Connected { address, latency_ms } => {
                format!("connected to {} ({}ms)", address, latency_ms)
            }
            ConnectionState::Error { message, retries_left } => {
                format!("error: {} ({} retries left)", message, retries_left)
            }
        }
    }
}

fn main() {
    let state = ConnectionState::connect("127.0.0.1:4000");
    println!("{}", state.status());
    // connecting to 127.0.0.1:4000 (attempt 1)

    let state = state.on_connected(5);
    println!("{}", state.status());
    // connected to 127.0.0.1:4000 (5ms)

    let state = state.on_error("connection reset");
    println!("{}", state.status());
    // error: connection reset (3 retries left)
}
```

Notice that `on_connected` and `on_error` take `self` by value (not `&mut self`). This means the old state is consumed and a new state is returned. You cannot accidentally access the old state after a transition — it has been moved. This is the **typestate pattern** in Rust: state transitions are enforced by the type system through ownership transfer.

</details>

### Drill 3: Majority Counting

Implement a vote counter that determines election outcomes:

```rust,ignore
struct ElectionResult {
    total_nodes: usize,
    votes_for: HashSet<u64>,
    votes_against: HashSet<u64>,
}

impl ElectionResult {
    fn new(total_nodes: usize) -> Self {
        todo!()
    }

    fn add_vote(&mut self, node_id: u64, granted: bool) {
        todo!()
    }

    fn quorum_size(&self) -> usize {
        todo!()
    }

    /// Has the candidate won?
    fn is_won(&self) -> bool {
        todo!()
    }

    /// Has the candidate definitely lost (cannot reach majority even
    /// if all remaining nodes vote yes)?
    fn is_lost(&self) -> bool {
        todo!()
    }

    /// Is the election still undecided?
    fn is_pending(&self) -> bool {
        todo!()
    }
}
```

<details>
<summary>Solution</summary>

```rust
use std::collections::HashSet;

struct ElectionResult {
    total_nodes: usize,
    votes_for: HashSet<u64>,
    votes_against: HashSet<u64>,
}

impl ElectionResult {
    fn new(total_nodes: usize) -> Self {
        ElectionResult {
            total_nodes,
            votes_for: HashSet::new(),
            votes_against: HashSet::new(),
        }
    }

    fn add_vote(&mut self, node_id: u64, granted: bool) {
        if granted {
            self.votes_for.insert(node_id);
        } else {
            self.votes_against.insert(node_id);
        }
    }

    fn quorum_size(&self) -> usize {
        self.total_nodes / 2 + 1
    }

    fn is_won(&self) -> bool {
        self.votes_for.len() >= self.quorum_size()
    }

    fn is_lost(&self) -> bool {
        let remaining = self.total_nodes - self.votes_for.len() - self.votes_against.len();
        self.votes_for.len() + remaining < self.quorum_size()
    }

    fn is_pending(&self) -> bool {
        !self.is_won() && !self.is_lost()
    }
}

fn main() {
    // 5-node cluster: need 3 votes to win
    let mut election = ElectionResult::new(5);
    election.add_vote(1, true);  // self-vote
    assert!(election.is_pending());

    election.add_vote(2, true);
    assert!(election.is_pending());

    election.add_vote(3, true);  // majority!
    assert!(election.is_won());

    // 5-node cluster: 3 rejections = lost
    let mut election = ElectionResult::new(5);
    election.add_vote(1, true);   // self-vote
    election.add_vote(2, false);
    election.add_vote(3, false);
    election.add_vote(4, false);
    assert!(election.is_lost());  // even if node 5 votes yes, only 2 < 3

    println!("All election tests passed!");
}
```

The `is_lost` method is an optimization: if the candidate has received enough rejections that it cannot possibly reach a majority, it should stop waiting and start a new election (or step down). Without this check, the candidate would wait for a timeout, which wastes time.

</details>

### Drill 4: Deterministic Timeout

Implement a deterministic timer for testing — one that you can manually advance:

```rust,ignore
struct MockClock {
    current_time: u64,  // milliseconds since start
}

struct Timer {
    deadline: u64,
}

impl MockClock {
    fn new() -> Self {
        todo!()
    }

    fn advance(&mut self, ms: u64) {
        todo!()
    }

    fn now(&self) -> u64 {
        todo!()
    }

    fn set_timer(&self, duration_ms: u64) -> Timer {
        todo!()
    }
}

impl Timer {
    fn is_expired(&self, clock: &MockClock) -> bool {
        todo!()
    }

    fn remaining(&self, clock: &MockClock) -> u64 {
        todo!()
    }
}
```

<details>
<summary>Solution</summary>

```rust
struct MockClock {
    current_time: u64,
}

struct Timer {
    deadline: u64,
}

impl MockClock {
    fn new() -> Self {
        MockClock { current_time: 0 }
    }

    fn advance(&mut self, ms: u64) {
        self.current_time += ms;
    }

    fn now(&self) -> u64 {
        self.current_time
    }

    fn set_timer(&self, duration_ms: u64) -> Timer {
        Timer {
            deadline: self.current_time + duration_ms,
        }
    }
}

impl Timer {
    fn is_expired(&self, clock: &MockClock) -> bool {
        clock.now() >= self.deadline
    }

    fn remaining(&self, clock: &MockClock) -> u64 {
        if self.deadline > clock.now() {
            self.deadline - clock.now()
        } else {
            0
        }
    }
}

fn main() {
    let mut clock = MockClock::new();
    let timer = clock.set_timer(200); // 200ms election timeout

    assert!(!timer.is_expired(&clock));
    assert_eq!(timer.remaining(&clock), 200);

    clock.advance(100);
    assert!(!timer.is_expired(&clock));
    assert_eq!(timer.remaining(&clock), 100);

    clock.advance(100);
    assert!(timer.is_expired(&clock));
    assert_eq!(timer.remaining(&clock), 0);

    clock.advance(50);
    assert!(timer.is_expired(&clock));
    assert_eq!(timer.remaining(&clock), 0);

    println!("All timer tests passed!");
}
```

Deterministic time is essential for testing distributed systems. Real-time (`Instant::now()`) makes tests non-deterministic — they might pass or fail depending on how fast the machine is, what other processes are running, and cosmic rays. A mock clock lets you control time precisely: "advance 150ms, check that the timer has not expired. Advance 51ms more, check that it has."

Production Raft implementations like etcd and TiKV use injectable time sources for exactly this reason.

</details>

---
