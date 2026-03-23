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
