/// Chapter 13: Async Networking with Tokio
/// Exercise: Convert a sync echo server to async.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

type SharedState = Arc<Mutex<HashMap<String, String>>>;

/// Handle a single client connection.
async fn handle_client(mut stream: TcpStream, state: SharedState) {
    // TODO:
    // 1. Read up to 1024 bytes from the stream
    // 2. Parse as "GET key" or "SET key value"
    // 3. For GET: lock state, look up key, respond with value or "NULL"
    // 4. For SET: lock state, insert, respond with "OK"
    // 5. Write response back to stream
    todo!("Implement handle_client")
}

/// Run the async server.
async fn run_server(addr: &str) {
    // TODO:
    // 1. Bind TcpListener to addr
    // 2. Create shared state: Arc<Mutex<HashMap>>
    // 3. Loop: accept connections, tokio::spawn handle_client for each
    todo!("Implement run_server")
}

#[tokio::main]
async fn main() {
    println!("=== Chapter 13: Async Networking ===");
    println!("Run `cargo test --bin exercise` to check.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_shared_state() {
        let state: SharedState = Arc::new(Mutex::new(HashMap::new()));
        {
            let mut lock = state.lock().await;
            lock.insert("key".into(), "value".into());
        }
        let lock = state.lock().await;
        assert_eq!(lock.get("key"), Some(&"value".to_string()));
    }

    #[tokio::test]
    async fn test_async_echo() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 1024];
            let n = stream.read(&mut buf).await.unwrap();
            stream.write_all(&buf[..n]).await.unwrap();
        });

        let mut client = TcpStream::connect(addr).await.unwrap();
        client.write_all(b"hello").await.unwrap();
        let mut buf = vec![0u8; 1024];
        let n = client.read(&mut buf).await.unwrap();
        assert_eq!(&buf[..n], b"hello");
    }
}
