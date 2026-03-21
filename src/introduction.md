# Introduction

Welcome to **Learn Rust by Building a Database** — a hands-on guide that teaches you Rust, data structures & algorithms, and system design by building a distributed SQL database from scratch.

## Why a Database?

Databases are the backbone of every application, yet most developers treat them as black boxes. By building one yourself, you'll understand how storage engines persist data, how SQL gets parsed into execution plans, how transactions maintain consistency, and how distributed consensus keeps replicas in sync. Along the way, you'll master Rust's type system, concurrency primitives, and async runtime.

## The Triple Goal

1. **Production Rust Skills** — Traits, generics, enums, async/await, Arc/Mutex, and more — all learned through code that solves real problems.
2. **DSA Fluency** — B-trees, hash tables, AST traversal, Raft consensus, and 16 Deep Dive chapters building data structures from scratch.
3. **System Design Interview Readiness** — Every chapter includes a System Design Corner connecting your implementation to interview-style architecture discussions.

## Two Learning Tracks

| Track | Who It's For | Where to Start |
|-------|-------------|----------------|
| **Beginner** | Never programmed before | Part 0: Programming Fundamentals |
| **Experienced** | Know another language, learning Rust | Chapter 1: What Is a Database? |

## How Each Chapter Works

- **Spotlight** — One Rust concept taught in depth
- **Building the Feature** — Exercises with progressive hints and full solutions
- **Rust Gym** — 2-3 drills on the spotlight concept
- **DSA in Context** — Links your code to interview patterns
- **System Design Corner** — Architecture decisions as interview talking points

## What You'll Build

By the end of this book, you'll have a distributed SQL database that can:

- Store and retrieve data with pluggable storage engines
- Execute SQL queries (SELECT, INSERT, UPDATE, DELETE, JOINs)
- Maintain transaction isolation with MVCC
- Replicate data across nodes with Raft consensus
- Handle leader election and failover

Let's build a database.
