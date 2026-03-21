# Chapter 14: Raft -- Leader Election

Your database runs on a single server. If that server's disk fails, the power goes out, or the process crashes, every byte of data is gone. Backups help, but they are always stale — you lose everything since the last backup. Production databases solve this with **replication**: keep copies of the data on multiple servers. If one dies, the others continue. But replication introduces a hard problem: if clients can write to any server, the copies can diverge. Server A says the balance is $100, server B says $150. Which is correct?

This is the **consensus problem** — getting multiple servers to agree on a single sequence of operations, even when servers crash, messages are delayed, and the network partitions. Raft is a consensus algorithm designed to be understandable. It was published in 2014 by Diego Ongaro and John Ousterhout (yes, the same Ousterhout whose design philosophy runs through this book) specifically because the previous state-of-the-art algorithm, Paxos, was notoriously difficult to understand and implement correctly.

This chapter implements Raft's leader election protocol. You will model node states as a Rust enum, implement the RequestVote RPC, manage election timeouts, and test leader election with a simulated network. The spotlight concept is **state machines in Rust** — using enums and match to model complex state transitions that the compiler verifies for completeness.

By the end of this chapter, you will have:

- A `NodeState` enum with `Follower`, `Candidate`, and `Leader` variants
- A `RaftNode` struct with term tracking, vote management, and peer communication
- Election timeout detection with randomized jitter
- The `RequestVote` RPC — sending, receiving, and counting votes
- A deterministic test harness that simulates a multi-node cluster
- A clear understanding of why distributed consensus is hard and how Raft tames the complexity

---

## Spotlight: State Machines in Rust

Every chapter has one spotlight concept. This chapter's spotlight is **state machines in Rust** — how enums, match expressions, and the type system combine to model state transitions with compile-time safety.

### State machines are everywhere

A state machine is a model with a finite number of **states**, a set of allowed **transitions** between states, and **actions** that occur on transitions. You already use state machines constantly:

- A TCP connection: LISTEN -> SYN_RECEIVED -> ESTABLISHED -> FIN_WAIT -> CLOSED
- An HTTP request: PENDING -> HEADERS_RECEIVED -> BODY_RECEIVED -> COMPLETE
- A database transaction: ACTIVE -> COMMITTED | ABORTED
- A Raft node: FOLLOWER -> CANDIDATE -> LEADER

The key property: at any moment, the system is in exactly one state, and only certain transitions are valid from that state. A FOLLOWER cannot become a LEADER directly — it must transition through CANDIDATE first.

### Enums as states

Rust enums are the natural representation for state machine states:

```rust
#[derive(Debug, Clone, PartialEq)]
enum NodeState {
    Follower,
    Candidate,
    Leader,
}
```

Unlike enums in C/C++ (which are just integers) or "enums" in Python/JavaScript (which are strings or objects), Rust enums are **algebraic data types**. Each variant can carry different data:

```rust,ignore
#[derive(Debug, Clone)]
enum NodeState {
    Follower {
        voted_for: Option<u64>,      // who we voted for in this term
        leader_id: Option<u64>,      // who we think the leader is
    },
    Candidate {
        votes_received: HashSet<u64>, // which peers have voted for us
    },
    Leader {
        next_index: HashMap<u64, u64>,  // for each peer: next log entry to send
        match_index: HashMap<u64, u64>, // for each peer: highest replicated entry
    },
}
```

Each state carries only the data relevant to that state. A follower does not need `next_index` — that is leader-specific bookkeeping. A leader does not need `voted_for` — voting happens during elections, not during leadership. The type system enforces this: you cannot access `next_index` when the node is a follower, because the data does not exist.

### Match expressions for state transitions

`match` is how you handle state transitions. The compiler ensures you handle every state:

```rust,ignore
fn handle_timeout(&mut self) {
    match &self.state {
        NodeState::Follower { .. } => {
            // Election timeout: become a candidate
            self.start_election();
        }
        NodeState::Candidate { .. } => {
            // Election timeout: start a new election
            self.start_election();
        }
        NodeState::Leader { .. } => {
            // Leaders do not have election timeouts.
            // They send heartbeats instead.
            self.send_heartbeats();
        }
    }
}
```

If you add a fourth state (say `PreCandidate` for the pre-vote protocol), the compiler flags every `match` expression that does not handle it. You cannot forget to handle a state — the code does not compile until every case is covered.

### The transition table

Raft's state transitions form a clear diagram:

```
                          ┌──────────────────────────┐
                          │                          │
                          ▼                          │
              ┌───────────────────┐                  │
     ┌───────►│     FOLLOWER      │◄──────┐          │
     │        └───────────────────┘       │          │
     │                 │                  │          │
     │   election      │                  │ discover │
     │   timeout       │                  │ higher   │
     │                 ▼                  │ term     │
     │        ┌───────────────────┐       │          │
     │        │    CANDIDATE      │───────┘          │
     │        └───────────────────┘                  │
     │                 │                             │
     │   wins          │                             │
     │   election      │                             │
     │                 ▼                             │
     │        ┌───────────────────┐                  │
     └────────│      LEADER       │──────────────────┘
              └───────────────────┘
              discover higher term
```

Four transitions:
1. **Follower -> Candidate**: election timeout fires (no heartbeat from leader)
2. **Candidate -> Leader**: receives votes from a majority of nodes
3. **Candidate -> Follower**: discovers a higher term (another node was elected)
4. **Leader -> Follower**: discovers a higher term (was partitioned, cluster moved on)

Every transition has a clear trigger. There is no Follower -> Leader transition — you must go through Candidate. There is no Leader -> Candidate transition — if a leader discovers a higher term, it steps down to follower directly.

### Why state machines in Rust

In languages without exhaustive pattern matching, state machines are fragile. A JavaScript `switch` statement with a missing `case` silently falls through. A Python `if/elif` chain with a missing branch silently does nothing. In Rust, a `match` with a missing variant is a compile error.

This matters enormously for consensus algorithms, where a missed state transition is not a minor bug — it is a correctness violation that can cause data loss. The compiler is your co-pilot: it verifies that every state is handled, every transition is accounted for, and every piece of state-specific data is properly accessed.

