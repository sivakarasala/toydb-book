## Design Insight: Define Errors Out of Existence

In *A Philosophy of Software Design*, Ousterhout advocates designing systems so that error conditions simply cannot occur, rather than detecting and handling them:

> *"The best way to deal with exception handling complexity is to define your APIs so that there are no exceptions to handle."*

Raft is a masterclass in this principle. Its predecessor, Paxos, is notoriously difficult to understand and implement. Lamport's original Paxos paper required multiple rounds of discussion before the research community understood it. Implementations frequently had subtle bugs.

Raft "defines errors out of existence" through simplification:

**1. Single leader instead of multi-proposer.** Paxos allows any node to propose values, which creates complex conflict resolution. Raft restricts proposals to a single leader. This eliminates an entire class of conflicts: if only one node can write, writes cannot conflict.

**2. Sequential terms instead of concurrent ballots.** Paxos ballots can overlap in complex ways. Raft terms are strictly sequential — each term has at most one leader, and a higher term always supersedes a lower one. This makes reasoning about correctness much simpler.

**3. Log entries are immutable once committed.** A committed entry will never be overwritten or removed. This eliminates the need for complex conflict resolution in the log.

**4. Leader completeness property.** The log completeness check in RequestVote ensures that any elected leader has all committed entries. This eliminates the need for a "log catch-up" protocol for new leaders — the leader already has everything.

Each simplification removes a category of error conditions. The result is an algorithm that is provably equivalent to Paxos in safety and liveness, but dramatically simpler to understand and implement correctly. The Raft paper's user study showed that students scored significantly higher on Raft questions than Paxos questions, even with the same amount of study time.

The lesson for software design: before writing error handling code, ask whether you can change the design so the error cannot occur. The best error handler is the one you never need to write.

> *"The best way to deal with exception handling complexity is to define your APIs so that there are no exceptions to handle."*
> — John Ousterhout

---
