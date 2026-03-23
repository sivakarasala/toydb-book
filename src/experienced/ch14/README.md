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