> **Coming from JS/Python/Go?**
>
> | Concept | JavaScript | Python | Go | Rust |
> |---------|-----------|--------|-----|------|
> | State enum | `const FOLLOWER = 'follower'` | `class State(Enum)` | `const (Follower = iota)` | `enum NodeState { Follower, ... }` |
> | State data | Separate fields on object | Separate fields on object | Separate fields on struct | Data inside enum variants |
> | Transitions | `switch(state)` | `if state ==` | `switch state` | `match state` |
> | Exhaustiveness | No checking | No checking | No checking | Compile-time error |
> | Invalid access | Runtime error (undefined) | Runtime error (AttributeError) | Runtime panic | Compile error |
>
> The Rust approach eliminates an entire class of bugs: accessing state-specific data when in the wrong state. In JavaScript, you might access `this.nextIndex` when the node is a follower — it returns `undefined`, and the bug shows up much later. In Rust, the compiler prevents this: `next_index` only exists inside `NodeState::Leader { next_index, .. }`, and you can only access it after matching that variant.

---

## Exercise 1: The RaftNode Struct

**Goal:** Define the core data structures for a Raft node — the node state enum, the RaftNode struct, and the message types for RPC communication.

### Step 1: Define the node state

Create `src/raft.rs`:

```rust,ignore
// src/raft.rs

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

/// A unique identifier for a node in the cluster.
pub type NodeId = u64;

/// A Raft term number. Terms are monotonically increasing.
/// Each term begins with an election. At most one leader is elected per term.
pub type Term = u64;

/// The state of a Raft node.
#[derive(Debug, Clone, PartialEq)]
pub enum NodeState {
    /// Passive: responds to RPCs, does not initiate.
    /// Becomes candidate if election timeout fires without hearing from a leader.
    Follower,
    /// Actively seeking votes. Transitions to Leader if it wins,
    /// or back to Follower if it discovers a higher term.
    Candidate,
    /// Manages the cluster: sends heartbeats, replicates log entries.
    /// Steps down to Follower if it discovers a higher term.
    Leader,
}
```

### Step 2: Define the RaftNode struct

```rust,ignore
/// A Raft node.
pub struct RaftNode {
    /// This node's unique ID.
    pub id: NodeId,
    /// IDs of all other nodes in the cluster.
    pub peers: Vec<NodeId>,
    /// Current state (Follower, Candidate, or Leader).
    pub state: NodeState,
    /// Latest term this node has seen.
    /// Monotonically increasing — never decreases.
    pub current_term: Term,
    /// ID of the candidate this node voted for in the current term.
    /// None if it has not voted yet.
    pub voted_for: Option<NodeId>,
    /// ID of the current leader (as known by this node).
    pub leader_id: Option<NodeId>,
    /// When the election timer was last reset.
    /// If enough time passes without reset, the node starts an election.
    pub election_deadline: Instant,
    /// The election timeout duration for this node.
    /// Randomized to prevent split votes.
    pub election_timeout: Duration,
    /// Set of nodes that voted for this node in the current election.
    /// Only meaningful when state is Candidate.
    pub votes_received: HashSet<NodeId>,
}
```

Why separate fields instead of data inside enum variants? For a learning implementation, separate fields are clearer. You can see all the state at a glance, and the code is easier to debug. A production implementation might encode state-specific data inside the variants (as shown in the spotlight section), but the clarity tradeoff favors simplicity here.

### Step 3: Implement constructors and helpers

```rust,ignore
impl RaftNode {
    /// Create a new Raft node as a follower.
    pub fn new(id: NodeId, peers: Vec<NodeId>) -> Self {
        let election_timeout = Self::random_election_timeout();
        RaftNode {
            id,
            peers,
            state: NodeState::Follower,
            current_term: 0,
            voted_for: None,
            leader_id: None,
            election_deadline: Instant::now() + election_timeout,
            election_timeout,
            votes_received: HashSet::new(),
        }
    }

    /// Generate a random election timeout between 150ms and 300ms.
    /// The randomization prevents all nodes from starting elections
    /// at the same time, which would cause repeated split votes.
    fn random_election_timeout() -> Duration {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        use std::time::SystemTime;

        // Simple pseudo-random based on current time
        // (In production, use a proper RNG)
        let mut hasher = DefaultHasher::new();
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .hash(&mut hasher);
        let hash = hasher.finish();

        let min_ms = 150;
        let max_ms = 300;
        let ms = min_ms + (hash % (max_ms - min_ms));
        Duration::from_millis(ms)
    }

    /// Reset the election timer. Called when:
    /// - Receiving a valid heartbeat from the leader
    /// - Granting a vote to a candidate
    /// - Starting a new election
    pub fn reset_election_timer(&mut self) {
        self.election_timeout = Self::random_election_timeout();
        self.election_deadline = Instant::now() + self.election_timeout;
    }

    /// Check if the election timer has expired.
    pub fn election_timeout_elapsed(&self) -> bool {
        Instant::now() >= self.election_deadline
    }

    /// The number of nodes needed for a majority (including self).
    pub fn quorum_size(&self) -> usize {
        (self.peers.len() + 1) / 2 + 1
    }

    /// Whether this node has received enough votes to win the election.
    pub fn has_quorum(&self) -> bool {
        self.votes_received.len() >= self.quorum_size()
    }
}
```

### Step 4: Define RPC message types

Raft uses two RPCs: `RequestVote` (for elections) and `AppendEntries` (for log replication and heartbeats). This chapter implements `RequestVote`:

```rust,ignore
/// Messages exchanged between Raft nodes.
#[derive(Debug, Clone)]
pub enum RaftMessage {
    /// Sent by candidates to request votes from other nodes.
    RequestVote {
        /// Candidate's term.
        term: Term,
        /// ID of the candidate requesting the vote.
        candidate_id: NodeId,
        /// Index of candidate's last log entry (for log completeness check).
        last_log_index: u64,
        /// Term of candidate's last log entry.
        last_log_term: Term,
    },
    /// Response to a RequestVote RPC.
    RequestVoteResponse {
        /// Current term of the responding node (for candidate to update itself).
        term: Term,
        /// True if the responding node granted its vote.
        vote_granted: bool,
    },
    /// Sent by the leader to replicate log entries and as heartbeats.
    /// (Implemented in Chapter 15 — included here for completeness.)
    AppendEntries {
        /// Leader's term.
        term: Term,
        /// Leader's ID.
        leader_id: NodeId,
        /// Index of log entry immediately preceding the new ones.
        prev_log_index: u64,
        /// Term of prev_log_index entry.
        prev_log_term: Term,
        /// Log entries to store (empty for heartbeat).
        entries: Vec<LogEntry>,
        /// Leader's commit index.
        leader_commit: u64,
    },
    /// Response to an AppendEntries RPC.
    AppendEntriesResponse {
        /// Current term of the responding node.
        term: Term,
        /// True if the follower's log matched and entries were appended.
        success: bool,
        /// The index of the last entry the follower has (for optimization).
        match_index: u64,
    },
}

/// A single entry in the Raft log.
#[derive(Debug, Clone)]
pub struct LogEntry {
    /// The term when this entry was received by the leader.
    pub term: Term,
    /// The index of this entry in the log (1-based).
    pub index: u64,
    /// The command to apply to the state machine.
    pub command: Vec<u8>,
}
```

