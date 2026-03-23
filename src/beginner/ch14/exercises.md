## Exercise 1: The RaftNode Struct

**Goal:** Define the core data structures for a Raft node -- the state enum, the node struct, and the message types.

### Step 1: Define the node state

Create a new file `src/raft.rs`:

```rust,ignore
// src/raft.rs

use std::collections::HashSet;
use std::time::{Duration, Instant};

/// A unique identifier for a node in the cluster.
/// Each node gets a number: 1, 2, 3, etc.
pub type NodeId = u64;

/// A Raft term number. Think of it like an election year.
/// Term 1 is the first election, term 2 is the second, and so on.
/// Terms only go up -- they never decrease.
pub type Term = u64;

/// The three possible states of a Raft node.
#[derive(Debug, Clone, PartialEq)]
pub enum NodeState {
    /// Passive: listens for messages, does not initiate anything.
    /// Becomes a Candidate if the election timer fires.
    Follower,
    /// Actively seeking votes. Becomes Leader if it wins,
    /// or Follower if it discovers a higher term.
    Candidate,
    /// Runs the cluster: sends heartbeats, handles writes.
    /// Steps down to Follower if it discovers a higher term.
    Leader,
}
```

> **Programming Concept: Type Aliases**
>
> `pub type NodeId = u64;` creates a **type alias**. `NodeId` is just another name for `u64`. The compiler treats them as identical types. So why bother?
>
> Readability. When you see `fn handle_vote(from: NodeId, term: Term)`, you immediately know what each parameter means. If it were `fn handle_vote(from: u64, term: u64)`, you might mix them up. Type aliases are like labels -- they do not add safety, but they make the code much easier to read.

### Step 2: Define the RaftNode struct

```rust,ignore
/// A Raft node -- one server in the cluster.
pub struct RaftNode {
    /// This node's unique ID.
    pub id: NodeId,

    /// IDs of all other nodes in the cluster.
    /// A 5-node cluster with id=1 would have peers: [2, 3, 4, 5].
    pub peers: Vec<NodeId>,

    /// Current state: Follower, Candidate, or Leader.
    pub state: NodeState,

    /// The latest term this node has seen.
    /// Monotonically increasing -- it only goes up.
    pub current_term: Term,

    /// Who this node voted for in the current term.
    /// None means "I have not voted yet this term."
    /// Each node can vote for at most ONE candidate per term.
    pub voted_for: Option<NodeId>,

    /// Who this node believes the leader is.
    /// None during elections when the leader is unknown.
    pub leader_id: Option<NodeId>,

    /// When the election timer expires.
    /// If we reach this time without hearing from a leader,
    /// we start an election.
    pub election_deadline: Instant,

    /// How long to wait before starting an election.
    /// This is randomized to prevent all nodes from
    /// starting elections at the same time.
    pub election_timeout: Duration,

    /// Which nodes have voted for us in the current election.
    /// Only meaningful when state is Candidate.
    pub votes_received: HashSet<NodeId>,
}
```

That is a lot of fields. Let's understand each one with an analogy.

Imagine Raft as a political election:

- **`id`** -- your name (e.g., "Node 1")
- **`peers`** -- the other politicians you are running against
- **`state`** -- are you a regular citizen (Follower), running for office (Candidate), or the current president (Leader)?
- **`current_term`** -- what election year is it? (Term 1, Term 2, etc.)
- **`voted_for`** -- who did you vote for this year? You can only vote once per election.
- **`leader_id`** -- who is the current president?
- **`election_deadline`** -- when is the next election? If the president stops communicating, citizens can call a new election.
- **`election_timeout`** -- how patient are you? Each citizen waits a slightly different amount of time before calling an election. This prevents everyone from calling an election at the exact same moment.
- **`votes_received`** -- if you are running for office, who has voted for you so far?

> **What Just Happened?**
>
> We defined the complete state of a Raft node. Every piece of information the node needs to participate in elections is stored in this struct. The `HashSet<NodeId>` for `votes_received` ensures we count each voter only once (you cannot vote twice!).

### Step 3: Implement the constructor

