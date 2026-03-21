/// Chapter 12: Client-Server Protocol
/// Exercise: Build TCP client-server with length-prefixed framing.

use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};

#[derive(Debug, Serialize, Deserialize)]
pub enum Request {
    Ping,
    Query(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Response {
    Pong,
    Result(String),
    Error(String),
}

/// Write a length-prefixed frame: [4-byte big-endian length][payload]
pub fn frame_write(stream: &mut impl Write, data: &[u8]) -> io::Result<()> {
    // TODO: Write data.len() as 4-byte big-endian, then write data, then flush
    todo!("Implement frame_write")
}

/// Read a length-prefixed frame.
pub fn frame_read(stream: &mut impl Read) -> io::Result<Vec<u8>> {
    // TODO: Read 4 bytes for length, then read that many bytes
    todo!("Implement frame_read")
}

/// Send a request over a stream.
pub fn send_request(stream: &mut TcpStream, req: &Request) -> io::Result<()> {
    // TODO: Serialize req with bincode, then frame_write
    todo!("Implement send_request")
}

/// Read a response from a stream.
pub fn read_response(stream: &mut TcpStream) -> io::Result<Response> {
    // TODO: frame_read, then deserialize with bincode
    todo!("Implement read_response")
}

fn main() {
    println!("=== Chapter 12: Client-Server ===");
    println!("Run `cargo test --bin exercise` to check.");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_framing_round_trip() {
        let data = b"hello, world!";
        let mut buf = Vec::new();
        frame_write(&mut buf, data).unwrap();
        let mut cursor = io::Cursor::new(buf);
        let result = frame_read(&mut cursor).unwrap();
        assert_eq!(result, data);
    }

    #[test]
    fn test_ping_pong() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let data = frame_read(&mut stream).unwrap();
            let req: Request = bincode::deserialize(&data).unwrap();
            let resp = match req {
                Request::Ping => Response::Pong,
                Request::Query(q) => Response::Result(format!("OK: {q}")),
            };
            let resp_bytes = bincode::serialize(&resp).unwrap();
            frame_write(&mut stream, &resp_bytes).unwrap();
        });

        let mut client = TcpStream::connect(addr).unwrap();
        send_request(&mut client, &Request::Ping).unwrap();
        let resp = read_response(&mut client).unwrap();
        match resp {
            Response::Pong => {}
            _ => panic!("Expected Pong"),
        }
        server.join().unwrap();
    }
}