### Step 5: Understand the design

```
RaftNode {
    id: 1,
    peers: [2, 3, 4, 5],         // 5-node cluster
    state: Follower,
    current_term: 3,               // we are in term 3
    voted_for: Some(2),            // we voted for node 2 in term 3
    leader_id: Some(2),            // node 2 is the leader
    election_deadline: Instant,    // when our timer fires
    election_timeout: 237ms,       // randomized timeout
    votes_received: {},            // empty (not in election)
}
```

A Raft cluster has 2F+1 nodes to tolerate F failures. A 5-node cluster tolerates 2 failures. A 3-node cluster tolerates 1 failure. The `peers` list does not include the node itself.

The **term** is Raft's logical clock. Every message carries a term number. If a node receives a message with a higher term, it immediately updates its term and reverts to follower state. This ensures that stale leaders (ones that were partitioned and did not know a new leader was elected) step down immediately upon rejoining the cluster.

<details>
<summary>Hint: Why randomized election timeouts?</summary>

Without randomization, all followers would timeout at the same instant and start elections simultaneously. Each would vote for itself, splitting the votes so no one gets a majority. They would all timeout again, and the cycle repeats. This is a **livelock** — the system is active but making no progress.

Randomized timeouts ensure that (with high probability) one node times out before the others. It starts an election and collects votes before the other nodes even begin their elections. The Raft paper recommends 150-300ms for election timeouts, which provides a wide enough spread to avoid collisions while keeping failover time under a second.

</details>

---

## Exercise 2: Starting an Election

**Goal:** Implement the election start logic. When a follower's election timer expires, it transitions to candidate and sends RequestVote RPCs to all peers.

### Step 1: The election algorithm

When the election timer fires, a node does the following:

1. Increment its current term
2. Transition to candidate state
3. Vote for itself
4. Reset the election timer
5. Send RequestVote RPCs to all peers

```rust,ignore
impl RaftNode {
    /// Start a new election. Called when:
    /// - A follower's election timer expires (no heartbeat from leader)
    /// - A candidate's election timer expires (split vote, try again)
    pub fn start_election(&mut self) -> Vec<(NodeId, RaftMessage)> {
        // Step 1: Increment term
        self.current_term += 1;

        // Step 2: Transition to candidate
        self.state = NodeState::Candidate;

        // Step 3: Vote for self
        self.voted_for = Some(self.id);
        self.votes_received.clear();
        self.votes_received.insert(self.id);

        // Step 4: Reset election timer
        self.reset_election_timer();

        // Step 5: Clear leader (we are in an election)
        self.leader_id = None;

        println!(
            "[Node {}] Starting election for term {}",
            self.id, self.current_term
        );

        // Step 6: Send RequestVote to all peers
        let (last_log_index, last_log_term) = self.last_log_info();
        let message = RaftMessage::RequestVote {
            term: self.current_term,
            candidate_id: self.id,
            last_log_index,
            last_log_term,
        };

        self.peers
            .iter()
            .map(|&peer_id| (peer_id, message.clone()))
            .collect()
    }

    /// Get the index and term of the last log entry.
    /// Returns (0, 0) if the log is empty.
    fn last_log_info(&self) -> (u64, Term) {
        // We will add a log field in Chapter 15.
        // For now, return empty log info.
        (0, 0)
    }
}
```

The method returns a `Vec<(NodeId, RaftMessage)>` — a list of messages to send. The node does not send messages directly; it produces messages that the caller (the network layer) delivers. This separation of concerns makes the node logic testable without real networking.

### Step 2: Handle the election timeout

The main event loop checks whether the election timer has expired:

```rust,ignore
impl RaftNode {
    /// Called periodically (e.g., every 10ms) to check for timeouts.
    /// Returns any messages that need to be sent.
    pub fn tick(&mut self) -> Vec<(NodeId, RaftMessage)> {
        match self.state {
            NodeState::Follower | NodeState::Candidate => {
                if self.election_timeout_elapsed() {
                    return self.start_election();
                }
            }
            NodeState::Leader => {
                // Leaders do not have election timeouts.
                // They send periodic heartbeats (covered in Chapter 15).
            }
        }
        Vec::new()
    }
}
```

### Step 3: Handle incoming RequestVote

When a node receives a RequestVote, it must decide whether to grant its vote:

```rust,ignore
impl RaftNode {
    /// Handle an incoming message. Returns any response messages.
    pub fn handle_message(
        &mut self,
        from: NodeId,
        message: RaftMessage,
    ) -> Vec<(NodeId, RaftMessage)> {
        // Rule: if any message has a higher term, update and step down
        let msg_term = match &message {
            RaftMessage::RequestVote { term, .. } => *term,
            RaftMessage::RequestVoteResponse { term, .. } => *term,
            RaftMessage::AppendEntries { term, .. } => *term,
            RaftMessage::AppendEntriesResponse { term, .. } => *term,
        };

        if msg_term > self.current_term {
            self.current_term = msg_term;
            self.state = NodeState::Follower;
            self.voted_for = None;
            self.leader_id = None;
            println!(
                "[Node {}] Discovered higher term {}, stepping down to follower",
                self.id, msg_term
            );
        }

        match message {
            RaftMessage::RequestVote {
                term,
                candidate_id,
                last_log_index,
                last_log_term,
            } => self.handle_request_vote(from, term, candidate_id, last_log_index, last_log_term),

            RaftMessage::RequestVoteResponse { term, vote_granted } => {
                self.handle_vote_response(from, term, vote_granted)
            }

            RaftMessage::AppendEntries { term, leader_id, .. } => {
                self.handle_append_entries(from, term, leader_id)
            }

            _ => Vec::new(), // other messages handled in Chapter 15
        }
    }
}
```

