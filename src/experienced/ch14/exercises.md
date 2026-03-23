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
