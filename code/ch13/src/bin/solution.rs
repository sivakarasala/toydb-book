/// Chapter 13: Async Networking — SOLUTION

use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

type SharedState = Arc<Mutex<HashMap<String, String>>>;

async fn handle_client(mut stream: TcpStream, state: SharedState) {
    let mut buf = vec![0u8; 1024];
    let n = match stream.read(&mut buf).await {
        Ok(0) | Err(_) => return,
        Ok(n) => n,
    };
    let cmd = String::from_utf8_lossy(&buf[..n]).to_string();
    let parts: Vec<&str> = cmd.trim().splitn(3, ' ').collect();

    let response = match parts[0] {
        "GET" if parts.len() == 2 => {
            let lock = state.lock().await;
            lock.get(parts[1]).cloned().unwrap_or_else(|| "NULL".into())
        }
        "SET" if parts.len() == 3 => {
            let mut lock = state.lock().await;
            lock.insert(parts[1].into(), parts[2].into());
            "OK".into()
        }
        _ => "ERROR".into(),
    };
    let _ = stream.write_all(response.as_bytes()).await;
}

#[tokio::main]
async fn main() {
    println!("=== Chapter 13: Async Server — Solution ===");
    let listener = TcpListener::bind("127.0.0.1:7878").await.unwrap();
    let state: SharedState = Arc::new(Mutex::new(HashMap::new()));
    println!("Listening on 127.0.0.1:7878");

    loop {
        let (stream, addr) = listener.accept().await.unwrap();
        println!("Client connected: {addr}");
        let state = state.clone();
        tokio::spawn(handle_client(stream, state));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_shared_state() {
        let state: SharedState = Arc::new(Mutex::new(HashMap::new()));
        { state.lock().await.insert("key".into(), "value".into()); }
        assert_eq!(state.lock().await.get("key"), Some(&"value".to_string()));
    }

    #[tokio::test]
    async fn test_async_echo() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut s, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 1024];
            let n = s.read(&mut buf).await.unwrap();
            s.write_all(&buf[..n]).await.unwrap();
        });
        let mut client = TcpStream::connect(addr).await.unwrap();
        client.write_all(b"hello").await.unwrap();
        let mut buf = vec![0u8; 1024];
        let n = client.read(&mut buf).await.unwrap();
        assert_eq!(&buf[..n], b"hello");
    }

    #[tokio::test]
    async fn test_handle_set_get() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let state: SharedState = Arc::new(Mutex::new(HashMap::new()));
        let s1 = state.clone();

        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            handle_client(stream, s1).await;
        });

        let mut client = TcpStream::connect(addr).await.unwrap();
        client.write_all(b"SET name ToyDB").await.unwrap();
        let mut buf = vec![0u8; 1024];
        let n = client.read(&mut buf).await.unwrap();
        assert_eq!(&buf[..n], b"OK");

        assert_eq!(state.lock().await.get("name"), Some(&"ToyDB".to_string()));
    }
}
