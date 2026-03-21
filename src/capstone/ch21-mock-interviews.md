# Chapter 21: Mock Interviews

You have built a database from scratch. You understand Rust's ownership model, trait-based abstractions, and the full stack from storage to SQL to consensus. You have solved DSA problems using database domain data and designed systems at scale. Now it is time to put it all together under pressure.

This chapter contains three complete mock interview scenarios: a behavioral interview, a system design interview, and a coding interview. Each one is written as a realistic simulation, complete with interviewer dialogue, candidate responses, and commentary on what works and what does not. Read them actively. Cover the candidate's response, try to answer yourself, then compare.

These are not abstract exercises. Every answer draws on the toydb codebase you have been building throughout this book.

---

## Pre-Interview Checklist

Before you simulate any interview, prepare the way you would for a real one.

### Environment Setup
- [ ] A quiet space with a whiteboard, paper, or tablet for diagrams
- [ ] A code editor or shared doc open (no autocomplete — most interviews disable it)
- [ ] A timer visible on your desk
- [ ] Water within reach

### Knowledge Review
- [ ] Review the DSA patterns from Chapter 19 — know when each applies
- [ ] Review the system design framework from Chapter 20 — requirements, capacity, HLD, deep dives
- [ ] Skim your toydb codebase — be ready to reference real decisions you made
- [ ] Practice explaining your approach out loud before writing code

### Mental Preparation
- [ ] Accept that you will not produce perfect code under time pressure
- [ ] Plan to spend at least 30% of your time talking before coding
- [ ] Remind yourself: interviewers evaluate your *process*, not just your output
- [ ] If you get stuck, say so. Silence is worse than "I am considering two approaches here."

---

## Mock Interview 1: Behavioral (30 Minutes)

**Setting:** You join a video call. The interviewer introduces herself as Sarah, an engineering manager. The call is camera-on, no code editor.

> **Sarah:** "Thanks for joining. I would like to hear about your experience building software systems. I will ask a few behavioral questions. There are no right or wrong answers — I am interested in how you think, how you make decisions, and how you work with complexity. Ready?"

> **You:** "Ready."

---

### Question 1: "Tell me about a complex system you built."

> **Sarah:** "Tell me about the most complex system you have built. Walk me through the architecture, the key technical decisions, and what you learned."

#### Model Answer

> **You:** "I built a distributed SQL database from scratch in Rust — a full stack from storage engine to SQL parser to consensus protocol. Let me walk you through the architecture and three key decisions.
>
> The system has six layers. At the bottom is a key-value storage engine. I built two implementations — an in-memory BTreeMap store and a BitCask-style log-structured store that persists to disk with CRC checksums and crash recovery. Both implement the same `Storage` trait, so the rest of the system works with either one.
>
> On top of storage, I built an MVCC layer for concurrency control. Instead of locking rows, it keeps multiple versions of each key. Each transaction reads from a snapshot frozen at its start time, so readers never block writers. This is the same approach PostgreSQL uses.
>
> The SQL layer has a hand-written lexer, a recursive descent parser with precedence climbing for expressions, a query planner, a cost-based optimizer, and a Volcano-model executor. The executor uses pull-based iteration — each operator implements `next()` and returns one row at a time.
>
> For fault tolerance, I implemented the Raft consensus protocol — leader election with randomized timeouts, log replication with consistency checks, and durability with snapshot support. The final integration layer routes writes through Raft and serves reads from local state.
>
> Three key decisions:
>
> First, I defined a `Storage` trait with four methods — set, get, delete, scan — before writing any implementation. This was a strategic investment. When I added the persistent BitCask engine, I swapped it in without changing any other code. When MVCC needed versioned keys, it composed with any storage backend automatically.
>
> Second, I chose the Volcano model for query execution over a simpler approach like evaluating everything into vectors. The Volcano model takes longer to implement because each operator needs internal state management, but it handles large tables gracefully — rows flow through the pipeline one at a time without loading everything into memory.
>
> Third, I implemented Raft with a deterministic test harness. Instead of testing with real network timeouts, I built a simulated network where I could control message delivery, drop packets, and trigger elections at specific moments. This made the distributed tests fast and reproducible — a 30-second integration test covers scenarios that would take minutes with real networking."

> **Sarah:** "That is a lot of ground. If you had to pick one thing you would design differently with hindsight, what would it be?"

> **You:** "Key encoding. I used string keys like `table/users/1/v3` because they are human-readable and easy to debug. In production, you would use binary key encoding — big-endian integers that sort correctly in byte order. The string approach works but creates a subtle coupling: the MVCC layer knows the key format because it constructs version-suffixed keys. Binary encoding with a structured key type would make that boundary cleaner and more efficient."

**Commentary:** This answer follows the STAR structure (Situation, Task, Action, Result) implicitly. It gives architectural context, highlights specific decisions with reasoning, and demonstrates self-awareness by naming a flaw. Sarah can drill into any layer because the answer is specific enough to invite follow-ups but broad enough to cover the full system.

