# Chapter 1: What Is a Database?

Imagine you have a notebook. You write down your friends' phone numbers in it. When you need to call someone, you flip through the pages until you find their name, and there is the number. That notebook is a database. It is a place to store information so you can find it again later.

Every app you use — Instagram, Spotify, your bank — has a database underneath. When you log in, the app looks up your username. When you post a photo, the app stores it. When you search for a song, the app searches through millions of records. All of that is a database doing its job.

In this book, you are going to build one from scratch. Not a toy that only pretends to work. A real database with storage, a query language, transactions, and networking. And you are going to build it in Rust — a programming language designed for exactly this kind of work.

This first chapter starts small. By the end, you will have:

- Your first Rust variables and types (`let`, `mut`, `String`, numbers, booleans)
- A `Value` enum that can hold different kinds of data (like a spreadsheet cell that can be a number OR text)
- A working key-value store backed by Rust's `HashMap`
- A REPL (read-eval-print loop) that accepts SET, GET, DELETE, LIST, and STATS commands from the terminal

Let's begin.

---
