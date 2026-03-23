# Chapter 16: Raft -- Durability & Recovery

Your Raft cluster elects leaders and replicates log entries. Everything works beautifully -- until you pull the plug. Turn off the power, restart the server, and what happens?

Nothing. The server wakes up with a blank memory. It does not know what term it was in, who it voted for, or what entries were in its log. All that carefully replicated data? Gone.

This is not a theoretical concern. Servers crash. Hard drives fail. Operating systems kernel-panic. Data centers lose power. A database that cannot survive a restart is a toy. This chapter makes it real.

You will build a **write-ahead log (WAL)** that persists Raft state to disk, a recovery procedure that reads everything back on startup, and snapshots that summarize old history so the log does not grow forever.

By the end of this chapter, you will have:

- An understanding of why durability matters and what must be persisted
- A write-ahead log that writes entries to disk with checksums
- Persistent storage for `current_term` and `voted_for`
- A recovery procedure that reconstructs state from disk on startup
- Snapshots for compacting old log entries
- A solid understanding of file ownership in Rust

---