The first thing every message handler does: check the term. This is Raft's consistency mechanism. Terms are a logical clock — if a node sees a higher term, its information is stale, and it must step down. This ensures that at most one leader exists per term.

### Step 4: Implement vote granting

```rust,ignore
impl RaftNode {
    /// Handle a RequestVote RPC.
    /// Grant the vote if:
    /// 1. The candidate's term is at least as large as ours
    /// 2. We have not voted for anyone else in this term
    /// 3. The candidate's log is at least as up-to-date as ours
    fn handle_request_vote(
        &mut self,
        from: NodeId,
        term: Term,
        candidate_id: NodeId,
        last_log_index: u64,
        last_log_term: Term,
    ) -> Vec<(NodeId, RaftMessage)> {
        let mut vote_granted = false;

        if term < self.current_term {
            // Candidate's term is behind ours — reject
            println!(
                "[Node {}] Rejecting vote for {} (term {} < {})",
                self.id, candidate_id, term, self.current_term
            );
        } else if self.voted_for.is_none() || self.voted_for == Some(candidate_id) {
            // We have not voted, or we already voted for this candidate
            // (the second case handles retransmissions)

            // Check log completeness: the candidate's log must be
            // at least as up-to-date as ours.
            let (my_last_index, my_last_term) = self.last_log_info();
            let candidate_log_ok = last_log_term > my_last_term
                || (last_log_term == my_last_term && last_log_index >= my_last_index);

            if candidate_log_ok {
                vote_granted = true;
                self.voted_for = Some(candidate_id);
                self.reset_election_timer(); // reset timer when granting a vote
                println!(
                    "[Node {}] Granting vote to {} for term {}",
                    self.id, candidate_id, term
                );
            } else {
                println!(
                    "[Node {}] Rejecting vote for {} (log not up-to-date)",
                    self.id, candidate_id
                );
            }
        } else {
            // Already voted for someone else in this term
            println!(
                "[Node {}] Rejecting vote for {} (already voted for {:?})",
                self.id, candidate_id, self.voted_for
            );
        }

        vec![(
            from,
            RaftMessage::RequestVoteResponse {
                term: self.current_term,
                vote_granted,
            },
        )]
    }
}
```

The vote granting rules are precise:
1. **Term check**: reject if the candidate's term is behind ours. A node from the past should not become leader.
2. **Vote uniqueness**: each node votes for at most one candidate per term. This ensures that at most one candidate can receive a majority.
3. **Log completeness**: the candidate's log must be at least as up-to-date as the voter's log. This ensures the elected leader has all committed entries. "Up-to-date" means: higher last term wins; if terms are equal, longer log wins.

### Step 5: Understand the election safety guarantee

The combination of these rules guarantees **election safety**: at most one leader is elected per term. The proof is by contradiction:

Suppose two nodes A and B both become leader in term T. Each received votes from a majority. But any two majorities of a 2F+1 cluster overlap by at least one node. That overlapping node voted for both A and B — but rule 2 says a node votes for at most one candidate per term. Contradiction.

This is the fundamental invariant of Raft. Everything else — log replication, commitment, state machine safety — depends on this guarantee.

<details>
<summary>Hint: Why reset the election timer when granting a vote?</summary>

If a follower grants a vote to a candidate, it should not immediately start its own election — the candidate might win and start sending heartbeats shortly. Resetting the timer gives the candidate time to collect votes and assume leadership before other nodes start competing.

Without this reset, a follower might grant a vote and then immediately timeout and start its own election, competing with the candidate it just voted for. This would cause unnecessary election conflicts and delay convergence to a stable leader.

</details>

---

## Exercise 3: Winning the Election

**Goal:** Handle vote responses and transition to leader when a majority of votes is received.

### Step 1: Count votes

```rust,ignore
impl RaftNode {
    /// Handle a RequestVoteResponse.
    fn handle_vote_response(
        &mut self,
        from: NodeId,
        term: Term,
        vote_granted: bool,
    ) -> Vec<(NodeId, RaftMessage)> {
        // Ignore if we are no longer a candidate
        if self.state != NodeState::Candidate {
            return Vec::new();
        }

        // Ignore if the response is from a different term
        if term != self.current_term {
            return Vec::new();
        }

        if vote_granted {
            self.votes_received.insert(from);
            println!(
                "[Node {}] Received vote from {} ({}/{} needed)",
                self.id,
                from,
                self.votes_received.len(),
                self.quorum_size()
            );

            // Check if we have a majority
            if self.has_quorum() {
                self.become_leader();
            }
        } else {
            println!(
                "[Node {}] Vote rejected by {}",
                self.id, from
            );
        }

        Vec::new()
    }

    /// Transition to leader state.
    fn become_leader(&mut self) {
        println!(
            "[Node {}] Won election for term {} with {} votes",
            self.id, self.current_term, self.votes_received.len()
        );

        self.state = NodeState::Leader;
        self.leader_id = Some(self.id);

        // Leader-specific initialization will happen in Chapter 15:
        // - Set nextIndex for each peer to last log index + 1
        // - Set matchIndex for each peer to 0
        // - Send initial empty AppendEntries (heartbeat) to all peers
    }
}
```

### Step 2: Handle AppendEntries (heartbeat)

Even though full log replication is Chapter 15, leaders must send heartbeats to prevent followers from starting elections. And followers must handle them to reset their election timers:

```rust,ignore
impl RaftNode {
    /// Handle an AppendEntries RPC.
    /// For now, this only handles the heartbeat aspect.
    /// Full log replication is implemented in Chapter 15.
    fn handle_append_entries(
        &mut self,
        from: NodeId,
        term: Term,
        leader_id: NodeId,
    ) -> Vec<(NodeId, RaftMessage)> {
        if term < self.current_term {
            // Reject: stale leader
            return vec![(
                from,
                RaftMessage::AppendEntriesResponse {
                    term: self.current_term,
                    success: false,
                    match_index: 0,
                },
            )];
        }

        // Valid heartbeat from the leader
        self.leader_id = Some(leader_id);
        self.reset_election_timer();

        // If we were a candidate, step down
        if self.state == NodeState::Candidate {
            self.state = NodeState::Follower;
            println!(
                "[Node {}] Stepping down from candidate — {} is leader for term {}",
                self.id, leader_id, term
            );
        }

        vec![(
            from,
            RaftMessage::AppendEntriesResponse {
                term: self.current_term,
                success: true,
                match_index: 0, // will be properly set in Chapter 15
            },
        )]
    }

    /// Generate heartbeat messages (called periodically by the leader).
    pub fn send_heartbeats(&self) -> Vec<(NodeId, RaftMessage)> {
        if self.state != NodeState::Leader {
            return Vec::new();
        }

        let message = RaftMessage::AppendEntries {
            term: self.current_term,
            leader_id: self.id,
            prev_log_index: 0,
            prev_log_term: 0,
            entries: Vec::new(), // empty = heartbeat
            leader_commit: 0,    // will be set properly in Chapter 15
        };

        self.peers
            .iter()
            .map(|&peer_id| (peer_id, message.clone()))
            .collect()
    }
}
```

