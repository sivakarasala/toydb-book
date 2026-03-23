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
