# Leader Election State Machine — "Who's in charge?"

Your distributed database has three servers maintaining a replicated log. One of them is the leader -- it accepts writes, replicates them, and decides when entries are committed. This works beautifully until the leader's power supply catches fire. The other two servers are still running, but nobody is accepting writes. Clients are timing out. The system is down.

Someone needs to become the new leader. But who? And how do two servers agree on which one should lead, without a third-party arbiter, without a shared file system, and without any guarantee about network delays? If both servers simultaneously decide "I'll be the leader," you get **split brain** -- two leaders accepting conflicting writes, corrupting your data.

Leader election solves this. It is the mechanism that makes a distributed system self-healing: when the leader fails, the remaining servers autonomously and safely choose a new one. Let's build the state machine that makes this possible.

---

## The Naive Way

The simplest approach: designate a fixed leader. Server 1 is always the leader. If it crashes, the system waits for it to come back:

```rust
fn main() {
    let servers = vec!["server-1", "server-2", "server-3"];
    let leader = "server-1"; // hardcoded

    // Simulate server-1 crashing
    let alive = vec![false, true, true]; // server-1 is down

    println!("Fixed leader: {}", leader);
    println!("Server status:");
    for (i, &server) in servers.iter().enumerate() {
        let status = if alive[i] { "ALIVE" } else { "CRASHED" };
        let role = if server == leader { " (LEADER)" } else { "" };
        println!("  {}: {}{}", server, status, role);
    }

    let leader_alive = alive[0];
    println!("\nLeader alive: {}", leader_alive);
    if !leader_alive {
        println!("System is DOWN. No writes accepted.");
        println!("Waiting for server-1 to recover...");
        println!("This could take seconds, minutes, or never.");
    }

    // The fundamental problem: single point of failure.
    // If the designated leader dies, the system stops.
    // We need the system to elect a new leader automatically.
}
```

A fixed leader is a **single point of failure**. The whole point of running multiple servers is surviving individual failures. If losing one server brings down the entire system, you have paid the complexity cost of distribution without gaining the reliability benefit.

You could try "if server 1 is unreachable, server 2 becomes leader." But "unreachable" is ambiguous in a network -- maybe server 1 is fine but the network between them is partitioned. Now both server 1 (thinking it is still leader) and server 2 (thinking server 1 is dead) accept writes. Split brain. Data corruption.

---

## The Insight

Think about how a club elects a new president after the current one resigns. The process has rules:

1. Any member can **nominate themselves** as a candidate.
2. They ask every other member for a **vote**.
3. Each member votes for at most **one candidate per election cycle**.
4. The candidate who receives votes from a **majority** becomes president.
5. If nobody gets a majority (a **split vote**), they wait a random amount of time and try again.

The randomized timeout is the crucial ingredient. If two candidates start their campaigns at exactly the same time, they might each get half the votes and nobody wins. But if candidate A waits 150ms before starting and candidate B waits 300ms, candidate A will ask for votes first, get the majority, and win before B even starts. The randomization breaks symmetry.

This is Raft's leader election. Each server is a **state machine** with three states:

- **Follower**: passive. Listens for heartbeats from the leader. If the leader goes silent for too long, becomes a candidate.
- **Candidate**: actively seeking votes. Sends vote requests to all other servers. Becomes leader if it gets a majority.
- **Leader**: accepts client requests, sends heartbeats to followers. Steps down if it discovers a higher term.

The **term** is the election cycle number. It monotonically increases. Each term has at most one leader. If a server sees a message with a higher term, it immediately steps down to follower -- it knows a newer election has happened.

---

## The Build

### The State Machine

Let's model the three states and the transitions between them:

```rust,ignore
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Role {
    Follower,
    Candidate,
    Leader,
}

#[derive(Debug)]
struct ElectionState {
    id: u64,            // this server's ID
    role: Role,
    current_term: u64,  // monotonically increasing election term
    voted_for: Option<u64>, // who we voted for in the current term
    votes_received: u64,    // votes received as a candidate
    cluster_size: u64,      // total servers in the cluster
    leader_id: Option<u64>, // who we think the leader is
}

impl ElectionState {
    fn new(id: u64, cluster_size: u64) -> Self {
        ElectionState {
            id,
            role: Role::Follower,
            current_term: 0,
            voted_for: None,
            votes_received: 0,
            cluster_size,
            leader_id: None,
        }
    }

    fn majority(&self) -> u64 {
        self.cluster_size / 2 + 1
    }
}
```

### State Transitions

