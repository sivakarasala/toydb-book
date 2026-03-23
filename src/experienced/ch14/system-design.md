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
