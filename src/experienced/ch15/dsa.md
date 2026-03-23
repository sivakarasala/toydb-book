## DSA in Context: The Raft Replicated Log

The Raft log is a specialized data structure with unique properties that arise from its distributed context.

### Log as an ordered event stream

At its core, the Raft log is an append-only sequence of events. This is the same abstraction as:

- **Database write-ahead logs (WAL)**: PostgreSQL's WAL, MySQL's binlog
- **Event sourcing**: storing every state change as an immutable event
- **Kafka**: a distributed commit log
- **Git**: a chain of commits (each commit references its parent)

The insight: if every node applies the same events in the same order, they end up in the same state. This is the **state machine replication** principle, proven by Schneider in 1990.

### Comparing replication strategies

| Strategy | Consistency | Latency | Availability |
|----------|------------|---------|-------------|
| Single leader (Raft) | Linearizable | Higher (majority ack) | Majority needed |
| Multi-leader | Eventual | Lower (local ack) | Any node can write |
| Leaderless (Dynamo) | Eventual | Lowest (quorum write) | Configurable |
| Chain replication | Linearizable | Higher (all nodes) | All nodes needed |

Raft chooses linearizable consistency at the cost of latency (must wait for majority acknowledgment) and availability (cannot accept writes without a majority).

### Log compaction

The log grows unboundedly. In production, you need **log compaction** — periodically taking a snapshot of the state machine and discarding all log entries before the snapshot:

```
Before compaction:
  Log: [1] [2] [3] [4] [5] [6] [7] [8] [9] [10]
  Snapshot: (none)

After compaction at index 7:
  Log: [8] [9] [10]
  Snapshot: state machine state as of entry 7
```

When a follower is so far behind that the leader has already compacted the entries it needs, the leader sends the snapshot instead. This is the `InstallSnapshot` RPC in the Raft paper.

### Performance characteristics

```
Operation          | Time complexity | Amortized
-------------------|-----------------|-----------
Append (leader)    | O(1)            | O(1)
Replicate (leader) | O(entries * peers) | O(peers) per entry
Commit check       | O(peers)        | O(peers)
Apply              | O(1) per entry  | O(1)
Consistency check  | O(1)            | O(1)
Conflict repair    | O(divergent entries) | Rare
```

The critical path for a write: client -> leader append -> replicate to majority -> commit -> respond. This is typically 2-4 network round trips, plus the time to persist the entry to disk (covered in Chapter 16).

---