```rust,ignore
impl RaftNode {
    /// Create a new Raft node. All nodes start as followers.
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
}
```

Every node starts as a Follower with term 0 and no votes. This makes sense -- when a cluster boots up, no one is the leader yet. Nodes wait for their election timeouts to fire, and then the election process determines who becomes leader.

### Step 4: Random election timeouts

This is one of the most important details in Raft:

```rust,ignore
impl RaftNode {
    /// Generate a random election timeout between 150ms and 300ms.
    fn random_election_timeout() -> Duration {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        use std::time::SystemTime;

        // Simple pseudo-random number based on the current time
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
}
```

Why random? Imagine if all five nodes had the same timeout -- say, 200ms. When the cluster starts, all five would timeout at the same instant, all five would start elections, all five would vote for themselves, and no one would get a majority. They would all timeout again, start new elections, and the cycle would repeat forever. This is called a **livelock** -- the system is active but making no progress.

Random timeouts break the tie. If Node 1 gets 173ms and Node 2 gets 241ms, Node 1 starts its election first, collects votes before Node 2 even starts, and wins. Problem solved.

> **Programming Concept: Hashing for Randomness**
>
> We use `DefaultHasher` to generate a pseudo-random number from the current time. This is not cryptographically secure random, but it is good enough for election timeouts. In a production system, you would use the `rand` crate for proper random number generation. We avoid adding that dependency to keep things simple.

### Step 5: Helper methods

```rust,ignore
impl RaftNode {
    /// Reset the election timer. Called when:
    /// - We receive a heartbeat from the leader (leader is alive, no need for election)
    /// - We grant a vote to a candidate (give them time to win)
    /// - We start a new election (reset our own timer)
    pub fn reset_election_timer(&mut self) {
        self.election_timeout = Self::random_election_timeout();
        self.election_deadline = Instant::now() + self.election_timeout;
    }

    /// Check if the election timer has expired.
    pub fn election_timeout_elapsed(&self) -> bool {
        Instant::now() >= self.election_deadline
    }

    /// How many votes are needed for a majority?
    /// In a 5-node cluster (self + 4 peers), a majority is 3.
    /// In a 3-node cluster (self + 2 peers), a majority is 2.
    pub fn quorum_size(&self) -> usize {
        (self.peers.len() + 1) / 2 + 1
    }

    /// Has this node received enough votes to win?
    pub fn has_quorum(&self) -> bool {
        self.votes_received.len() >= self.quorum_size()
    }
}
```

The `quorum_size` calculation deserves explanation. In a cluster of N nodes, a majority is N/2 + 1 (integer division). For 5 nodes: 5/2 + 1 = 3. For 3 nodes: 3/2 + 1 = 2. This ensures that any two majorities overlap by at least one node -- a critical property for safety.

> **Programming Concept: Why Majorities?**
>
> Raft requires a majority (quorum) to agree before anything is committed. Why? Because any two majorities in the same group must share at least one member. If Group A (3 of 5 nodes) agrees on value X, and Group B (3 of 5 nodes) agrees on value Y, at least one node is in both groups. That node knows about both values and can prevent a conflict. This is the mathematical foundation of consensus.

---

## Exercise 2: Defining Messages

**Goal:** Define the message types that Raft nodes send to each other during elections.

### Step 1: The RequestVote RPC

When a candidate wants to become leader, it sends a `RequestVote` message to every other node. Think of it as a campaign message: "Hi, I am Node 3, running for leader in Term 5. Please vote for me!"