The election state machine has four key transitions:

1. **Follower -> Candidate**: election timeout expires (leader is presumed dead)
2. **Candidate -> Leader**: received votes from a majority
3. **Candidate -> Follower**: discovered a higher term, or another leader was elected
4. **Leader -> Follower**: discovered a higher term

```rust,ignore
#[derive(Debug)]
enum Event {
    ElectionTimeout,           // haven't heard from leader
    ReceivedHeartbeat {        // leader sent a heartbeat
        term: u64,
        leader_id: u64,
    },
    ReceivedVoteRequest {      // another server wants our vote
        term: u64,
        candidate_id: u64,
        last_log_index: u64,
        last_log_term: u64,
    },
    ReceivedVote {             // response to our vote request
        term: u64,
        granted: bool,
    },
    DiscoveredHigherTerm {     // saw a message with a higher term
        term: u64,
    },
}

#[derive(Debug)]
enum Action {
    BecomeCandidate,
    BecomeLeader,
    BecomeFollower { term: u64 },
    SendVoteRequests,
    SendHeartbeats,
    GrantVote { to: u64, term: u64 },
    DenyVote { to: u64, term: u64 },
    NoAction,
}
```

### Processing Events

Each state handles events differently. This is the heart of the election protocol:

```rust,ignore
impl ElectionState {
    fn handle_event(&mut self, event: Event) -> Vec<Action> {
        match event {
            Event::ElectionTimeout => self.handle_election_timeout(),
            Event::ReceivedHeartbeat { term, leader_id } => {
                self.handle_heartbeat(term, leader_id)
            }
            Event::ReceivedVoteRequest { term, candidate_id, last_log_index, last_log_term } => {
                self.handle_vote_request(term, candidate_id, last_log_index, last_log_term)
            }
            Event::ReceivedVote { term, granted } => {
                self.handle_vote_response(term, granted)
            }
            Event::DiscoveredHigherTerm { term } => {
                self.step_down(term)
            }
        }
    }

    fn handle_election_timeout(&mut self) -> Vec<Action> {
        match self.role {
            Role::Follower | Role::Candidate => {
                // Start a new election
                self.current_term += 1;
                self.role = Role::Candidate;
                self.voted_for = Some(self.id); // vote for self
                self.votes_received = 1;        // count self-vote
                self.leader_id = None;

                vec![Action::BecomeCandidate, Action::SendVoteRequests]
            }
            Role::Leader => {
                // Leaders don't have election timeouts (they send heartbeats instead)
                vec![Action::SendHeartbeats]
            }
        }
    }

    fn handle_heartbeat(&mut self, term: u64, leader_id: u64) -> Vec<Action> {
        if term < self.current_term {
            // Stale heartbeat from an old leader -- ignore
            return vec![Action::NoAction];
        }

        if term > self.current_term {
            // New term -- step down unconditionally
            return self.step_down(term);
        }

        // Same term -- acknowledge the leader
        self.role = Role::Follower;
        self.leader_id = Some(leader_id);
        // Reset election timeout (the leader is alive)
        vec![Action::NoAction] // in a real system, this resets the timer
    }

    fn handle_vote_request(
        &mut self,
        term: u64,
        candidate_id: u64,
        _last_log_index: u64,
        _last_log_term: u64,
    ) -> Vec<Action> {
        if term < self.current_term {
            return vec![Action::DenyVote { to: candidate_id, term: self.current_term }];
        }

        if term > self.current_term {
            // Step down to follower for the new term
            self.current_term = term;
            self.role = Role::Follower;
            self.voted_for = None;
            self.leader_id = None;
        }

        // Grant vote if we haven't voted in this term yet
        // (or if we already voted for this candidate -- idempotent)
        let can_vote = match self.voted_for {
            None => true,
            Some(id) => id == candidate_id,
        };

        if can_vote {
            self.voted_for = Some(candidate_id);
            vec![Action::GrantVote { to: candidate_id, term: self.current_term }]
        } else {
            vec![Action::DenyVote { to: candidate_id, term: self.current_term }]
        }
    }

    fn handle_vote_response(&mut self, term: u64, granted: bool) -> Vec<Action> {
        if self.role != Role::Candidate || term != self.current_term {
            return vec![Action::NoAction];
        }

        if granted {
            self.votes_received += 1;
            if self.votes_received >= self.majority() {
                self.role = Role::Leader;
                self.leader_id = Some(self.id);
                return vec![Action::BecomeLeader, Action::SendHeartbeats];
            }
        }

        vec![Action::NoAction]
    }

    fn step_down(&mut self, new_term: u64) -> Vec<Action> {
        self.current_term = new_term;
        self.role = Role::Follower;
        self.voted_for = None;
        self.votes_received = 0;
        self.leader_id = None;
        vec![Action::BecomeFollower { term: new_term }]
    }
}
```

