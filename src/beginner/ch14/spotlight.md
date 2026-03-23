## Spotlight: State Machines in Rust

Every chapter in this book has one **spotlight concept**. This chapter's spotlight is **state machines in Rust** -- using enums and `match` to model states and transitions, with the compiler checking that you handle every case.

### What is a state machine?

A state machine is a system that can be in one of a fixed number of states, with defined rules for moving between states.

You encounter state machines every day:

- **A traffic light:** Green -> Yellow -> Red -> Green. It can never be Green and Red at the same time.
- **An elevator:** Stopped -> Moving Up -> Stopped -> Moving Down -> Stopped. It cannot jump from Moving Up to Moving Down without stopping.
- **A vending machine:** Waiting -> Coins Inserted -> Item Selected -> Dispensing -> Waiting.

The key property: **at any moment, the system is in exactly one state**, and only certain transitions are allowed from that state.

### A Raft node as a state machine

A Raft node can be in one of three states:

- **Follower** -- the passive state. Listens for messages from the leader. Does what it is told.
- **Candidate** -- actively running for leader. Asking other nodes to vote for it.
- **Leader** -- the boss. Sends heartbeats to followers, coordinates writes.

The allowed transitions:

```
                      +------------------+
                      |                  |
                      v                  |
          +-----------------+            |
  +------>|    FOLLOWER     |<-----+     |
  |       +-----------------+      |     |
  |              |                 |     |
  |   election   |                 |     |
  |   timeout    |           discover    |
  |              v           higher      |
  |       +-----------------+  term      |
  |       |   CANDIDATE     |-----+     |
  |       +-----------------+            |
  |              |                       |
  |   wins       |                       |
  |   election   |                       |
  |              v                       |
  |       +-----------------+            |
  +-------|     LEADER      |------------+
          +-----------------+
           discover higher term
```

Four transitions, each with a clear trigger:

1. **Follower -> Candidate**: the election timer fires (no heartbeat from a leader)
2. **Candidate -> Leader**: receives votes from a majority of nodes
3. **Candidate -> Follower**: discovers a higher term (another node won)
4. **Leader -> Follower**: discovers a higher term (was disconnected, cluster moved on)

Notice what is NOT allowed: a Follower cannot become a Leader directly. It must go through Candidate first. A Leader cannot become a Candidate -- if it discovers a higher term, it steps all the way down to Follower.

### Enums as states

Rust enums are perfect for modeling states:

```rust
#[derive(Debug, Clone, PartialEq)]
enum NodeState {
    Follower,
    Candidate,
    Leader,
}
```

Unlike enums in some other languages (where they are just numbers or strings), Rust enums can carry data inside each variant:

```rust,ignore
#[derive(Debug, Clone)]
enum NodeState {
    Follower {
        voted_for: Option<u64>,      // who we voted for
        leader_id: Option<u64>,      // who we think the leader is
    },
    Candidate {
        votes_received: HashSet<u64>, // who voted for us
    },
    Leader {
        next_index: HashMap<u64, u64>,  // per-peer bookkeeping
    },
}
```

Each state carries only the data it needs. A Follower does not need `next_index` -- that is leader-specific data. A Leader does not need `voted_for` -- voting only happens during elections.

### Match expressions for transitions

The `match` keyword forces you to handle every state:

```rust,ignore
fn handle_timeout(&mut self) {
    match &self.state {
        NodeState::Follower => {
            // No heartbeat from leader -- start an election
            self.start_election();
        }
        NodeState::Candidate => {
            // Election timed out (split vote) -- try again
            self.start_election();
        }
        NodeState::Leader => {
            // Leaders send heartbeats, they do not time out
            self.send_heartbeats();
        }
    }
}
```

If you add a fourth state later, every `match` in your code that does not handle it becomes a compiler error. The compiler says: "You forgot to handle the new case." This is called **exhaustive matching**, and it prevents an entire category of bugs.

> **What Just Happened?**
>
> We defined a state machine using a Rust enum. Each variant represents a state, and `match` expressions handle transitions. The compiler guarantees we never forget to handle a state. This is one of Rust's superpowers -- in many other languages, a `switch` or `if/else` chain with a missing case silently does nothing. In Rust, it is a compile error.

---
