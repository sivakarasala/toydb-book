## Spotlight: Ownership & Persistence

Every chapter has one **spotlight concept**. This chapter's spotlight is **ownership and persistence** -- how Rust's ownership model naturally maps to questions about file handles, resource lifetimes, and who is responsible for what.

### File handles are owned resources

In languages with garbage collectors (JavaScript, Python, Java), a file handle might stay open long after you stop using it. The garbage collector "eventually" cleans it up, but "eventually" might be too late -- file descriptors are limited, and leaving files open can cause data corruption.

In Rust, a file handle is an owned value. When the owner goes out of scope, the file is closed. This happens immediately and deterministically:

```rust
use std::fs::File;
use std::io::Write;

fn save_term(path: &str, term: u64) -> std::io::Result<()> {
    let mut file = File::create(path)?;   // file opened here
    file.write_all(&term.to_le_bytes())?;
    file.sync_all()?;
    Ok(())
    // file closed here automatically -- Drop trait runs
}
```

There is no `file.close()` call. When `file` goes out of scope at the end of the function, Rust calls its `Drop` implementation, which closes the file descriptor. This is called **RAII** (Resource Acquisition Is Initialization) -- you acquire the resource when you create the variable, and release it when the variable is dropped.

> **Programming Concept: The Drop Trait**
>
> When a value goes out of scope in Rust, the compiler automatically calls `drop()` on it. The `File` type implements `Drop` to close the file descriptor. You can implement `Drop` for your own types too -- for example, to flush a buffer or release a lock. This is how Rust guarantees that resources are always cleaned up, even if your function returns early due to an error (the `?` operator).

### Ownership determines responsibility

When building a WAL (Write-Ahead Log), someone must own the file handle. This is not just a compiler concept -- it is a design decision about responsibility:

```rust,ignore
/// The WAL writer owns the file handle.
/// When the writer is dropped, the file is closed.
struct WalWriter {
    file: File,         // WalWriter owns this
    path: PathBuf,      // WalWriter knows where the file lives
    entry_count: u64,   // WalWriter tracks its position
}

/// The RaftNode owns the WalWriter.
/// When the node is dropped, the writer is dropped, which closes the file.
struct RaftNode {
    wal: WalWriter,     // ownership chain: Node -> Writer -> File
    // ... other fields
}
```

The ownership chain is explicit: `RaftNode` owns `WalWriter`, which owns `File`. When `RaftNode` is dropped, `WalWriter` is dropped, which drops `File`, which closes the file descriptor. No manual cleanup needed, no finalizers, no "remember to close the file."

### Mutable references and exclusive file access

Rust's borrow checker guarantees that only one mutable reference to a value exists at a time. For file I/O, this naturally enforces exclusive write access:

```rust,ignore
impl WalWriter {
    // &mut self means: only one caller can write at a time.
    // The compiler enforces this!
    fn append(&mut self, entry: &LogEntry) -> std::io::Result<()> {
        let bytes = entry.serialize();
        self.file.write_all(&bytes)?;
        self.entry_count += 1;
        Ok(())
    }
}
```

The `&mut self` signature means you need exclusive access to the writer. If two threads tried to write simultaneously, the compiler would refuse to compile the code. This is the same guarantee that file locks provide at the operating system level, but enforced at compile time.

### BufWriter: batching writes for performance

Writing to disk byte-by-byte is slow because each `write()` call is a system call (a round trip to the operating system kernel). `BufWriter` collects writes in a memory buffer and sends them in batches:

```rust,ignore
use std::io::BufWriter;

let file = File::create("raft.wal")?;
let mut writer = BufWriter::new(file);

// These go to the buffer, not the disk
writer.write_all(&entry1_bytes)?;
writer.write_all(&entry2_bytes)?;

// This sends the buffer to the OS kernel
writer.flush()?;

// This forces the OS to write to the PHYSICAL DISK
writer.get_ref().sync_all()?;
```

There is an important distinction:
- **`flush()`** moves data from your program's buffer to the operating system's buffer. The OS "has" the data but has not written it to disk yet.
- **`sync_all()`** forces the OS to write its buffer to the actual physical disk. This is the `fsync` system call. After it returns, the data survives a power loss.

For durability, you need both: flush your buffer, then sync to disk.

> **What Just Happened?**
>
> We explored how Rust's ownership model maps to file I/O:
> - **Ownership** determines who holds the file handle and when it is closed
> - **Mutable references** (`&mut self`) prevent concurrent writes at compile time
> - **RAII/Drop** ensures files are closed even when errors occur
> - **BufWriter** batches writes for performance
> - **`sync_all()`** guarantees data reaches the physical disk

---