### Randomized Election Timeouts

The key to preventing split votes: each server picks a **random** election timeout between a minimum and maximum (e.g., 150-300ms). The server whose timeout fires first starts an election and usually wins before others even start:

```rust,ignore
use std::time::Duration;

fn random_election_timeout() -> Duration {
    // In production, use a proper random number generator.
    // Here we simulate the range 150-300ms.
    let min_ms = 150;
    let max_ms = 300;

    // Pseudo-random based on current time (for illustration)
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let random_ms = min_ms + (now % (max_ms - min_ms + 1) as u128) as u64;

    Duration::from_millis(random_ms)
}

// In the election loop:
// 1. Start a timer with random_election_timeout()
// 2. If we receive a heartbeat before the timer fires, reset it
// 3. If the timer fires, start an election
```

The randomization makes simultaneous elections rare. If 5 servers all lose their leader at the same instant, they pick timeouts of (say) 157ms, 243ms, 189ms, 271ms, 201ms. The 157ms server starts first, sends vote requests, and wins before the others even begin. Only if two servers pick nearly identical timeouts (within a network round-trip time of each other) do you get a split vote -- and even then, the next attempt uses fresh random values.

### Term Monotonicity

The term number is the backbone of Raft's safety. It acts as a **logical clock** that totally orders elections:

```rust,ignore
impl ElectionState {
    /// A server must reject any message with a term lower than its own.
    /// A server must step down if it sees a term higher than its own.
    /// This ensures there is at most one leader per term.
    fn validate_term(&mut self, incoming_term: u64) -> TermComparison {
        if incoming_term > self.current_term {
            // The sender is in a newer term -- we are stale
            self.step_down(incoming_term);
            TermComparison::Newer
        } else if incoming_term < self.current_term {
            // The sender is in an older term -- reject
            TermComparison::Older
        } else {
            TermComparison::Same
        }
    }
}

enum TermComparison {
    Newer,  // incoming term > ours: step down
    Same,   // same term: process normally
    Older,  // incoming term < ours: reject
}
```

Why does this guarantee at most one leader per term? Because each server votes for at most one candidate per term. A candidate needs a majority to become leader. Since two majorities always overlap, two candidates cannot both get majorities in the same term -- the overlapping server voted for one and will refuse the other.

### Leader Lease and Heartbeats

The leader maintains its authority by sending periodic heartbeats -- empty AppendEntries RPCs. Each heartbeat resets the followers' election timeouts. If the leader stops sending heartbeats (because it crashed or is network-partitioned), followers' timers expire and a new election begins:

```rust,ignore
/// The heartbeat interval must be much shorter than the election timeout.
/// Typical values: heartbeat = 50ms, election timeout = 150-300ms.
/// This gives followers at least 2-5 missed heartbeats before they
/// start an election, preventing false elections from temporary network blips.

const HEARTBEAT_INTERVAL_MS: u64 = 50;
const ELECTION_TIMEOUT_MIN_MS: u64 = 150;
const ELECTION_TIMEOUT_MAX_MS: u64 = 300;

// Invariant: HEARTBEAT_INTERVAL_MS << ELECTION_TIMEOUT_MIN_MS
// If heartbeat >= election_timeout, followers would constantly start elections.
```

---

## The Payoff

Let's simulate a complete election scenario: a leader crashes, a follower detects the failure, starts an election, and becomes the new leader:

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
enum Role {
    Follower,
    Candidate,
    Leader,
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::Follower => write!(f, "Follower"),
            Role::Candidate => write!(f, "Candidate"),
            Role::Leader => write!(f, "Leader"),
        }
    }
}

struct Server {
    id: u64,
    role: Role,
    current_term: u64,
    voted_for: Option<u64>,
    votes_received: u64,
    alive: bool,
}

impl Server {
    fn new(id: u64) -> Self {
        Server {
            id,
            role: Role::Follower,
            current_term: 0,
            voted_for: None,
            votes_received: 0,
            alive: true,
        }
    }

    fn start_election(&mut self) {
        self.current_term += 1;
        self.role = Role::Candidate;
        self.voted_for = Some(self.id);
        self.votes_received = 1; // vote for self
    }

