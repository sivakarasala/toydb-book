# Chapter 14: Raft -- Leader Election

Your database runs on one server. It handles multiple connections beautifully thanks to async. But what if that server's power goes out? What if the hard drive fails? What if the process crashes?

Everything is gone. Every row, every table, every piece of data -- vanished. You might have backups, but those are always behind. Everything since the last backup is lost.

The solution is to keep copies of the data on multiple servers. If one server dies, the others continue. But this introduces a hard problem: if three servers have copies of the data and clients can write to any of them, the copies can diverge. Server A says Alice's balance is $100. Server B says it is $150. Which one is right?

This is the **consensus problem** -- getting multiple servers to agree on a single version of the truth. And the algorithm we will use to solve it is called **Raft**.

By the end of this chapter, you will have:

- An understanding of why distributed consensus is hard
- A `NodeState` enum with `Follower`, `Candidate`, and `Leader` variants
- A `RaftNode` struct with term tracking and vote management
- Election timeout detection with randomized timing
- The `RequestVote` RPC -- asking for votes and granting them
- A working election simulation that elects a leader from a cluster of nodes

---