```rust,ignore
/// Messages that Raft nodes send to each other.
#[derive(Debug, Clone)]
pub enum RaftMessage {
    /// "Please vote for me!"
    /// Sent by candidates to all other nodes.
    RequestVote {
        /// The candidate's term (election year).
        term: Term,
        /// Who is asking for the vote.
        candidate_id: NodeId,
        /// How up-to-date the candidate's log is.
        /// (We will use these in Chapter 15.)
        last_log_index: u64,
        last_log_term: Term,
    },

    /// "Yes, you have my vote" or "No, I am not voting for you."
    /// Sent in response to a RequestVote.
    RequestVoteResponse {
        /// The responding node's current term.
        term: Term,
        /// Did the node grant its vote?
        vote_granted: bool,
    },

    /// "I am the leader, here is your data."
    /// Sent by the leader to all followers.
    /// (Fully implemented in Chapter 15.)
    AppendEntries {
        /// The leader's term.
        term: Term,
        /// Who the leader is.
        leader_id: NodeId,
    },

    /// "Got it" or "No, something is wrong."
    /// Sent in response to AppendEntries.
    AppendEntriesResponse {
        /// The responding node's current term.
        term: Term,
        /// Did the operation succeed?
        success: bool,
    },
}
```

### Step 2: Understanding terms (election years)

Terms are Raft's version of time. Every message carries a term number. Here is why they matter:

Imagine Node 1 is the leader in Term 3. A network problem separates Node 1 from the rest of the cluster. Nodes 2-5 cannot hear from Node 1, so they start an election and elect Node 2 as leader in Term 4.

Now Node 1 comes back online. It still thinks it is the leader (of Term 3). But when it sends a message, the other nodes reply with Term 4. Node 1 sees "Term 4 is higher than my Term 3" and immediately steps down to Follower. The term number is how stale leaders learn that they have been replaced.

```
Timeline:
  Term 3: Node 1 is leader
  Term 3: Network partition! Node 1 is isolated.
  Term 4: Nodes 2-5 elect Node 2 as leader
  Term 4: Node 1 reconnects, sees Term 4, steps down
```

The rule is simple: **if you see a term higher than yours, update your term and become a Follower.** This ensures old leaders always step down.

> **What Just Happened?**
>
> We defined the message types for Raft communication using a Rust enum. Each variant carries different data appropriate to its purpose. The `RequestVote` message is the heart of elections -- it is how candidates ask for votes and how other nodes respond.
>
> Terms act as a logical clock. They only increase. Any node that sees a higher term immediately knows its information is outdated and steps down.

---

## Exercise 3: Starting an Election

**Goal:** Implement the election logic. When a Follower's timer fires, it becomes a Candidate and asks for votes.

### Step 1: The election algorithm

When the election timer fires, the node does five things:

1. **Increment its term** -- we are starting a new election, so the term goes up
2. **Become a Candidate** -- change state from Follower to Candidate
3. **Vote for itself** -- every candidate votes for itself first
4. **Reset the election timer** -- so it does not immediately fire again
5. **Ask all peers for their votes** -- send RequestVote messages

```rust,ignore
impl RaftNode {
    /// Start a new election.
    /// Returns a list of messages to send to peers.
    pub fn start_election(&mut self) -> Vec<(NodeId, RaftMessage)> {
        // Step 1: New term
        self.current_term += 1;

        // Step 2: Become a candidate
        self.state = NodeState::Candidate;

        // Step 3: Vote for yourself
        self.voted_for = Some(self.id);
        self.votes_received.clear();
        self.votes_received.insert(self.id);

        // Step 4: Reset the timer
        self.reset_election_timer();

        // Step 5: Clear leader (we are in an election)
        self.leader_id = None;

        println!(
            "[Node {}] Starting election for term {}",
            self.id, self.current_term
        );

        // Build RequestVote messages for each peer
        let message = RaftMessage::RequestVote {
            term: self.current_term,
            candidate_id: self.id,
            last_log_index: 0,  // will be used in Chapter 15
            last_log_term: 0,   // will be used in Chapter 15
        };

        // Return one message per peer
        self.peers
            .iter()
            .map(|&peer_id| (peer_id, message.clone()))
            .collect()
    }
}
```

> **Programming Concept: Why Return Messages Instead of Sending Them?**
>
> Notice that `start_election` does not actually send messages. It returns a `Vec<(NodeId, RaftMessage)>` -- a list of "please send this to that node." The caller (the network layer) is responsible for actual delivery.
>
> This separation of concerns is a powerful design pattern. The Raft logic is pure -- it takes inputs and produces outputs without touching the network. This makes it testable without real networking. You can simulate a cluster entirely in memory.

### Step 2: The tick method