    fn receive_vote_request(&mut self, candidate_id: u64, candidate_term: u64) -> bool {
        if !self.alive {
            return false;
        }

        if candidate_term < self.current_term {
            return false; // stale term
        }

        if candidate_term > self.current_term {
            self.current_term = candidate_term;
            self.role = Role::Follower;
            self.voted_for = None;
        }

        match self.voted_for {
            None => {
                self.voted_for = Some(candidate_id);
                true
            }
            Some(id) if id == candidate_id => true, // already voted for this candidate
            _ => false, // already voted for someone else
        }
    }

    fn become_leader(&mut self) {
        self.role = Role::Leader;
    }

    fn receive_heartbeat(&mut self, leader_term: u64, leader_id: u64) {
        if leader_term >= self.current_term {
            self.current_term = leader_term;
            self.role = Role::Follower;
            self.voted_for = None;
        }
    }
}

fn print_cluster(servers: &[Server], phase: &str) {
    println!("--- {} ---", phase);
    for s in servers {
        let leader_marker = if s.role == Role::Leader { " ***" } else { "" };
        let alive_marker = if s.alive { "" } else { " [CRASHED]" };
        println!("  Server {}: term={}, role={}, voted_for={:?}{}{}",
                 s.id, s.current_term, s.role,
                 s.voted_for, leader_marker, alive_marker);
    }
    println!();
}

fn main() {
    println!("=== Raft Leader Election Simulation ===\n");

    let mut servers = vec![
        Server::new(1),
        Server::new(2),
        Server::new(3),
    ];
    let cluster_size = servers.len() as u64;
    let majority = cluster_size / 2 + 1;
    println!("Cluster: {} servers, majority = {}\n", cluster_size, majority);

    // Phase 1: Initial election
    // Server 1 times out first (randomized timeout) and starts an election
    println!("Phase 1: Server 1's election timeout fires first");
    servers[0].start_election();
    println!("  Server 1 starts election, term -> {}", servers[0].current_term);

    // Server 1 requests votes from 2 and 3
    let term = servers[0].current_term;
    let vote_2 = servers[1].receive_vote_request(1, term);
    let vote_3 = servers[2].receive_vote_request(1, term);
    println!("  Server 2 votes: {}", vote_2);
    println!("  Server 3 votes: {}", vote_3);

    servers[0].votes_received += vote_2 as u64 + vote_3 as u64;
    println!("  Server 1 received {} votes (need {})", servers[0].votes_received, majority);

    if servers[0].votes_received >= majority {
        servers[0].become_leader();
        println!("  Server 1 becomes LEADER!");
    }
    print_cluster(&servers, "After initial election");

    // Phase 2: Leader sends heartbeats
    println!("Phase 2: Leader 1 sends heartbeats");
    let leader_term = servers[0].current_term;
    servers[1].receive_heartbeat(leader_term, 1);
    servers[2].receive_heartbeat(leader_term, 1);
    println!("  Followers reset their election timeouts");
    print_cluster(&servers, "Steady state");

    // Phase 3: Leader crashes!
    println!("Phase 3: Server 1 CRASHES!");
    servers[0].alive = false;
    print_cluster(&servers, "After crash");

    // Phase 4: Server 3 times out first and starts an election
    println!("Phase 4: Server 3's election timeout fires");
    servers[2].start_election();
    println!("  Server 3 starts election, term -> {}", servers[2].current_term);

    // Server 3 requests vote from server 2 (server 1 is dead)
    let term = servers[2].current_term;
    let vote_2 = servers[1].receive_vote_request(3, term);
    println!("  Server 2 votes for 3: {}", vote_2);

    // Server 1 is dead, no response
    println!("  Server 1: no response (crashed)");

    servers[2].votes_received += vote_2 as u64;
    println!("  Server 3 has {} votes (need {})", servers[2].votes_received, majority);

    if servers[2].votes_received >= majority {
        servers[2].become_leader();
        println!("  Server 3 becomes NEW LEADER!");
    }
    print_cluster(&servers, "After re-election");

    // Phase 5: Server 1 comes back
    println!("Phase 5: Server 1 recovers");
    servers[0].alive = true;
    // Server 1 still thinks it is leader in term 1
    println!("  Server 1 wakes up thinking it is leader (term {})", servers[0].current_term);

    // Server 3 sends a heartbeat with term 2
    let new_leader_term = servers[2].current_term;
    servers[0].receive_heartbeat(new_leader_term, 3);
    println!("  Server 1 receives heartbeat from server 3 with term {}", new_leader_term);
    println!("  Server 1 steps down to follower (term {} > {})",
             new_leader_term, 1);
    print_cluster(&servers, "Final state: cluster recovered");

    println!("Key properties:");
    println!("  - At most one leader per term");
    println!("  - Leader crash detected within election timeout (~150-300ms)");
    println!("  - New leader elected within one additional round trip");
    println!("  - Total failover time: ~300-600ms");
    println!("  - Stale leaders automatically step down when they see higher terms");
}
```

---

## Complexity Table

| Operation | Cost | Notes |
|-----------|------|-------|
| Election timeout detection | O(1) | Local timer expiry |
| Vote request broadcast | O(N) messages | N = cluster size |
| Vote collection | O(N) responses | Wait for majority |
| Term comparison | O(1) | Simple integer comparison |
| State transition | O(1) | Update role + term + voted_for |
| Election convergence | O(1) expected rounds | Randomization prevents split votes |
| Worst case (split votes) | O(k) rounds | k rounds until one candidate wins; expected k ~ 1-2 |
| Leader detection latency | O(heartbeat interval) | 50-100ms typically |
| Failover time | O(election timeout) | 150-300ms typically |

The randomized timeout makes split votes rare but not impossible. In practice, elections converge in 1-2 rounds. The probability of k consecutive split votes is roughly (1/2)^k -- exponentially unlikely. After 5 failed rounds (probability < 3%), the system adds jitter and retries.

---

## Where This Shows Up in Our Database

In Chapter 14, we implement the Raft election protocol for our replicated database. The state machine drives the server's behavior:

```rust,ignore
pub struct RaftServer {
    state: ElectionState,
    // ...
}

