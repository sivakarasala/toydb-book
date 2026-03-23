## What You Built

In this chapter, you:

1. **Modeled Raft states as an enum** — `Follower`, `Candidate`, and `Leader` variants, with Rust making sure you handle every case
2. **Built a RaftNode struct** — tracking the current term, who you voted for, and when the election timer expires
3. **Implemented leader election** — a candidate increments its term, votes for itself, and asks peers for votes. If it gets a majority, it becomes leader
4. **Handled term rules** — if a node sees a higher term, it immediately steps down to follower. This prevents stale leaders
5. **Tested elections** — a simulated cluster that elects a leader, handles split votes, and recovers from partitions

Your database is no longer a single point of failure. Multiple nodes can coordinate to elect a leader. In Chapter 15, the leader will actually replicate data to followers — so if the leader crashes, nothing is lost.

---

### Key Rust concepts practiced

- **Enums with data** — `NodeState` is a perfect fit for Rust enums: each variant means something different, and `match` forces you to handle all of them
- **Randomized timeouts** — election timeouts use a random range so that nodes do not all try to become leader at the same time
- **Message passing** — `RequestVote` and `VoteResponse` structs carry the information nodes need to make voting decisions