The node needs to be checked periodically (say, every 10ms) to see if the election timer has expired:

```rust,ignore
impl RaftNode {
    /// Called periodically to check for timeouts.
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
                // They send heartbeats instead (Chapter 15).
            }
        }
        Vec::new()  // nothing to do
    }
}
```

Notice the `match` expression handles all three states. Followers and Candidates check the election timer. Leaders do not -- they are the ones sending heartbeats, not waiting for them.

The `|` in `NodeState::Follower | NodeState::Candidate` means "match either of these." It is like saying "if the state is Follower OR Candidate."

### Step 3: Handling incoming RequestVote

When a node receives a RequestVote, it must decide whether to grant its vote. There are three rules:

1. **Term check:** if the candidate's term is lower than mine, reject. A candidate from the past should not become leader.
2. **Vote uniqueness:** I can only vote for ONE candidate per term. If I already voted for someone else, reject.
3. **Log check:** the candidate's log must be at least as up-to-date as mine. (We will implement this check fully in Chapter 15.)

```rust,ignore
impl RaftNode {
    /// Handle an incoming message. Returns any response messages.
    pub fn handle_message(
        &mut self,
        from: NodeId,
        message: RaftMessage,
    ) -> Vec<(NodeId, RaftMessage)> {
        // CRITICAL RULE: if any message has a higher term,
        // update our term and step down to Follower.
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

        // Now handle the specific message type
        match message {
            RaftMessage::RequestVote {
                term,
                candidate_id,
                ..
            } => self.handle_request_vote(from, term, candidate_id),

            RaftMessage::RequestVoteResponse {
                term,
                vote_granted,
            } => self.handle_vote_response(from, term, vote_granted),

            RaftMessage::AppendEntries {
                term,
                leader_id,
            } => self.handle_append_entries(from, term, leader_id),

            _ => Vec::new(),
        }
    }
}
```

> **What Just Happened?**
>
> Every incoming message first triggers the term check. This is the most important rule in Raft: if you see a higher term, you know your information is stale. You immediately update your term and become a Follower. This ensures that old leaders step down and the cluster converges to a single leader.
>
> After the term check, we dispatch to a specific handler based on the message type. This is the `match` expression doing its job -- the compiler ensures we handle every variant of `RaftMessage`.

### Step 4: Vote granting logic

```rust,ignore
impl RaftNode {
    /// Handle a RequestVote message.
    /// Decide whether to grant our vote to the candidate.
    fn handle_request_vote(
        &mut self,
        from: NodeId,
        term: Term,
        candidate_id: NodeId,
    ) -> Vec<(NodeId, RaftMessage)> {
        let mut vote_granted = false;

        if term < self.current_term {
            // The candidate is in an older term -- reject
            println!(
                "[Node {}] Rejecting vote for {} (term {} < {})",
                self.id, candidate_id, term, self.current_term
            );
        } else if self.voted_for.is_none() || self.voted_for == Some(candidate_id) {
            // We have not voted yet, OR we already voted for this
            // candidate (handling a retransmission)
            vote_granted = true;
            self.voted_for = Some(candidate_id);
            self.reset_election_timer();
            println!(
                "[Node {}] Granting vote to {} for term {}",
                self.id, candidate_id, term
            );
        } else {
            // We already voted for someone else this term
            println!(
                "[Node {}] Rejecting vote for {} (already voted for {:?})",
                self.id, candidate_id, self.voted_for
            );
        }

        // Send our response
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

The vote granting logic has three possible outcomes:

1. **Reject (old term):** the candidate is behind us. It should not become leader.
2. **Grant:** we have not voted yet this term (or we already voted for this same candidate). We grant the vote and reset our election timer to give the candidate time to win.
3. **Reject (already voted):** we already voted for a different candidate this term. We can only vote once.

> **Programming Concept: `Option<T>` -- Representing "Maybe"**
>
> `voted_for` has type `Option<NodeId>`. This means it is either `Some(node_id)` (we voted for a specific node) or `None` (we have not voted yet). The `Option` type is Rust's way of saying "this value might not exist." Unlike languages that use `null` or `undefined`, Rust forces you to handle both cases -- you cannot accidentally use a `None` as if it were a real value.
>
> `self.voted_for.is_none()` returns `true` if we have not voted. `self.voted_for == Some(candidate_id)` checks if we voted for this specific candidate.

### Step 5: Why reset the timer when granting a vote?

This is a subtle but important detail. When we grant a vote, we reset our election timer. Why?

If we voted for Node 3 and then immediately timed out and started our own election, we would be competing with the candidate we just voted for. That is counterproductive -- we should give Node 3 time to collect votes and become leader before we start our own election.

---

## Exercise 4: Winning the Election

**Goal:** Handle vote responses and transition to Leader when we have a majority.

### Step 1: Count the votes

```rust,ignore
impl RaftNode {
    /// Handle a response to our RequestVote.
    fn handle_vote_response(
        &mut self,
        from: NodeId,
        term: Term,
        vote_granted: bool,
    ) -> Vec<(NodeId, RaftMessage)> {
        // If we are no longer a candidate, ignore the vote
        if self.state != NodeState::Candidate {
            return Vec::new();
        }

        // If the response is from a different term, ignore it
        if term != self.current_term {
            return Vec::new();
        }

        if vote_granted {
            // Record the vote
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
            println!("[Node {}] Vote rejected by {}", self.id, from);
        }

        Vec::new()
    }