impl RaftServer {
    pub async fn run(&mut self) {
        loop {
            match self.state.role {
                Role::Follower => self.run_follower().await,
                Role::Candidate => self.run_candidate().await,
                Role::Leader => self.run_leader().await,
            }
        }
    }
}
```

Beyond our toydb, leader election is a critical component of many distributed systems:

- **Raft** (etcd, CockroachDB, TiKV) uses the term-based voting protocol we just built. Elections complete in 1-2 round trips, with failover times typically under 500ms.
- **ZAB** (ZooKeeper Atomic Broadcast) uses a similar leader election but with a different protocol. The candidate with the most up-to-date log is preferred, which reduces the catch-up work after election.
- **Viewstamped Replication** uses "view numbers" (equivalent to Raft's terms) and a designated "view change coordinator." It predates both Raft and Paxos but is less widely known.
- **Paxos** does not have a formal leader election protocol -- any proposer can propose at any time. In practice, Multi-Paxos implementations add leader election (often Raft-like) for performance, because leaderless Paxos has high contention.
- **Kubernetes** uses leader election for controller managers and schedulers. Only one instance of each runs at a time; the others are standby. The election uses etcd's lease mechanism (which itself uses Raft).

The state machine pattern -- modeling the server as Follower/Candidate/Leader with explicit transitions -- is remarkably powerful. It makes the protocol testable (feed events, check transitions), debuggable (log state changes), and verifiable (check invariants at each transition). This is why Raft was designed for understandability: the state machine is the specification.

---

## Try It Yourself

### Exercise 1: Pre-Vote Protocol

Raft has a problem: a server that is network-partitioned from the cluster keeps incrementing its term (it keeps timing out and starting elections). When it reconnects, its high term forces the current leader to step down, disrupting the cluster. Implement a **pre-vote** mechanism: before starting a real election, the candidate sends "pre-vote" requests. Other servers respond without incrementing their term. Only if the candidate gets a majority of pre-votes does it start a real election with term increment.

<details>
<summary>Solution</summary>

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
enum Role {
    Follower,
    Candidate,
    Leader,
}

struct Server {
    id: u64,
    role: Role,
    current_term: u64,
    voted_for: Option<u64>,
    leader_alive: bool, // does this server think the leader is alive?
}

impl Server {
    fn new(id: u64) -> Self {
        Server {
            id,
            role: Role::Follower,
            current_term: 0,
            voted_for: None,
            leader_alive: false,
        }
    }

    /// Handle a pre-vote request. Pre-vote does NOT change our term
    /// or voted_for. It just checks: "would I vote for this candidate
    /// if an election happened?"
    fn handle_pre_vote(&self, candidate_id: u64, candidate_term: u64) -> bool {
        // Deny if we think the leader is alive (prevents disruption)
        if self.leader_alive {
            return false;
        }

        // Deny if candidate's term is not higher than ours
        if candidate_term <= self.current_term {
            return false;
        }

        // Grant pre-vote (but don't change our state)
        true
    }

    /// Start a real election (only after getting majority pre-votes).
    fn start_real_election(&mut self) {
        self.current_term += 1;
        self.role = Role::Candidate;
        self.voted_for = Some(self.id);
    }

    fn receive_heartbeat(&mut self, term: u64) {
        if term >= self.current_term {
            self.current_term = term;
            self.role = Role::Follower;
            self.leader_alive = true;
        }
    }

    fn election_timeout(&mut self) {
        self.leader_alive = false;
    }
}

fn main() {
    let mut servers = vec![
        Server::new(1),
        Server::new(2),
        Server::new(3),
    ];
    let majority = 2u64;

    // Establish server 1 as leader in term 1
    servers[0].current_term = 1;
    servers[0].role = Role::Leader;
    for s in &mut servers[1..] {
        s.receive_heartbeat(1);
    }
    println!("=== Pre-Vote Protocol Demo ===\n");
    println!("Initial state: Server 1 is leader (term 1)");
    println!("All followers think leader is alive\n");

    // Scenario 1: Server 3 is partitioned and keeps timing out
    println!("--- Scenario 1: Partitioned server (without pre-vote) ---");
    let mut partitioned_term = 1u64;
    for _ in 0..10 {
        partitioned_term += 1; // would increment term each timeout
    }
    println!("Without pre-vote: partitioned server reaches term {}", partitioned_term);
    println!("When it reconnects, it disrupts the cluster!\n");

    // Scenario 2: Same situation but with pre-vote
    println!("--- Scenario 2: Partitioned server (with pre-vote) ---");
    let mut partitioned = Server::new(3);
    partitioned.current_term = 1; // starts at term 1

    for attempt in 1..=5 {
        partitioned.election_timeout();

        // Pre-vote: ask other servers (but they are unreachable)
        // Simulate: partitioned server cannot reach anyone
        let pre_votes = 0u64; // no responses from partition

        if pre_votes + 1 >= majority {
            partitioned.start_real_election();
            println!("  Attempt {}: pre-vote passed, starting real election", attempt);
        } else {
            println!("  Attempt {}: pre-vote failed ({} votes, need {}), term stays at {}",
                     attempt, pre_votes + 1, majority, partitioned.current_term);
        }
    }

    println!("\nWith pre-vote: partitioned server stays at term {}", partitioned.current_term);
    println!("When it reconnects, it does NOT disrupt the cluster!\n");

    // Scenario 3: Legitimate election with pre-vote
    println!("--- Scenario 3: Leader actually dies, legitimate election ---");
    // Leader dies, followers detect it
    servers[1].election_timeout();
    servers[2].election_timeout();

    // Server 2 starts pre-vote
    let pre_vote_term = servers[1].current_term + 1;
    let pv_from_3 = servers[2].handle_pre_vote(2, pre_vote_term);
    println!("Server 2 pre-votes: self=true, server3={}", pv_from_3);

    let pre_votes = 1 + pv_from_3 as u64; // self + server 3
    if pre_votes >= majority {
        servers[1].start_real_election();
        println!("Pre-vote passed! Server 2 starts real election (term {})",
                 servers[1].current_term);
    }
}
```

