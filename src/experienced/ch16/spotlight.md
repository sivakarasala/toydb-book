## Spotlight: Ownership & Persistence

Every chapter has one spotlight concept. This chapter's spotlight is **ownership and persistence** — how Rust's ownership system governs file handles, buffers, and the lifecycle of persistent state.

### File handles are resources

In languages with garbage collectors, a file handle might stay open long after you stop using it — the GC cleans it up "eventually." In Rust, file handles are owned values. When the owner goes out of scope, the file is closed. This is deterministic resource management, enforced by the compiler:

```rust
use std::fs::File;
use std::io::Write;

fn write_term(path: &str, term: u64) -> std::io::Result<()> {
    let mut file = File::create(path)?;  // file is opened here
    file.write_all(&term.to_le_bytes())?;
    file.sync_all()?;
    Ok(())
    // file is CLOSED here — Drop runs automatically
}
```

There is no `file.close()` call. The `File` struct implements `Drop`, and when `file` goes out of scope at the end of the function, `Drop::drop()` runs, which closes the file descriptor. This is Rust's RAII pattern — Resource Acquisition Is Initialization. You acquire the resource (open the file) when you initialize the variable, and release it (close the file) when the variable is dropped.

### Ownership determines who controls the file

When building a WAL, someone must own the file handle. If the `RaftNode` owns it, the file lives as long as the node. If you put it in a separate `WalWriter` struct, the writer controls the file's lifetime. Ownership is not just a compiler concept — it is a design decision about responsibility:

```rust,ignore
struct WalWriter {
    file: File,        // WalWriter owns the file handle
    path: PathBuf,     // WalWriter knows where the file lives
    offset: u64,       // WalWriter tracks the write position
}

struct RaftNode {
    wal: WalWriter,    // RaftNode owns the WalWriter
    // ...
}
```

When `RaftNode` is dropped, it drops `WalWriter`, which drops `File`, which closes the file descriptor. The ownership chain is explicit, deterministic, and enforced by the compiler. No finalizers, no `try-finally`, no "remember to close the file."

### Mutable references and exclusive file access

Rust's borrow checker guarantees that only one mutable reference exists at a time. For file I/O, this maps directly to exclusive write access:

```rust,ignore
impl WalWriter {
    // Only one caller can append at a time — &mut self guarantees this
    fn append(&mut self, entry: &LogEntry) -> std::io::Result<()> {
        let bytes = entry.serialize();
        self.file.write_all(&bytes)?;
        self.offset += bytes.len() as u64;
        Ok(())
    }
}
```

The `&mut self` signature means you need exclusive access to the `WalWriter` to append. If you tried to share the writer across threads without synchronization, the compiler would refuse. This is the same guarantee that `flock()` provides at the OS level, but enforced at compile time.

### BufWriter and flush semantics

Writing to disk byte-by-byte is slow — each `write()` is a system call. `BufWriter` batches writes into a user-space buffer and flushes them in larger chunks:

```rust
use std::io::BufWriter;

let file = File::create("raft.wal")?;
let mut writer = BufWriter::new(file);

// These go to the buffer, not the disk
writer.write_all(&entry1_bytes)?;
writer.write_all(&entry2_bytes)?;

// This sends the buffer to the OS
writer.flush()?;

// This forces the OS to write to the physical disk
writer.get_ref().sync_all()?;
```

There is a critical distinction: `flush()` moves data from your process's buffer to the OS kernel's buffer. `sync_all()` forces the kernel to write its buffer to the physical disk (the `fsync` system call). For durability, you need both.

> **Coming from JS/Python/Go?**
>
> | Concept | JavaScript | Python | Go | Rust |
> |---------|-----------|--------|-----|------|
> | Open file | `fs.openSync()` | `open()` | `os.Create()` | `File::create()` |
> | Close file | `fs.closeSync(fd)` | `f.close()` | `f.Close()` | Automatic (Drop) |
> | Sync to disk | `fs.fsyncSync(fd)` | `os.fsync(fd)` | `f.Sync()` | `file.sync_all()` |
> | Buffered write | `fs.writeFileSync()` | `io.BufferedWriter` | `bufio.Writer` | `BufWriter::new()` |
> | Exclusive access | Manual locking | Manual locking | `sync.Mutex` | `&mut self` (compile-time) |
> | Resource cleanup | Manual or `finally` | `with` statement | `defer f.Close()` | `Drop` trait (automatic) |
>
> The key difference: in Go, forgetting `defer f.Close()` is a bug that compiles fine and leaks file descriptors. In Python, forgetting `with` is a bug that might or might not cause problems. In Rust, the file is always closed when the owner goes out of scope — you cannot forget, because the compiler handles it. And Rust's `&mut self` replaces mutexes for single-threaded file access — the borrow checker proves at compile time that no two writers exist simultaneously.

---
