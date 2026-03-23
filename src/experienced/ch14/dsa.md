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