</details>

### Exercise 2: Election with Log Comparison

In Raft, a candidate's vote request includes its last log entry's index and term. A voter denies the vote if the candidate's log is less up-to-date than its own. Implement the **log comparison** rule: a log is more up-to-date if its last entry has a higher term, or if the terms are equal and the log is longer.

<details>
<summary>Solution</summary>

```rust
#[derive(Debug, Clone)]
struct LogSummary {
    last_index: u64,
    last_term: u64,
}

impl LogSummary {
    fn is_at_least_as_up_to_date_as(&self, other: &LogSummary) -> bool {
        // Raft's log comparison rule (Section 5.4.1):
        // 1. If the logs have different last terms, the one with the higher
        //    term is more up-to-date.
        // 2. If the logs end with the same term, the longer log is more
        //    up-to-date.
        if self.last_term != other.last_term {
            self.last_term > other.last_term
        } else {
            self.last_index >= other.last_index
        }
    }
}

struct Voter {
    id: u64,
    current_term: u64,
    voted_for: Option<u64>,
    log: LogSummary,
}

impl Voter {
    fn handle_vote_request(
        &mut self,
        candidate_id: u64,
        candidate_term: u64,
        candidate_log: &LogSummary,
    ) -> (bool, &'static str) {
        // Rule 1: Reject if candidate's term is lower
        if candidate_term < self.current_term {
            return (false, "candidate term too low");
        }

        // Step down if candidate's term is higher
        if candidate_term > self.current_term {
            self.current_term = candidate_term;
            self.voted_for = None;
        }

        // Rule 2: Already voted for someone else in this term
        if let Some(voted_for) = self.voted_for {
            if voted_for != candidate_id {
                return (false, "already voted for another candidate");
            }
        }

        // Rule 3: Candidate's log must be at least as up-to-date as ours
        if !candidate_log.is_at_least_as_up_to_date_as(&self.log) {
            return (false, "candidate log is less up-to-date");
        }

        // All checks passed -- grant vote
        self.voted_for = Some(candidate_id);
        (true, "granted")
    }
}

fn main() {
    println!("=== Log Comparison in Raft Elections ===\n");

    // Scenario 1: Candidate has higher last term
    println!("Scenario 1: Candidate has higher last term");
    let mut voter = Voter {
        id: 2,
        current_term: 3,
        voted_for: None,
        log: LogSummary { last_index: 10, last_term: 2 },
    };
    let candidate_log = LogSummary { last_index: 8, last_term: 3 };
    let (granted, reason) = voter.handle_vote_request(1, 4, &candidate_log);
    println!("  Voter log: index={}, term={}", 10, 2);
    println!("  Candidate log: index={}, term={}", 8, 3);
    println!("  Vote: {} ({})", if granted { "GRANTED" } else { "DENIED" }, reason);
    println!("  (Higher term wins, even though voter has more entries)\n");

    // Scenario 2: Same last term, candidate has shorter log
    println!("Scenario 2: Same last term, candidate has shorter log");
    let mut voter = Voter {
        id: 2,
        current_term: 3,
        voted_for: None,
        log: LogSummary { last_index: 10, last_term: 3 },
    };
    let candidate_log = LogSummary { last_index: 7, last_term: 3 };
    let (granted, reason) = voter.handle_vote_request(1, 4, &candidate_log);
    println!("  Voter log: index={}, term={}", 10, 3);
    println!("  Candidate log: index={}, term={}", 7, 3);
    println!("  Vote: {} ({})", if granted { "GRANTED" } else { "DENIED" }, reason);
    println!("  (Same term, but voter's log is longer)\n");

    // Scenario 3: Same last term, candidate has longer log
    println!("Scenario 3: Same last term, candidate has longer log");
    let mut voter = Voter {
        id: 2,
        current_term: 3,
        voted_for: None,
        log: LogSummary { last_index: 7, last_term: 3 },
    };
    let candidate_log = LogSummary { last_index: 10, last_term: 3 };
    let (granted, reason) = voter.handle_vote_request(1, 4, &candidate_log);
    println!("  Voter log: index={}, term={}", 7, 3);
    println!("  Candidate log: index={}, term={}", 10, 3);
    println!("  Vote: {} ({})", if granted { "GRANTED" } else { "DENIED" }, reason);
    println!("  (Candidate has more entries with same last term)\n");

    // Scenario 4: Already voted for another candidate
    println!("Scenario 4: Already voted for another candidate");
    let mut voter = Voter {
        id: 2,
        current_term: 4,
        voted_for: Some(3), // already voted for server 3
        log: LogSummary { last_index: 7, last_term: 3 },
    };
    let candidate_log = LogSummary { last_index: 10, last_term: 3 };
    let (granted, reason) = voter.handle_vote_request(1, 4, &candidate_log);
    println!("  Already voted for: server 3");
    println!("  Vote for server 1: {} ({})", if granted { "GRANTED" } else { "DENIED" }, reason);

    println!("\nThe log comparison rule ensures the elected leader has all");
    println!("committed entries. Since committed entries exist on a majority,");
    println!("and the candidate needs a majority of votes, at least one voter");
    println!("has the committed entry and will deny a candidate without it.");
}
```