    /// Transition to Leader state.
    fn become_leader(&mut self) {
        println!(
            "[Node {}] === WON ELECTION for term {} with {} votes ===",
            self.id, self.current_term, self.votes_received.len()
        );

        self.state = NodeState::Leader;
        self.leader_id = Some(self.id);

        // Leader-specific initialization will happen in Chapter 15:
        // - Set nextIndex for each peer to last log index + 1
        // - Set matchIndex for each peer to 0
        // - Send initial heartbeat to all peers
    }
}
```

The logic is straightforward:

1. Only count votes if we are still a Candidate (we might have stepped down due to a higher term).
2. Only count votes for our current term (stale responses are irrelevant).
3. Insert the voter into our `HashSet` (automatically prevents double-counting).
4. If we have a majority, we win!

### Step 2: Handle heartbeats (AppendEntries)

Even though full log replication is Chapter 15, we need heartbeats now. A leader sends empty `AppendEntries` messages to tell followers: "I am still here, do not start an election."

```rust,ignore
impl RaftNode {
    /// Handle an AppendEntries message (heartbeat or log replication).
    fn handle_append_entries(
        &mut self,
        from: NodeId,
        term: Term,
        leader_id: NodeId,
    ) -> Vec<(NodeId, RaftMessage)> {
        if term < self.current_term {
            // Old leader -- reject
            return vec![(
                from,
                RaftMessage::AppendEntriesResponse {
                    term: self.current_term,
                    success: false,
                },
            )];
        }

        // Valid heartbeat from the leader
        self.state = NodeState::Follower;
        self.leader_id = Some(leader_id);
        self.reset_election_timer();

        vec![(
            from,
            RaftMessage::AppendEntriesResponse {
                term: self.current_term,
                success: true,
            },
        )]
    }
}
```

When a follower receives a heartbeat:
1. It confirms the leader is alive by resetting the election timer.
2. It records who the leader is.
3. It replies with success.

If a Candidate receives a heartbeat from a leader with a valid term, it steps down to Follower. The election is over -- someone else won.

> **What Just Happened?**
>
> We implemented the full election lifecycle:
>
> 1. **Start:** follower times out, becomes candidate, sends RequestVote to all peers
> 2. **Vote:** peers check the rules and grant or reject the vote
> 3. **Win:** candidate collects a majority of votes and becomes leader
> 4. **Maintain:** leader sends heartbeats to prevent new elections
>
> This cycle repeats whenever the leader fails. Followers notice the missing heartbeats, start a new election, and a new leader is chosen. The cluster recovers automatically.

---

## Exercise 5: Running an Election Simulation

**Goal:** Build a simple simulation that creates a cluster of nodes and runs an election.

### Step 1: Create the simulation

Create `src/bin/raft-election-sim.rs`:

```rust,ignore
// src/bin/raft-election-sim.rs