### Step 3: The complete election flow

Let us trace through a complete election in a 3-node cluster:

```
Time 0ms:  All nodes start as followers (term 0)
           Node 1: timeout=200ms, Node 2: timeout=250ms, Node 3: timeout=180ms

Time 180ms: Node 3 election timer fires
            Node 3 -> Candidate (term 1)
            Node 3 votes for itself (1/2 votes needed)
            Node 3 sends RequestVote to Node 1, Node 2

Time 181ms: Node 1 receives RequestVote from Node 3 (term 1)
            Node 1 has not voted in term 1
            Node 3's log is ok (empty == empty)
            Node 1 grants vote to Node 3
            Node 1 resets election timer

Time 182ms: Node 3 receives VoteGranted from Node 1
            Node 3 now has 2 votes (self + Node 1) = majority!
            Node 3 -> Leader (term 1)
            Node 3 sends heartbeats to Node 1, Node 2

Time 183ms: Node 2 receives RequestVote from Node 3 (term 1)
            Node 2 has not voted in term 1
            Node 2 grants vote to Node 3 (but election is already won)

Time 184ms: Node 1 receives heartbeat from Node 3
            Node 1 resets election timer
            Node 2 receives heartbeat from Node 3
            Node 2 resets election timer

... Node 3 continues sending heartbeats every ~50ms ...
... Nodes 1 and 2 keep resetting their election timers ...
```

### Step 4: Test with a deterministic simulation

Real networking is non-deterministic — messages can arrive in any order, at any time. For testing, we use a deterministic simulation:

```rust,ignore
/// A simulated network for testing Raft.
struct SimulatedNetwork {
    nodes: HashMap<NodeId, RaftNode>,
    /// Messages in transit: (from, to, message)
    in_flight: Vec<(NodeId, NodeId, RaftMessage)>,
}

impl SimulatedNetwork {
    fn new(node_count: usize) -> Self {
        let all_ids: Vec<NodeId> = (1..=node_count as u64).collect();
        let mut nodes = HashMap::new();

        for &id in &all_ids {
            let peers: Vec<NodeId> = all_ids.iter().copied().filter(|&p| p != id).collect();
            nodes.insert(id, RaftNode::new(id, peers));
        }

        SimulatedNetwork {
            nodes,
            in_flight: Vec::new(),
        }
    }

    /// Tick all nodes and collect outgoing messages.
    fn tick_all(&mut self) {
        let mut new_messages = Vec::new();

        for (&id, node) in &mut self.nodes {
            let messages = node.tick();
            for (to, msg) in messages {
                new_messages.push((id, to, msg));
            }
        }

        self.in_flight.extend(new_messages);
    }

    /// Deliver all in-flight messages.
    fn deliver_all(&mut self) {
        let messages: Vec<_> = self.in_flight.drain(..).collect();

        for (from, to, message) in messages {
            if let Some(node) = self.nodes.get_mut(&to) {
                let responses = node.handle_message(from, message);
                for (resp_to, resp_msg) in responses {
                    self.in_flight.push((to, resp_to, resp_msg));
                }
            }
        }
    }

    /// Find the current leader (if any).
    fn find_leader(&self) -> Option<NodeId> {
        self.nodes
            .iter()
            .find(|(_, node)| node.state == NodeState::Leader)
            .map(|(&id, _)| id)
    }

    /// Force a specific node's election timer to expire.
    fn expire_election_timer(&mut self, id: NodeId) {
        if let Some(node) = self.nodes.get_mut(&id) {
            node.election_deadline = Instant::now() - Duration::from_secs(1);
        }
    }
}
```

### Step 5: Run the test

```rust,ignore
#[test]
fn test_leader_election() {
    let mut network = SimulatedNetwork::new(3);

    // Initially: no leader, all followers
    assert!(network.find_leader().is_none());
    for (_, node) in &network.nodes {
        assert_eq!(node.state, NodeState::Follower);
    }

    // Force node 1's election timer to expire
    network.expire_election_timer(1);
    network.tick_all();

    // Node 1 should be a candidate
    assert_eq!(network.nodes[&1].state, NodeState::Candidate);
    assert_eq!(network.nodes[&1].current_term, 1);

    // Deliver RequestVote messages
    network.deliver_all();

    // Nodes 2 and 3 should have voted for node 1
    assert_eq!(network.nodes[&2].voted_for, Some(1));
    assert_eq!(network.nodes[&3].voted_for, Some(1));

    // Deliver vote responses
    network.deliver_all();

    // Node 1 should now be the leader
    assert_eq!(network.nodes[&1].state, NodeState::Leader);
    assert_eq!(network.find_leader(), Some(1));
    assert_eq!(network.nodes[&1].current_term, 1);
}

#[test]
fn test_split_vote_new_election() {
    let mut network = SimulatedNetwork::new(3);

    // Force nodes 1 and 2 to start elections simultaneously
    network.expire_election_timer(1);
    network.expire_election_timer(2);
    network.tick_all();

    // Both are candidates in term 1
    assert_eq!(network.nodes[&1].state, NodeState::Candidate);
    assert_eq!(network.nodes[&2].state, NodeState::Candidate);
    assert_eq!(network.nodes[&1].current_term, 1);
    assert_eq!(network.nodes[&2].current_term, 1);

    // Deliver messages — node 3 can only vote for one of them
    network.deliver_all();

    // Node 3 voted for whoever's RequestVote arrived first
    // (in our simulation, message order depends on HashMap iteration)
    let node3_vote = network.nodes[&3].voted_for;
    assert!(node3_vote == Some(1) || node3_vote == Some(2));

    // Deliver vote responses
    network.deliver_all();

    // Whoever got node 3's vote wins (2 out of 3 = majority)
    let leader = network.find_leader();
    assert!(leader.is_some());
}
```