### What makes this answer strong

1. **Specific technical depth.** "BitCask-style log-structured store with CRC checksums" is much stronger than "a database that stores things."
2. **Decision reasoning.** Each decision includes *why*: "strategic investment," "handles large tables gracefully," "fast and reproducible."
3. **Trade-off awareness.** The redesign answer shows engineering maturity — you can identify weaknesses in your own work.
4. **Naming real patterns.** "Volcano model," "MVCC," "Raft" — using correct terminology signals competence.

### What would weaken this answer

- Describing only what you built without explaining *why* you made specific choices
- Listing technologies without demonstrating understanding
- Claiming the design was perfect
- Speaking for more than 3 minutes without checking if the interviewer wants to go deeper on something

---

### Question 2: "Tell me about a time you had to make a difficult trade-off."

> **Sarah:** "Can you tell me about a specific technical trade-off you faced? What were the options, how did you decide, and what were the consequences?"

#### Model Answer

> **You:** "The hardest trade-off was choosing between strong consistency and simplicity in the storage layer.
>
> When I built the MVCC transaction layer, I needed to encode version numbers into storage keys. I had two options. Option one: use the existing string-based keys and embed version numbers as suffixes — `users/1/v3`. This was quick to implement and debuggable. Option two: redesign the storage trait to support composite keys natively — `Storage::set_versioned(key, version, value)`. This was cleaner architecturally but required changing every storage implementation, every test, and every caller.
>
> I chose option one — the string-based approach. My reasoning: I was building a learning project, not a production database. The string keys let me inspect data with simple print statements during debugging. The architectural impurity — MVCC knowing the key format — was a real cost but a manageable one.
>
> The consequence showed up later. When I added the query planner, it needed range scans over table keys. The planner had to construct key prefixes like `table/users/` to scan all user rows. This meant the planner also knew the key format — the abstraction leak spread from MVCC to the SQL layer. If I had invested in structured keys upfront, both MVCC and the planner would have used typed key objects instead of formatted strings.
>
> The lesson I took away: in software design, coupling tends to spread. A small shortcut in one layer becomes a pattern that other layers imitate. If the shortcut is truly short-lived, it is fine. But if it persists, it becomes the architecture."

> **Sarah:** "How would you approach this kind of decision on a team with other engineers?"

> **You:** "I would write a short design document — maybe half a page — describing both options with their trade-offs. I would timebox the discussion to 30 minutes. The key is framing it as a reversibility question: how hard would it be to change this later? If the decision is easily reversible, I would bias toward the simpler option and move fast. If it is hard to reverse — like a storage key format that every layer depends on — I would invest the upfront time."

**Commentary:** This answer works because it is honest. The candidate chose the simpler option, acknowledges the cost, and extracts a generalizable lesson. The follow-up about team decisions shows collaboration skills and decision-making frameworks.

---

### Question 3: "How do you approach debugging a complex issue?"

> **Sarah:** "Tell me about a time you had to debug something that was not straightforward. How did you approach it?"

#### Model Answer

> **You:** "When I was testing the Raft leader election, I hit a bug where elections would sometimes hang — no node would become leader, and the cluster would sit in a perpetual candidate state. This happened maybe one in ten test runs, which made it hard to reproduce.
>
> My approach had three phases. First, I added structured logging — every state transition (follower to candidate, vote granted, vote denied, term change) was logged with the node ID, term, and timestamp. I ran the test fifty times and collected the logs from every failure.
>
> Second, I looked for patterns. In every hanging case, I noticed that two nodes started elections at the same time with the same term. Both received one vote (their own) but neither received a majority. Then they both timed out and started new elections — again at the same time, because their election timeouts were identical.
>
> The root cause was that I was using a fixed election timeout instead of a randomized one. Raft requires randomized timeouts specifically to prevent this — if every node waits a different random duration, it is overwhelmingly likely that one node times out first and wins the election before others start competing.
>
> The fix was two lines: replace `timeout = 150ms` with `timeout = 150ms + rand(0..150ms)`. But finding those two lines required understanding the invariant that the Raft paper assumes: election timeouts must be randomly distributed across nodes.
>
> The lesson: when debugging distributed systems, add enough logging to reconstruct the event sequence across all participants. The bug is usually in the interaction pattern, not in a single node's logic. Looking at one node's logs would not have revealed the simultaneous-election pattern."

**Commentary:** Strong debugging answers have three components: (1) how you reproduced the issue, (2) how you narrowed down the cause, and (3) what you learned. This answer demonstrates systematic thinking (structured logging, pattern recognition) rather than trial-and-error. The mention of the Raft paper's assumption shows you understand not just the code but the theory behind it.

---

### Behavioral Interview Tips

| Do | Do Not |
|----|--------|
| Give specific examples with technical detail | Speak in vague generalities |
| Explain your reasoning for decisions | Just state what you did without why |
| Acknowledge trade-offs and mistakes | Claim everything was perfect |
| Quantify where possible (timelines, metrics) | Use unmeasurable adjectives ("it was really hard") |
| Connect lessons to general principles | Treat each story as one-off |
| Keep answers to 2-3 minutes, then check in | Monologue for 5+ minutes |