use std::collections::HashMap;
use std::thread;
use std::time::Duration;

// Import our raft module
use toydb::raft::{NodeId, RaftNode, RaftMessage, NodeState};

fn main() {
    println!("=== Raft Leader Election Simulation ===\n");

    // Create a 5-node cluster
    let node_ids: Vec<NodeId> = vec![1, 2, 3, 4, 5];
    let mut nodes: HashMap<NodeId, RaftNode> = HashMap::new();

    for &id in &node_ids {
        let peers: Vec<NodeId> = node_ids.iter()
            .filter(|&&peer_id| peer_id != id)
            .copied()
            .collect();
        nodes.insert(id, RaftNode::new(id, peers));
    }

    println!("Created {} nodes, all starting as followers\n", nodes.len());

    // Run the simulation for 20 ticks
    for tick in 1..=20 {
        println!("--- Tick {} ---", tick);

        // Collect all outgoing messages from all nodes
        let mut all_messages: Vec<(NodeId, NodeId, RaftMessage)> = Vec::new();

        for (&id, node) in nodes.iter_mut() {
            let messages = node.tick();
            for (to, msg) in messages {
                all_messages.push((id, to, msg));
            }
        }

        // Deliver all messages
        let mut responses: Vec<(NodeId, NodeId, RaftMessage)> = Vec::new();

        for (from, to, msg) in all_messages {
            if let Some(node) = nodes.get_mut(&to) {
                let reply_messages = node.handle_message(from, msg);
                for (reply_to, reply_msg) in reply_messages {
                    responses.push((to, reply_to, reply_msg));
                }
            }
        }

        // Deliver all responses
        for (from, to, msg) in responses {
            if let Some(node) = nodes.get_mut(&to) {
                node.handle_message(from, msg);
            }
        }

        // Print cluster status
        let mut leader_count = 0;
        for (&id, node) in &nodes {
            let state_str = match node.state {
                NodeState::Follower => "Follower",
                NodeState::Candidate => "Candidate",
                NodeState::Leader => {
                    leader_count += 1;
                    "LEADER"
                }
            };
            println!(
                "  Node {}: {} (term {})",
                id, state_str, node.current_term
            );
        }

        if leader_count == 1 {
            println!("\n=== Election complete! One leader elected. ===");
            break;
        }

        // Small delay between ticks (simulates time passing)
        thread::sleep(Duration::from_millis(50));
    }
}
```

### Step 2: Run the simulation

```
$ cargo run --bin raft-election-sim
=== Raft Leader Election Simulation ===

Created 5 nodes, all starting as followers

--- Tick 1 ---
  Node 1: Follower (term 0)
  Node 2: Follower (term 0)
  Node 3: Follower (term 0)
  Node 4: Follower (term 0)
  Node 5: Follower (term 0)

--- Tick 2 ---
[Node 3] Starting election for term 1
[Node 1] Granting vote to 3 for term 1
[Node 2] Granting vote to 3 for term 1
[Node 4] Granting vote to 3 for term 1
[Node 3] Received vote from 1 (2/3 needed)
[Node 3] Received vote from 2 (3/3 needed)
[Node 3] === WON ELECTION for term 1 with 3 votes ===
  Node 1: Follower (term 1)
  Node 2: Follower (term 1)
  Node 3: LEADER (term 1)
  Node 4: Follower (term 1)
  Node 5: Follower (term 1)