### Step 6: Understand what you built

The test harness is deterministic: we control when timers expire and when messages are delivered. This lets us test specific scenarios (split votes, network partitions, term conflicts) that would be hard to trigger with real networking and timing.

The separation between the `RaftNode` (pure logic) and the `SimulatedNetwork` (message delivery) is intentional. In production, you would replace `SimulatedNetwork` with real TCP/async networking, but the `RaftNode` logic would be identical. This is the same information-hiding principle from Chapter 12 — the Raft algorithm does not know or care how messages are delivered.

---

## Exercise 4: Term Management and Step-Down

**Goal:** Implement correct term management — the mechanism that ensures stale leaders step down and the cluster converges to a single leader.

### Step 1: The term rule

The most important rule in Raft: **if a node receives a message with a higher term, it immediately updates its term and steps down to follower**. This applies to every message type, in every state.

We already implemented this in `handle_message`. Let us test it:

```rust,ignore
#[test]
fn test_leader_steps_down_on_higher_term() {
    let mut network = SimulatedNetwork::new(3);

    // Elect node 1 as leader in term 1
    network.expire_election_timer(1);
    network.tick_all();
    network.deliver_all();
    network.deliver_all();
    assert_eq!(network.nodes[&1].state, NodeState::Leader);

    // Simulate: node 2 was partitioned, had its own election,
    // and is now in term 5 (even though it did not win)
    network.nodes.get_mut(&2).unwrap().current_term = 5;
    network.nodes.get_mut(&2).unwrap().state = NodeState::Follower;

    // Node 1 (leader, term 1) sends a heartbeat to node 2 (follower, term 5)
    let heartbeats = network.nodes[&1].send_heartbeats();
    for (to, msg) in heartbeats {
        if to == 2 {
            let responses = network.nodes.get_mut(&2).unwrap()
                .handle_message(1, msg);

            // Node 2 should reject (term 1 < 5) and respond with term 5
            for (resp_to, resp_msg) in responses {
                if let RaftMessage::AppendEntriesResponse { term, success, .. } = &resp_msg {
                    assert_eq!(*term, 5);
                    assert!(!success);
                }

                // Deliver the response to node 1
                let _ = network.nodes.get_mut(&resp_to).unwrap()
                    .handle_message(2, resp_msg);
            }
        }
    }

    // Node 1 should have stepped down to follower
    assert_eq!(network.nodes[&1].state, NodeState::Follower);
    assert_eq!(network.nodes[&1].current_term, 5);
}
```

### Step 2: Handle network partitions

Network partitions are the hardest scenario for consensus. A partition splits the cluster into groups that cannot communicate:

```
Before partition:
  Node 1 (Leader) ←→ Node 2 ←→ Node 3

During partition:
  [Node 1]  |  [Node 2, Node 3]
  Leader    |  Followers
  term 1    |  term 1
  (cannot   |  (Node 2 times out,
   reach    |   starts election,
   anyone)  |   wins in term 2)
            |
  [Node 1]  |  [Node 2 (Leader), Node 3]
  Leader    |  term 2
  term 1    |

After partition heals:
  Node 1 sends heartbeat (term 1) to Node 2
  Node 2 responds with term 2
  Node 1 sees term 2 > 1, steps down to follower
  Cluster converges: Node 2 is leader, term 2
```

The term mechanism ensures safety through partitions: the old leader (Node 1) steps down when it discovers a higher term, even though it was happily serving as leader before the partition healed.

### Step 3: Test the partition scenario

```rust,ignore
#[test]
fn test_network_partition_and_recovery() {
    let mut network = SimulatedNetwork::new(3);

    // Elect node 1 as leader in term 1
    network.expire_election_timer(1);
    network.tick_all();
    network.deliver_all();
    network.deliver_all();
    assert_eq!(network.nodes[&1].state, NodeState::Leader);
    assert_eq!(network.nodes[&1].current_term, 1);

    // Simulate partition: node 1 is isolated
    // Node 2 and 3 can communicate; node 1 cannot reach anyone

    // Node 2 times out and starts election (term 2)
    network.expire_election_timer(2);

    // Only tick node 2
    let messages = network.nodes.get_mut(&2).unwrap().tick();

    // Only deliver messages between nodes 2 and 3 (partition!)
    for (to, msg) in messages {
        if to == 3 {
            let responses = network.nodes.get_mut(&3).unwrap()
                .handle_message(2, msg);
            for (resp_to, resp_msg) in responses {
                if resp_to == 2 {
                    let _ = network.nodes.get_mut(&2).unwrap()
                        .handle_message(3, resp_msg);
                }
            }
        }
        // Messages to node 1 are dropped (partition)
    }

    // Node 2 should be leader in term 2
    assert_eq!(network.nodes[&2].state, NodeState::Leader);
    assert_eq!(network.nodes[&2].current_term, 2);

    // Node 1 still thinks it is leader in term 1
    assert_eq!(network.nodes[&1].state, NodeState::Leader);
    assert_eq!(network.nodes[&1].current_term, 1);

    // Partition heals: node 2 sends heartbeat to node 1
    let heartbeats = network.nodes[&1].send_heartbeats();
    // Node 1's heartbeats have term 1, which is less than node 2's term 2

    // Node 2 sends heartbeat to node 1
    let heartbeats = network.nodes[&2].send_heartbeats();
    for (to, msg) in heartbeats {
        if to == 1 {
            let _ = network.nodes.get_mut(&1).unwrap()
                .handle_message(2, msg);
        }
    }

    // Node 1 should have stepped down
    assert_eq!(network.nodes[&1].state, NodeState::Follower);
    assert_eq!(network.nodes[&1].current_term, 2);
    assert_eq!(network.nodes[&1].leader_id, Some(2));
}
```

### Step 4: Pre-vote protocol (extension)

The standard Raft election has a problem: a node that is partitioned from the cluster keeps incrementing its term and starting elections. When the partition heals, it sends RequestVote messages with a very high term, causing the current leader to step down — even though the cluster was healthy.

The **pre-vote protocol** fixes this. Before starting a real election, a candidate sends PreVote messages. These do not increment the term. If the candidate does not receive enough pre-votes, it does not start a real election. This prevents partitioned nodes from disrupting the cluster.