---

## Mock Interview 2: System Design (45 Minutes)

**Setting:** You join a video call. The interviewer introduces himself as Marcus, a staff engineer. He shares a collaborative whiteboard.

> **Marcus:** "We are going to work through a system design problem. I will describe the requirements and then we will iterate on the design together. I care about how you break down the problem and reason about trade-offs. Feel free to ask clarifying questions at any point. Ready?"

> **You:** "Ready."

---

### The Problem

> **Marcus:** "Design a database backend for a social media platform. Think Instagram-scale — hundreds of millions of users, billions of posts, real-time feeds, and global access."

---

### Step 1: Requirements Gathering (5 minutes)

> **You:** "Before I start drawing, I want to understand the scope. Let me ask a few questions.
>
> What are the core features? I am thinking: user profiles, posts (text + images), follow relationships, news feed (timeline of posts from people you follow), likes, and comments. Is that roughly right?"

> **Marcus:** "Yes, that covers it. Focus on the feed — that is the hardest part."

> **You:** "What is the expected scale? Users, posts per day, feed reads per second?"

> **Marcus:** "500 million registered users, 100 million daily active. 10 million new posts per day. 1 billion feed reads per day."

> **You:** "Consistency requirements for the feed? If I post something, do my followers need to see it instantly, or is a few seconds of delay acceptable?"

> **Marcus:** "A few seconds is fine. Eventual consistency for the feed. But likes and comments should be reflected within a second."

> **You:** "Geographic distribution?"

> **Marcus:** "Global users. Data centers in US, Europe, and Asia."

> **You:** "Got it. Let me summarize the requirements, then I will estimate capacity."

**Commentary:** Five minutes of questions saves twenty minutes of designing the wrong system. Notice the candidate asked about consistency (the hardest design constraint) and scale (which drives every architectural choice).

---

### Step 2: Capacity Estimation (3 minutes)

> **You:** "Let me do some back-of-envelope math."

```
Users: 500M registered, 100M DAU

Posts:
  10M new posts/day = ~116 posts/sec
  Average post: 1 KB text + metadata, 500 KB image (stored separately)
  Daily storage: 10M * 1 KB = 10 GB metadata, 10M * 500 KB = 5 TB images

Feed reads:
  1B feed reads/day = ~11,600 reads/sec
  Average feed: 50 posts = 50 KB of metadata per read
  Daily feed bandwidth: 1B * 50 KB = 50 TB

Follow graph:
  Average user follows 200 people
  Total edges: 500M * 200 = 100B follow edges
  Storage: 100B * 16 bytes (two user IDs) = 1.6 TB

Storage totals (1 year):
  Post metadata: 3.6 TB
  Images: 1.8 PB
  Follow graph: 1.6 TB (relatively stable)
```

> **Marcus:** "Those numbers look reasonable. The feed read rate — 11,600 reads/sec — is the key bottleneck?"

> **You:** "Exactly. And those are averages. Peak will be 5-10x, so we need to handle ~100,000 feed reads per second."

---

### Step 3: High-Level Design (10 minutes)

> **You:** "I am going to structure this around three subsystems: the post ingestion pipeline, the feed generation system, and the serving layer."

```
┌──────────────────────────────────────────────────────┐
│                     Clients                           │
│              (Mobile apps, Web)                       │
└──────────────────────┬───────────────────────────────┘
                       │
┌──────────────────────▼───────────────────────────────┐
│              API Gateway / Load Balancer               │
│         (Rate limiting, auth, routing)                 │
├──────────────────────────────────────────────────────┤
│                                                       │
│  ┌─────────────┐  ┌──────────────┐  ┌─────────────┐ │
│  │ Post Service │  │ Feed Service  │  │ User Service│ │
│  │             │  │              │  │             │ │
│  │ - Create    │  │ - Read feed  │  │ - Profile   │ │
│  │ - Delete    │  │ - Refresh    │  │ - Follow    │ │
│  │ - Like      │  │              │  │ - Unfollow  │ │
│  └──────┬──────┘  └──────┬───────┘  └──────┬──────┘ │
│         │                │                  │        │
├─────────┼────────────────┼──────────────────┼────────┤
│         │                │                  │        │
│  ┌──────▼──────┐  ┌──────▼───────┐  ┌──────▼──────┐ │
│  │ Posts DB    │  │ Feed Cache   │  │ Graph DB    │ │
│  │ (Sharded)  │  │ (Redis)      │  │ (Follow     │ │
│  │            │  │              │  │  relations) │ │
│  └─────────────┘  └──────────────┘  └─────────────┘ │
│                                                       │
│         ┌──────────────────────┐                      │
│         │  Message Queue       │                      │
│         │  (Fan-out pipeline)  │                      │
│         └──────────────────────┘                      │
│                                                       │
│         ┌──────────────────────┐                      │
│         │  Object Storage (S3) │                      │
│         │  (Images, videos)    │                      │
│         └──────────────────────┘                      │
└──────────────────────────────────────────────────────┘
```