=== Election complete! One leader elected. ===
```

> **What Just Happened?**
>
> We simulated a complete Raft election in memory, with no real networking. Node 3 happened to timeout first (random timeout), started an election, received votes from a majority (itself + Nodes 1 and 2 = 3 out of 5), and became leader.
>
> The output might vary each time you run it because the election timeouts are random. Sometimes Node 1 wins, sometimes Node 5 wins. That randomness is the whole point -- it prevents ties.

### Common mistakes

**Mistake 1: Not handling the "already voted" case**

If your vote granting code does not check `voted_for`, a node might vote for multiple candidates in the same term. This breaks the safety guarantee: two candidates could each get a majority (impossible if each node votes only once).

**Mistake 2: Not stepping down on higher term**

If a leader does not step down when it sees a higher term, you can end up with two leaders. The term check at the top of `handle_message` prevents this.

**Mistake 3: Not resetting the timer on heartbeat**

If followers do not reset their election timer when they receive a heartbeat, they will keep starting elections even when the leader is healthy. This causes unnecessary leader changes and instability.

---

## The Election Safety Guarantee

Let's prove that Raft can never elect two leaders in the same term.

**Claim:** At most one leader is elected per term.

**Proof:** Suppose two nodes, A and B, both become leader in Term T. Each received votes from a majority. In a 5-node cluster, a majority is 3. But any two groups of 3 from a set of 5 must overlap by at least one node. Call that overlapping node X.

Node X voted for A and also voted for B in Term T. But our code says each node votes for at most one candidate per term (`voted_for` is set to one value and only cleared when the term changes).

Contradiction. Therefore, two leaders cannot be elected in the same term.

This is the fundamental safety property of Raft. Everything else -- log replication, commitment, durability -- builds on this guarantee.

---

## Exercises

### Exercise A: Split Vote Scenario

Modify the simulation to force a split vote: make two nodes start elections at the exact same time (by setting their timeouts to expire on the same tick). Observe how the randomized retry resolves the split.

<details>
<summary>Hint</summary>

Set both nodes' `election_deadline` to `Instant::now()` so they both timeout on the first tick. After the split vote, they will generate new random timeouts, and one will win on the retry.

```rust,ignore
nodes.get_mut(&1).unwrap().election_deadline = Instant::now();
nodes.get_mut(&2).unwrap().election_deadline = Instant::now();
```

</details>

### Exercise B: Node Crash Simulation

Add a "crash" feature to the simulation: remove a node from the cluster after it becomes leader, and observe how the remaining nodes elect a new leader.

<details>
<summary>Hint</summary>

After a leader is elected, remove it from the `nodes` HashMap. Continue ticking. The followers will not receive heartbeats, their timers will fire, and a new election will begin.

```rust,ignore
// Find and remove the leader
let leader_id = nodes.iter()
    .find(|(_, n)| n.state == NodeState::Leader)
    .map(|(&id, _)| id);

if let Some(id) = leader_id {
    println!("!!! Node {} (leader) crashed !!!", id);
    nodes.remove(&id);
    // Also remove the crashed node from each remaining node's peers list
}
```

</details>

### Exercise C: Cluster Size Experiment

Run the simulation with cluster sizes of 3, 5, 7, and 9. For each size, answer:
- How many nodes can fail while still electing a leader?
- What is the minimum quorum size?

<details>
<summary>Hint</summary>

A cluster of size N can tolerate (N-1)/2 failures. The quorum size is N/2 + 1.

| Cluster Size | Quorum | Tolerated Failures |
|---|---|---|
| 3 | 2 | 1 |
| 5 | 3 | 2 |
| 7 | 4 | 3 |
| 9 | 5 | 4 |

This is why production Raft clusters use odd numbers (3 or 5). Even numbers give no additional fault tolerance: a 4-node cluster tolerates 1 failure (same as 3), but requires 3 votes instead of 2.

</details>

---

## Summary

You built the foundation of a distributed consensus system:

- **Distributed consensus** solves the problem of keeping multiple servers in agreement
- **Raft** is a consensus algorithm designed to be understandable
- **State machines** model the three roles: Follower, Candidate, Leader
- **Rust enums** represent states, and **match** ensures every state is handled
- **Terms** act as a logical clock -- higher terms always win
- **Elections** use randomized timeouts to prevent ties
- **Vote uniqueness** (one vote per term) guarantees at most one leader per term
- **Heartbeats** keep the leader alive in the eyes of followers

In the next chapter, we tackle the real purpose of all this: **log replication**. A leader that wins elections but does not replicate data is useless. Chapter 15 makes the leader actually do its job -- accepting writes, sending them to followers, and confirming when a majority has the data.