```rust,ignore
/// Extended message type with pre-vote support.
#[derive(Debug, Clone)]
pub enum RaftMessage {
    // ... existing variants ...

    /// Pre-vote: "Would you vote for me if I started an election?"
    /// Does not increment term.
    PreVote {
        term: Term,          // the term the candidate WOULD use
        candidate_id: NodeId,
        last_log_index: u64,
        last_log_term: Term,
    },
    PreVoteResponse {
        term: Term,
        vote_granted: bool,
    },
}
```

The pre-vote flow:
1. Follower's election timer expires
2. Send PreVote to all peers (using `current_term + 1`, but do NOT increment `current_term`)
3. If majority responds positively, start a real election (now increment `current_term`)
4. If not, go back to waiting as a follower

This is implemented in TiKV, etcd, and other production Raft implementations. It is not part of the original Raft paper but is described in Diego Ongaro's PhD dissertation.

### Step 5: Understanding election safety in depth

Let us formalize the safety guarantee with an invariant that must hold at all times:

```
INVARIANT: For any given term T, at most one node can be elected leader.

PROOF:
  - A leader needs votes from a majority (> N/2 nodes)
  - Each node votes for at most one candidate per term
  - Any two majorities of N nodes overlap by at least one node
  - That overlapping node voted for both candidates
  - But a node can only vote for one candidate per term
  - CONTRADICTION: two leaders in the same term is impossible
```

This invariant holds even with network partitions, message delays, node crashes, and restarts — as long as voted_for is persisted to stable storage before responding (covered in Chapter 16).

> **Coming from JS/Python/Go?**
>
> State machines are implemented differently across languages:
>
> | Concept | JavaScript | Python | Go | Rust |
> |---------|-----------|--------|-----|------|
> | States | String constants | Enum class | iota constants | `enum` variants |
> | State data | Properties on object | Attributes | Fields on struct | Data in variants |
> | Transition | switch/if-else | match (3.10+) | switch | `match` (exhaustive) |
> | Completeness | None | None | None | Compile-time check |
> | Testing | Jest + mocks | pytest + mocks | go test | cargo test (no mocks needed) |
>
> Rust's advantage for consensus algorithms: the compiler prevents you from forgetting to handle a state. In Go's etcd (a Raft implementation), state transitions are large switch statements where a missing case is a silent bug. In Rust, a missing match arm is a compile error. For safety-critical code like consensus, this is significant.

---

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

## DSA in Context: Leader Election as a State Machine

Raft's leader election is a distributed state machine — each node runs the same state machine independently, and the combination of their states determines the cluster's behavior.

### State machine formalization

```
States:  S = {Follower, Candidate, Leader}
Events:  E = {ElectionTimeout, VoteGranted, VoteDenied,
              MajorityReached, HigherTermSeen, AppendEntriesReceived}
Transitions:
  (Follower,  ElectionTimeout)       → Candidate
  (Candidate, MajorityReached)       → Leader
  (Candidate, ElectionTimeout)       → Candidate  (new term)
  (Candidate, HigherTermSeen)        → Follower
  (Candidate, AppendEntriesReceived) → Follower
  (Leader,    HigherTermSeen)        → Follower
```

### Why not a simpler algorithm?

You might wonder: why not just pick the node with the lowest ID as leader? Or use a timestamp — the node that started first is leader?

**Lowest ID**: requires all nodes to know all other nodes' IDs, and requires reliable failure detection. "Is node 1 dead, or just slow?" is undecidable in an asynchronous system (the FLP impossibility result).

**Timestamps**: clocks skew. Node A thinks it is 10:00:00, node B thinks it is 10:00:03. There is no global clock in a distributed system. Google's Spanner uses GPS-synchronized atomic clocks to approximate this, but that requires specialized hardware.

**Raft's approach**: terms (logical clocks) + randomized timeouts + majority voting. No physical clocks, no global coordinator, no special hardware. The algorithm is correct as long as messages are eventually delivered (partial synchrony assumption).

### The FLP impossibility

In 1985, Fischer, Lynch, and Paterson proved that no deterministic consensus algorithm can guarantee termination in an asynchronous system where even one process can fail. This means consensus algorithms must make assumptions:

- **Paxos/Raft**: assume partial synchrony (messages are eventually delivered within some unknown time bound). Use timeouts and retries to ensure progress.
- **Practical BFT**: assume bounded synchrony and fewer than 1/3 Byzantine (malicious) failures.

Raft uses randomized timeouts to satisfy the termination requirement probabilistically — the probability of repeated split votes decreases exponentially with each round.

### Comparison with other election algorithms

| Algorithm | Nodes needed | Failures tolerated | Complexity | Used by |
|-----------|-------------|-------------------|------------|---------|
| Raft | 2F+1 | F crash failures | Simple | etcd, TiKV, CockroachDB |
| Paxos | 2F+1 | F crash failures | Complex | Chubby, Megastore |
| ZAB | 2F+1 | F crash failures | Moderate | ZooKeeper |
| Bully | Any | Crash failures | Very simple | Small systems |
| Ring | Any | Crash failures | Simple | Token-ring networks |

Raft and Paxos provide the same safety guarantees — they differ in understandability and implementation complexity. The Raft paper was explicitly designed to be easier to teach and implement than Paxos, which is why we use it here.

---

## System Design Corner: Distributed Consensus

Consensus is at the heart of every distributed database. Understanding it deeply distinguishes a senior engineer from a junior one.

### Why consensus matters for databases

Without consensus, a replicated database faces the **split-brain problem**:

```
Before partition:
  Primary ←→ Replica
  Both agree: balance = $100

During partition:
  [Primary]  |  [Replica]
  Client A:  |  Client B:
  balance -  |  balance -
  $30        |  $50
  = $70      |  = $50

After partition:
  Primary says: $70
  Replica says: $50
  Which is correct? Both? Neither?
```

Consensus algorithms prevent this by ensuring that only one node can accept writes at a time (the leader), and writes are only committed after a majority acknowledges them. Even during a partition, at most one side can form a majority and continue operating.

### Raft in production: etcd

etcd is a distributed key-value store used by Kubernetes for cluster coordination. It uses Raft for consensus:

```
Kubernetes                 etcd cluster
  │                      ┌──────────────────┐
  │── PUT /nodes/n1 ────►│ Leader (Node 1)  │
  │                      │   │              │
  │                      │   ├── replicate ──►  Node 2
  │                      │   └── replicate ──►  Node 3
  │                      │                  │
  │                      │   majority ack   │
  │                      │   commit entry   │
  │◄── OK ──────────────│                  │
  │                      └──────────────────┘
```