> **You:** "The core challenge is the feed. There are two approaches: **fan-out on write** and **fan-out on read**. Let me explain both and then choose.
>
> Fan-out on write: when a user posts, immediately push the post ID to every follower's feed cache. If the user has 1,000 followers, that is 1,000 writes. The advantage: reading a feed is fast — just read from the cache. The disadvantage: celebrity problem. A user with 10 million followers generates 10 million writes per post.
>
> Fan-out on read: when a user reads their feed, query the posts table for all users they follow, merge the results, sort by timestamp. The advantage: posting is O(1). The disadvantage: reading is expensive — you must query N users' posts and merge them.
>
> The standard solution — and what I would use — is a hybrid. For normal users (< 10,000 followers), fan-out on write. For celebrities (> 10,000 followers), fan-out on read. When a normal user reads their feed, they get pre-computed results from the cache plus a real-time merge of celebrity posts."

> **Marcus:** "Good. Tell me more about how the feed cache works."

> **You:** "Each user's feed is a sorted set in Redis, keyed by user ID. The values are post IDs sorted by timestamp. When user A creates a post:
>
> 1. The Post Service writes the post to the Posts DB.
> 2. The Post Service publishes an event to the message queue: 'User A created post P.'
> 3. A fan-out worker consumes the event, queries the follow graph for A's followers, and for each follower, adds post ID P to their feed sorted set in Redis.
> 4. The feed cache is trimmed to the latest 500 posts per user — older posts fall off and are fetched from the Posts DB on demand.
>
> When a user opens their app and reads their feed, the Feed Service reads post IDs from their Redis sorted set, fetches the full post data from the Posts DB (with a local cache for hot posts), and returns the assembled feed."

---

### Step 4: Deep Dives (15 minutes)

> **Marcus:** "Let us talk about the Posts DB. How would you shard it?"

> **You:** "I would shard by user ID. All posts from a single user live on the same shard. This has three benefits:
>
> First, a user's profile page — which shows their own posts — hits a single shard. No scatter-gather.
>
> Second, deleting a user's account is a single-shard operation.
>
> Third, the fan-out worker already knows the post author's user ID, so it can look up the post from a single shard for cache population.
>
> The downside: hot users (celebrities who post frequently) create hot shards. I would mitigate this with read replicas for hot shards and by caching celebrity posts in a CDN."

> **Marcus:** "What about the follow graph? You mentioned a graph database."