</details>

### Exercise 3: Split Vote Simulation

Simulate 1,000 elections in a 5-node cluster where all servers start their election at slightly different times (random timeout between 150-300ms). Count how often a split vote occurs (no candidate gets a majority in round 1). What percentage of elections require multiple rounds?

<details>
<summary>Solution</summary>

```rust
fn main() {
    let cluster_size = 5u64;
    let majority = cluster_size / 2 + 1; // 3
    let num_simulations = 10_000;
    let mut single_round_wins = 0u64;
    let mut multi_round_elections = 0u64;
    let mut total_rounds = 0u64;
    let mut max_rounds = 0u64;

    // Simple deterministic pseudo-random for reproducibility
    let mut seed: u64 = 42;
    let mut next_random = || -> u64 {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };

    for _sim in 0..num_simulations {
        // Each server picks a random timeout between 150-300ms
        let mut timeouts: Vec<u64> = (0..cluster_size)
            .map(|_| 150 + (next_random() % 151))
            .collect();

        let mut rounds = 0u64;
        let mut elected = false;

        while !elected {
            rounds += 1;

            // Sort by timeout -- first server to timeout starts election
            let mut order: Vec<(u64, u64)> = timeouts.iter()
                .enumerate()
                .map(|(i, &t)| (i as u64, t))
                .collect();
            order.sort_by_key(|&(_, t)| t);

            // First server starts election
            let candidate = order[0].0;
            let candidate_timeout = order[0].1;

            // Servers whose timeout is within 10ms of the candidate
            // also start elections (simulating near-simultaneous timeouts)
            let competing_candidates: Vec<u64> = order.iter()
                .filter(|&&(_, t)| t <= candidate_timeout + 10)
                .map(|&(id, _)| id)
                .collect();

            if competing_candidates.len() == 1 {
                // Only one candidate -- it wins easily
                // It gets votes from all others (assuming no log issues)
                elected = true;
            } else {
                // Multiple candidates -- votes are split
                // Each non-candidate server votes for the first candidate
                // that reaches it (approximated by closest timeout)
                let mut votes: Vec<u64> = vec![0; cluster_size as usize];

                // Each candidate votes for itself
                for &c in &competing_candidates {
                    votes[c as usize] = 1; // self-vote
                }

                // Non-candidates vote for the candidate with the lowest timeout
                // (the one that reached them first)
                for &(server_id, _) in &order {
                    if !competing_candidates.contains(&server_id) {
                        // Vote for the candidate whose timeout was closest
                        // (simulate: distribute roughly evenly among candidates)
                        let chosen = competing_candidates[
                            (next_random() as usize) % competing_candidates.len()
                        ];
                        votes[chosen as usize] += 1;
                    }
                }

                // Check if any candidate got a majority
                let winner = votes.iter().enumerate()
                    .find(|(_, &v)| v >= majority);

                if winner.is_some() {
                    elected = true;
                } else {
                    // Split vote -- retry with new random timeouts
                    timeouts = (0..cluster_size)
                        .map(|_| 150 + (next_random() % 151))
                        .collect();
                }
            }

            if rounds > 100 {
                // Safety valve (should never happen in practice)
                elected = true;
            }
        }

        total_rounds += rounds;
        if rounds == 1 {
            single_round_wins += 1;
        } else {
            multi_round_elections += 1;
        }
        if rounds > max_rounds {
            max_rounds = rounds;
        }
    }

    let avg_rounds = total_rounds as f64 / num_simulations as f64;
    let single_pct = single_round_wins as f64 / num_simulations as f64 * 100.0;
    let multi_pct = multi_round_elections as f64 / num_simulations as f64 * 100.0;

    println!("=== Split Vote Simulation ({} elections, {} nodes) ===\n",
             num_simulations, cluster_size);
    println!("Single-round elections:   {} ({:.1}%)", single_round_wins, single_pct);
    println!("Multi-round elections:    {} ({:.1}%)", multi_round_elections, multi_pct);
    println!("Average rounds per election: {:.2}", avg_rounds);
    println!("Maximum rounds needed:    {}", max_rounds);
    println!();
    println!("The randomized timeout (150-300ms) makes split votes rare.");
    println!("Most elections complete in a single round.");
    println!("Even when split votes occur, they resolve quickly.");
}
```

</details>

---

## Recap

Leader election is the self-healing mechanism that makes distributed databases fault-tolerant. When the leader fails, followers detect the silence (election timeout), nominate themselves (become candidates), collect votes (majority quorum), and establish a new leader -- all within a few hundred milliseconds, with no human intervention.

The protocol's correctness rests on three invariants: each server votes for at most one candidate per term, a candidate needs a majority to win, and any two majorities overlap. These guarantee at most one leader per term. The randomized election timeout breaks symmetry to prevent split votes: the first server to time out usually wins the election before others even start.

The state machine pattern -- modeling each server as Follower, Candidate, or Leader with explicit event-driven transitions -- makes the protocol clean, testable, and debuggable. Every production Raft implementation follows this structure. The state machine is not just an implementation detail; it is the specification.
