## Design Insight: Modules Should Be Deep

In *A Philosophy of Software Design*, Ousterhout distinguishes between **deep modules** (simple interface, complex implementation) and **shallow modules** (complex interface, simple implementation). Deep modules are better — they hide complexity behind simple abstractions.

> *"The best modules are those whose interfaces are much simpler than their implementations."*

The Raft log is a deep module. Its interface is remarkably simple:

```rust,ignore
// The interface (what callers see):
log.append(term, command)           // add an entry
log.get(index)                      // read an entry
log.matches(index, term)            // consistency check
log.append_entries(prev, entries)   // replicate entries
```

Five methods. That is the entire interface. Behind this interface, the implementation handles:
- Term-based conflict detection
- Automatic truncation of divergent entries
- The Log Matching Property invariant
- Index translation (1-based external, 0-based internal)
- Efficient sequential access patterns

The `RaftNode` is also a deep module. Its interface is two methods:

```rust,ignore
node.tick()                         // check timeouts, produce messages
node.handle_message(from, msg)      // process incoming message, produce responses
```

Behind this interface: leader election with randomized timeouts, vote counting with quorum detection, term management with automatic step-down, log replication with consistency repair, commitment with majority verification, and state machine application.

A shallow alternative would expose all the internal state transitions as separate methods: `start_election()`, `grant_vote()`, `count_votes()`, `become_leader()`, `check_commit()`, `advance_commit()`, `apply_entry()`. The caller would need to understand the Raft protocol to call them in the right order. The deep interface — `tick()` and `handle_message()` — hides all of this. The caller just delivers messages and checks for outgoing messages.

This is why the Raft paper is so effective as a teaching tool: it presents a complex algorithm through a simple interface (two RPCs: RequestVote and AppendEntries). The implementation is non-trivial, but the interface is elegant. Deep modules make complex systems manageable.

> *"The best modules are those whose interfaces are much simpler than their implementations."*
> — John Ousterhout

---