> **You:** "The follow graph is the social network structure — who follows whom. The core query is: 'given user A, return all user IDs that A follows.' This is a classic adjacency list query.
>
> For our scale — 100 billion edges — I would use a sharded key-value store rather than a dedicated graph database. Each user's follow list is stored as a sorted set:
>
> - Key: `following:{user_id}` → Set of user IDs that this user follows
> - Key: `followers:{user_id}` → Set of user IDs that follow this user
>
> Both are maintained on every follow/unfollow operation. The `following` set drives fan-out on read (which users' posts to fetch). The `followers` set drives fan-out on write (which feed caches to update).
>
> This is stored in Redis for hot data with the full graph in a persistent store like DynamoDB as the source of truth."

> **Marcus:** "Good. Now here is the interesting question. How does this connect to what you built with toydb?"

> **You:** "Several connections:
>
> First, the Posts DB sharding strategy mirrors the range partitioning from our distributed SQL design in Chapter 20. Sharding by user ID is essentially assigning each user to a key range.
>
> Second, the fan-out pipeline is an eventually consistent system — exactly the opposite of what we built with Raft. Raft gives strong consistency at the cost of latency and availability. The feed pipeline gives eventual consistency at the cost of potential staleness. Understanding both extremes — and knowing when each is appropriate — is the core lesson.
>
> Third, the feed cache in Redis is a materialized view. In our database, a view is a stored query. The feed cache is the same concept applied at scale — we pre-compute and store the result of 'SELECT posts FROM users WHERE user_id IN (SELECT followed_id FROM follows WHERE follower_id = ?)' so we do not have to execute that expensive query on every feed read.
>
> Fourth, the MVCC concept from Chapter 5 applies to the feed. When a user opens their feed, they see a consistent snapshot — even if new posts are being fan-out in the background. The feed cache provides this implicitly: a read gets the current state of the sorted set, and concurrent writes to the set do not corrupt the read."

> **Marcus:** "That is a strong connection. One more question: how do you handle deletes? If a user deletes a post, how do you remove it from all the feed caches?"

> **You:** "Two approaches. The lazy approach: when the Feed Service fetches full post data for a feed, it checks if each post still exists. If the Posts DB returns 'not found,' the Feed Service removes the post ID from the feed cache and skips it. This is eventually consistent and requires no fan-out.
>
> The eager approach: when a post is deleted, publish a delete event to the message queue. The fan-out workers remove the post ID from every follower's feed cache. This is faster but requires the same fan-out cost as creating the post.
>
> I would use the lazy approach for normal deletes and the eager approach for content moderation (where speed matters — you want policy-violating content removed from all feeds within seconds).
>
> This is actually the tombstone pattern from our BitCask storage engine in Chapter 3. In BitCask, deletes append a tombstone record rather than modifying the original. The lazy feed delete is the same idea — mark the post as deleted in the Posts DB, and let readers discover the tombstone naturally."

**Commentary:** The toydb connections are what elevate this answer. Any candidate can describe a feed architecture — it is well-documented. But connecting it to specific design decisions from a system you built demonstrates real understanding, not just pattern matching from a study guide.

---

### Step 5: Follow-Up Questions

> **Marcus:** "How would you monitor this system? What metrics would you track?"

> **You:** "Four categories:
>
> **Latency.** Feed read p50/p95/p99. Post creation latency. Fan-out completion time (time from post creation to the post appearing in the last follower's cache).
>
> **Throughput.** Requests per second by endpoint. Fan-out messages processed per second. Redis operations per second.
>
> **Queue depth.** Message queue backlog. If the fan-out queue depth grows, it means fan-out workers cannot keep up with post creation rate — we need to scale workers.
>
> **Consistency lag.** Time between post creation and feed cache update for a sample of posts. This measures how 'eventual' our eventual consistency is. Target: p99 < 5 seconds."

> **Marcus:** "What is the first thing that breaks at 10x scale?"

> **You:** "The fan-out queue. At 10x scale, we have 100 million new posts per day, and each post fans out to an average of 200 followers — that is 20 billion feed cache writes per day, or 230,000 per second. Redis can handle the write throughput, but the fan-out workers become the bottleneck.
>
> The fix: horizontal scaling of fan-out workers. They are stateless — each one reads from the queue, queries the follow graph, and writes to Redis. You can add workers linearly. The message queue (Kafka) partitions by user ID so each worker handles a subset of users, maintaining ordering guarantees."

---

### System Design Interview Tips

| Phase | Time | Focus |
|-------|------|-------|
| Requirements | 5 min | Clarify scope, consistency, scale |
| Estimation | 3 min | Back-of-envelope numbers, identify bottlenecks |
| High-level design | 10 min | Major components, data flow, key decision |
| Deep dives | 15 min | 2-3 components in detail, trade-offs |
| Follow-ups | 10 min | Monitoring, failure modes, scaling |

| Do | Do Not |
|----|--------|
| Start with requirements before designing | Jump straight to architecture |
| Quantify capacity to justify decisions | Hand-wave about "it should be fast enough" |
| Discuss trade-offs for every decision | Present one approach as the only option |
| Connect to real systems you have built | Describe theoretical systems you have only read about |
| Name concrete technologies with reasoning | Name-drop technologies without explaining why |
| Address the hardest part first (feed) | Spend time on easy parts (user profiles) |

---

## Mock Interview 3: Coding (45 Minutes)

**Setting:** You join a video call. The interviewer introduces himself as Raj, a senior engineer. He shares a collaborative code editor.

> **Raj:** "We will work through a coding problem today. I care about your problem-solving process more than perfect code. Talk me through your thinking as you go. Ready?"

> **You:** "Ready."

---

### Problem: Raft Log Reconciliation

> **Raj:** "In a distributed database using Raft consensus, when a new leader is elected, it needs to reconcile its log with each follower. The leader's log is the source of truth, and each follower's log may have diverged — it may be missing entries, or it may have extra entries from a previous leader that were never committed.
>
> Given the leader's log and a follower's log (both as arrays of `(term, index)` pairs), find the last index where they agree — the 'match point.' All entries after this point in the follower's log should be replaced with the leader's entries."

He types:

```
struct LogEntry {
    term: u64,
    index: u64,
}

Leader log:  [(1,1), (1,2), (2,3), (2,4), (3,5), (3,6)]
Follower log: [(1,1), (1,2), (2,3), (4,4), (4,5)]

Match point: index 3
  - Entries 1-3 agree (same term at same index)
  - At index 4, leader has term 2, follower has term 4 — divergence
  - Follower should discard entries 4-5 and accept leader's entries 4-6

Output: 3  (the last matching index)
```

---

#### Step 1: Clarifying Questions (2 minutes)

> **You:** "A few questions. Are the logs guaranteed to be sorted by index?"

> **Raj:** "Yes, always sorted in index order."

> **You:** "Are the indexes guaranteed to be contiguous? That is, no gaps — 1, 2, 3, 4?"

> **Raj:** "Yes, no gaps."

> **You:** "Can either log be empty?"

> **Raj:** "Yes. If the follower's log is empty, the match point is 0 — nothing matches."

> **You:** "And the match criterion is: the entries at the same index must have the same term?"

> **Raj:** "Exactly. Same index, same term. Once they disagree on a term at any index, all subsequent entries are considered divergent."

> **You:** "One more: do the logs always start at index 1?"

> **Raj:** "For this problem, yes. In a real Raft implementation they might start at a snapshot index, but let us keep it simple."

**Commentary:** The question about contiguous indexes matters — it determines whether you can use array indexing (O(1) access by index) or need a map (O(log n) lookup by index). The question about what constitutes a "match" ensures you are solving the right problem.

---

#### Step 2: Brute Force (3 minutes)

> **You:** "The straightforward approach: compare entries one by one from the beginning. Walk through both logs simultaneously. At each position, if the terms match, continue. If they differ, the previous position is the match point. If we exhaust the shorter log without disagreement, the match point is the length of the shorter log."

```rust
fn find_match_point_brute(
    leader: &[(u64, u64)],   // (term, index)
    follower: &[(u64, u64)],
) -> u64 {
    let mut match_point = 0;

    for i in 0..leader.len().min(follower.len()) {
        let (leader_term, leader_idx) = leader[i];
        let (follower_term, follower_idx) = follower[i];

        // Sanity check: indexes should be the same at position i
        if leader_idx != follower_idx {
            break;
        }

        if leader_term == follower_term {
            match_point = leader_idx;
        } else {
            break;  // Divergence found
        }
    }

    match_point
}
```

> **You:** "Time: O(min(L, F)) where L and F are the log lengths. Space: O(1). This is already linear — can we do better?"

> **Raj:** "Can we? Think about what Raft actually does."

---

#### Step 3: Optimized Approach (5 minutes)

> **You:** "In Raft's actual protocol, the leader does not send its entire log for comparison. It uses a binary search-like approach via the `AppendEntries` RPC.
>
> The leader starts by sending the follower the last entry. If the follower has a matching entry at that index and term, the match point is the end of the follower's log. If not, the leader tries an earlier index — specifically, it decrements `nextIndex` for that follower.
>
> We can simulate this with a binary search over the overlapping range. The key insight: once the logs agree at index `k`, they agree at all indices before `k` (because Raft's Log Matching Property guarantees that if two entries have the same index and term, all preceding entries are identical). This is the property that makes binary search correct.
>
> Wait — that is exactly the Log Matching Property from Chapter 15. If entry at index `k` matches in both logs (same term), then all entries 1 through `k` match. So we binary search for the rightmost index where they agree."

```rust
fn find_match_point(
    leader: &[(u64, u64)],   // (term, index)
    follower: &[(u64, u64)],
) -> u64 {
    if leader.is_empty() || follower.is_empty() {
        return 0;
    }

    let overlap = leader.len().min(follower.len());
    // Binary search for the rightmost matching index

    let mut lo: usize = 0;
    let mut hi: usize = overlap; // exclusive upper bound
    let mut match_point: u64 = 0;

    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        let (leader_term, leader_idx) = leader[mid];
        let (follower_term, _) = follower[mid];

        if leader_term == follower_term {
            // They agree at mid — the match point is at least mid
            match_point = leader_idx;
            lo = mid + 1; // search right half for a later match
        } else {
            // They disagree at mid — match point must be before mid
            hi = mid;
        }
    }

    match_point
}
```

> **Raj:** "Hold on. Is the binary search correct here? The Log Matching Property says if they agree at index k, they agree at all prior indices. But does disagreement at index k mean they disagree at all later indices?"

> **You:** "Good catch. Let me think about this... No, it does not. A follower could have entries from a different leader at indices 4-5 but then happen to have the same term as the leader at index 6 by coincidence (if the same leader held both terms).
>
> Actually wait — if the follower has a different term at index 4, and the leader has a different term at index 4, then the Log Matching Property tells us that the follower's entries from index 4 onward are suspect. But it is possible for the follower to have the same term at index 6 as the leader if both originated from the same leader in the same term.
>
> Hmm, but Raft says: if two logs contain an entry with the same index and term, then the logs are identical in all entries up through that index. So if the follower agrees with the leader at index 6, it MUST agree at all prior indices, including index 4. That contradicts our assumption that they disagree at index 4.
>
> So disagreement at index k DOES imply disagreement at all later indices (in the overlapping range). The binary search IS correct."

> **Raj:** "Walk me through why one more time."

> **You:** "The property is bidirectional in a sense. If they agree at index 6 — same term — then by the Log Matching Property, they agree at indices 1 through 6. If they disagree at index 4, then they cannot agree at any index 6 > 4, because agreeing at 6 would imply agreeing at 4. Contradiction. So the match/no-match boundary is monotonic: all agrees then all disagrees. Binary search works on monotonic predicates."

> **Raj:** "Good reasoning. That was the key insight. Now write the full solution with tests."

---

#### Step 4: Complete Solution (10 minutes)

```rust
#[derive(Debug, Clone, PartialEq)]
struct LogEntry {
    term: u64,
    index: u64,
}

/// Find the last log index where the leader and follower agree.
///
/// Uses binary search over the overlapping portion of both logs.
/// Correctness relies on Raft's Log Matching Property: if two logs
/// contain an entry with the same index and term, then the logs
/// are identical in all preceding entries.
fn find_match_point(leader: &[LogEntry], follower: &[LogEntry]) -> u64 {
    if leader.is_empty() || follower.is_empty() {
        return 0;
    }

    let overlap = leader.len().min(follower.len());
    let mut lo: usize = 0;
    let mut hi: usize = overlap;
    let mut match_point: u64 = 0;

    while lo < hi {
        let mid = lo + (hi - lo) / 2;

        if leader[mid].term == follower[mid].term {
            match_point = leader[mid].index;
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }

    match_point
}

/// Given the match point, return the entries the leader should send
/// to bring the follower up to date.
fn entries_to_send(leader: &[LogEntry], match_point: u64) -> Vec<LogEntry> {
    leader
        .iter()
        .filter(|e| e.index > match_point)
        .cloned()
        .collect()
}

fn main() {
    let leader = vec![
        LogEntry { term: 1, index: 1 },
        LogEntry { term: 1, index: 2 },
        LogEntry { term: 2, index: 3 },
        LogEntry { term: 2, index: 4 },
        LogEntry { term: 3, index: 5 },
        LogEntry { term: 3, index: 6 },
    ];

    let follower = vec![
        LogEntry { term: 1, index: 1 },
        LogEntry { term: 1, index: 2 },
        LogEntry { term: 2, index: 3 },
        LogEntry { term: 4, index: 4 },
        LogEntry { term: 4, index: 5 },
    ];

    let mp = find_match_point(&leader, &follower);
    println!("Match point: {}", mp); // 3

    let to_send = entries_to_send(&leader, mp);
    println!("Entries to send: {:?}", to_send);
    // [(2,4), (3,5), (3,6)]
}
```

---

#### Step 5: Test Cases (5 minutes)

> **You:** "Let me write test cases covering the edge cases."

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn entry(term: u64, index: u64) -> LogEntry {
        LogEntry { term, index }
    }

    #[test]
    fn normal_divergence() {
        let leader = vec![entry(1, 1), entry(1, 2), entry(2, 3), entry(2, 4)];
        let follower = vec![entry(1, 1), entry(1, 2), entry(3, 3)];
        assert_eq!(find_match_point(&leader, &follower), 2);
    }

    #[test]
    fn fully_matching() {
        let leader = vec![entry(1, 1), entry(1, 2), entry(2, 3)];
        let follower = vec![entry(1, 1), entry(1, 2), entry(2, 3)];
        assert_eq!(find_match_point(&leader, &follower), 3);
    }

    #[test]
    fn follower_shorter_but_matching() {
        let leader = vec![entry(1, 1), entry(1, 2), entry(2, 3), entry(3, 4)];
        let follower = vec![entry(1, 1), entry(1, 2)];
        assert_eq!(find_match_point(&leader, &follower), 2);
    }

    #[test]
    fn leader_shorter_but_matching() {
        let leader = vec![entry(1, 1), entry(1, 2)];
        let follower = vec![entry(1, 1), entry(1, 2), entry(4, 3), entry(4, 4)];
        assert_eq!(find_match_point(&leader, &follower), 2);
    }

    #[test]
    fn no_match_at_all() {
        let leader = vec![entry(2, 1), entry(2, 2)];
        let follower = vec![entry(1, 1), entry(1, 2)];
        assert_eq!(find_match_point(&leader, &follower), 0);
    }

    #[test]
    fn empty_follower() {
        let leader = vec![entry(1, 1), entry(1, 2)];
        let follower = vec![];
        assert_eq!(find_match_point(&leader, &follower), 0);
    }

    #[test]
    fn empty_leader() {
        let leader = vec![];
        let follower = vec![entry(1, 1), entry(1, 2)];
        assert_eq!(find_match_point(&leader, &follower), 0);
    }

    #[test]
    fn both_empty() {
        let leader: Vec<LogEntry> = vec![];
        let follower: Vec<LogEntry> = vec![];
        assert_eq!(find_match_point(&leader, &follower), 0);
    }

    #[test]
    fn single_entry_match() {
        let leader = vec![entry(1, 1)];
        let follower = vec![entry(1, 1)];
        assert_eq!(find_match_point(&leader, &follower), 1);
    }

    #[test]
    fn single_entry_no_match() {
        let leader = vec![entry(2, 1)];
        let follower = vec![entry(1, 1)];
        assert_eq!(find_match_point(&leader, &follower), 0);
    }

    #[test]
    fn divergence_at_first_entry() {
        let leader = vec![entry(1, 1), entry(2, 2), entry(2, 3)];
        let follower = vec![entry(3, 1), entry(3, 2)];
        assert_eq!(find_match_point(&leader, &follower), 0);
    }

    #[test]
    fn entries_to_send_after_match() {
        let leader = vec![
            entry(1, 1), entry(1, 2), entry(2, 3),
            entry(2, 4), entry(3, 5), entry(3, 6),
        ];
        let to_send = entries_to_send(&leader, 3);
        assert_eq!(to_send, vec![entry(2, 4), entry(3, 5), entry(3, 6)]);
    }

    #[test]
    fn entries_to_send_nothing() {
        let leader = vec![entry(1, 1), entry(1, 2)];
        let to_send = entries_to_send(&leader, 2);
        assert_eq!(to_send, vec![]);
    }
}
```

> **Raj:** "Strong test coverage. Let me ask a follow-up."

---

#### Step 6: Follow-Up (5 minutes)

> **Raj:** "In a real Raft implementation, the leader does not have both logs available locally. It can only probe the follower by sending an `AppendEntries` RPC with a `prevLogIndex` and `prevLogTerm`. The follower responds with success (if it has a matching entry) or failure (if it does not). How would you adapt your approach?"

> **You:** "The binary search still works, but instead of comparing array elements directly, each 'comparison' is a network round trip:
>
> 1. The leader picks a `prevLogIndex` (the midpoint).
> 2. It sends `AppendEntries` with that `prevLogIndex` and the corresponding `prevLogTerm`.
> 3. The follower checks if it has an entry at `prevLogIndex` with the matching `prevLogTerm`.
> 4. If yes, the leader searches the right half (try a later index).
> 5. If no, the leader searches the left half (try an earlier index).
>
> This takes O(log n) network round trips instead of n (the naive decrement-by-one approach in the basic Raft protocol).
>
> In practice, most Raft implementations use a hybrid: start with the decrement approach (which works well when logs are nearly in sync — usually just 1-2 entries behind), and fall back to binary search if the gap is large. CockroachDB does exactly this — if `nextIndex` has been decremented more than a threshold, switch to binary search to converge faster."

> **Raj:** "Excellent. You connected the algorithmic optimization to a real system implementation. That is exactly what I was looking for."

**Commentary:** The follow-up tests whether the candidate can bridge the gap between a clean algorithmic problem and the messy reality of distributed systems. The mention of CockroachDB's hybrid approach shows awareness of practical engineering, not just textbook algorithms.

---

### Coding Interview Tips

| Phase | Time | Focus |
|-------|------|-------|
| Clarifying questions | 2 min | Edge cases, constraints, input format |
| Brute force | 3 min | Simple working solution, complexity analysis |
| Optimize | 5 min | Key insight, approach discussion |
| Implement | 10 min | Clean code, good naming, handle edge cases |
| Test | 5 min | Walk through examples, write unit tests |
| Follow-ups | 5-10 min | Extensions, real-world connections |

| Do | Do Not |
|----|--------|
| Talk through your thinking before coding | Code in silence |
| Start with brute force, then optimize | Jump to the optimal solution without explaining |
| Write clean function signatures first | Start writing implementation details immediately |
| Test with the given example AND edge cases | Only test the happy path |
| Mention time and space complexity | Leave complexity analysis to the end (or skip it) |
| Connect to real systems when possible | Treat the problem as purely abstract |

---

## Common Anti-Patterns to Avoid

These patterns appear across all three interview types. Recognizing them in practice sessions will help you avoid them under pressure.

### 1. The Knowledge Dump

Reciting everything you know about a topic instead of answering the question. If asked "tell me about a trade-off," do not explain MVCC from scratch. Give a specific example of a specific trade-off you faced.

### 2. The Silent Thinker

Thinking in silence for 30+ seconds. Interviewers cannot evaluate your process if they cannot hear it. Think out loud: "I am considering two approaches — binary search and linear scan. Binary search works here because the predicate is monotonic."

### 3. The Premature Optimizer

Jumping to the optimal solution without discussing the brute force first. The brute force demonstrates that you understand the problem. The optimization demonstrates that you can improve on it. Skipping the first step makes the second less impressive.

### 4. The Architecture Astronaut

Designing a system with 15 microservices, three message queues, and a service mesh for a problem that could be solved with a single PostgreSQL instance. Start simple. Scale when the numbers demand it.

### 5. The Perfect Engineer

Claiming your design has no flaws. Every design has trade-offs. Naming them is a strength, not a weakness. "This approach trades write latency for read performance" is a stronger statement than "this approach is optimal."

---

## Summary

| Interview Type | Key Skill | toydb Connection |
|---------------|-----------|-----------------|
| Behavioral | Structured storytelling with technical depth | Real decisions from building storage, MVCC, Raft |
| System Design | Requirements-first thinking, quantitative reasoning | Architectural patterns from Chapters 1-17 |
| Coding | Problem decomposition, correctness reasoning | DSA patterns from the database domain |

The common thread: specificity. "I built a database" is forgettable. "I built a BitCask-style storage engine with CRC checksums and crash recovery, then layered MVCC on top for snapshot isolation, then connected it to Raft for replication" is memorable and credible. The database you built is your strongest interview asset — use it.