Every Kubernetes pod, service, and config change goes through etcd's Raft consensus. If the etcd leader dies, a new leader is elected within 1-2 seconds (election timeout + vote collection + heartbeat). Kubernetes barely notices.

### Leader lease optimization

Standard Raft requires the leader to send heartbeats to maintain leadership. Some implementations add a **leader lease** — the leader is guaranteed to remain leader for a certain duration after each heartbeat acknowledgment:

```
Leader sends heartbeat at T=0
Followers acknowledge at T=1ms
Leader lease: valid until T=150ms (election timeout)

During lease:
  - Leader can serve reads without contacting followers
  - Followers will not start elections (their timers are reset)
  - Reads are linearizable because no other leader can exist
```

This reduces read latency from "one round-trip to majority" to "local read." TiKV and CockroachDB use leader leases. The tradeoff: lease correctness depends on bounded clock skew, which is a weaker assumption than Raft normally requires.

### Multi-Raft: scaling beyond one group

A single Raft group has a single leader that handles all writes. This limits throughput to what one node can handle. Multi-Raft partitions data across many Raft groups, each with its own leader:

```
Data: keys A-Z

Raft Group 1 (keys A-H):  Leader=Node1, Replicas=Node2,Node3
Raft Group 2 (keys I-P):  Leader=Node2, Replicas=Node1,Node3
Raft Group 3 (keys Q-Z):  Leader=Node3, Replicas=Node1,Node2
```

Now writes to different key ranges go to different leaders, spreading the load. TiKV uses this approach — each ~96MB data region is a separate Raft group. CockroachDB uses a similar approach with ranges.

> **Interview talking point:** *"Our database uses Raft for consensus with a 3-node or 5-node cluster. Leader election uses randomized timeouts (150-300ms) to avoid split votes, and the election safety property guarantees at most one leader per term through majority voting with the constraint that each node votes at most once per term. For production, I would add the pre-vote protocol to prevent disruptive re-elections from partitioned nodes, leader leases for fast local reads, and Multi-Raft to distribute write throughput across multiple Raft groups. The key correctness invariant is that voted_for and the log must be persisted to stable storage before sending any response — without this, a node restart could violate the election safety guarantee."*

---

## Design Insight: Define Errors Out of Existence

In *A Philosophy of Software Design*, Ousterhout advocates designing systems so that error conditions simply cannot occur, rather than detecting and handling them:

> *"The best way to deal with exception handling complexity is to define your APIs so that there are no exceptions to handle."*

Raft is a masterclass in this principle. Its predecessor, Paxos, is notoriously difficult to understand and implement. Lamport's original Paxos paper required multiple rounds of discussion before the research community understood it. Implementations frequently had subtle bugs.

Raft "defines errors out of existence" through simplification:

**1. Single leader instead of multi-proposer.** Paxos allows any node to propose values, which creates complex conflict resolution. Raft restricts proposals to a single leader. This eliminates an entire class of conflicts: if only one node can write, writes cannot conflict.

**2. Sequential terms instead of concurrent ballots.** Paxos ballots can overlap in complex ways. Raft terms are strictly sequential — each term has at most one leader, and a higher term always supersedes a lower one. This makes reasoning about correctness much simpler.

**3. Log entries are immutable once committed.** A committed entry will never be overwritten or removed. This eliminates the need for complex conflict resolution in the log.

**4. Leader completeness property.** The log completeness check in RequestVote ensures that any elected leader has all committed entries. This eliminates the need for a "log catch-up" protocol for new leaders — the leader already has everything.

Each simplification removes a category of error conditions. The result is an algorithm that is provably equivalent to Paxos in safety and liveness, but dramatically simpler to understand and implement correctly. The Raft paper's user study showed that students scored significantly higher on Raft questions than Paxos questions, even with the same amount of study time.

The lesson for software design: before writing error handling code, ask whether you can change the design so the error cannot occur. The best error handler is the one you never need to write.

> *"The best way to deal with exception handling complexity is to define your APIs so that there are no exceptions to handle."*
> — John Ousterhout

---

## What You Built

In this chapter, you:

1. **Modeled Raft states** — `NodeState` enum with `Follower`, `Candidate`, `Leader` variants, using Rust's exhaustive pattern matching for safety
2. **Built the RaftNode struct** — term management, vote tracking, election timeouts with randomized jitter, peer list, quorum calculation
3. **Implemented leader election** — `start_election()` for term increment and vote solicitation, `handle_request_vote()` with vote granting rules, `handle_vote_response()` with majority detection
4. **Handled term management** — higher-term step-down in every message handler, ensuring at most one leader per term
5. **Built a test harness** — `SimulatedNetwork` for deterministic testing of election scenarios, including partition recovery and split votes

Your database can now elect a leader among multiple nodes. In Chapter 15, the leader will replicate data to followers using AppendEntries RPCs, implementing the second half of Raft — log replication. Together, leader election and log replication make your database fault-tolerant: if the leader crashes, a new leader is elected and takes over with all committed data intact.

---

### DS Deep Dive

Raft's leader election is one of many approaches to distributed agreement. This deep dive compares Raft with Paxos (the academic gold standard), ZAB (used by ZooKeeper), Viewstamped Replication, and PBFT (for Byzantine faults). We trace the intellectual lineage from Lamport's original Paxos paper through Raft's deliberate simplification, and explore why "understandability" is a valid design goal for consensus algorithms.

**-> [Consensus Algorithms — "The Council Chamber"](../ds-narratives/ch14-consensus-algorithms.md)**

---

### Reference implementation

The files you built in this chapter correspond to these files in the reference codebase:

| Your file | Reference |
|-----------|-----------|
| `src/raft.rs` — RaftNode, NodeState, messages | [`src/raft/node.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/raft/node.rs) — `Node` struct and state machine |
| `RaftMessage` — RequestVote, AppendEntries | [`src/raft/message.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/raft/message.rs) — RPC message types |
| `start_election()`, `handle_request_vote()` | [`src/raft/node.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/raft/node.rs) — `campaign()`, `vote()` |
| `SimulatedNetwork` — test harness | [`tests/`](https://github.com/erikgrinaker/toydb/tree/master/tests) — cluster test harness |
