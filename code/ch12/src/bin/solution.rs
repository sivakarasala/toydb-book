/// Chapter 12: Client-Server — SOLUTION

use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};

#[derive(Debug, Serialize, Deserialize)]
pub enum Request { Ping, Query(String) }

#[derive(Debug, Serialize, Deserialize)]
pub enum Response { Pong, Result(String), Error(String) }

pub fn frame_write(stream: &mut impl Write, data: &[u8]) -> io::Result<()> {
    let len = (data.len() as u32).to_be_bytes();
    stream.write_all(&len)?;
    stream.write_all(data)?;
    stream.flush()
}

pub fn frame_read(stream: &mut impl Read) -> io::Result<Vec<u8>> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf)?;
    let len = u32::from_be_bytes(len_buf) as usize;
    let mut data = vec![0u8; len];
    stream.read_exact(&mut data)?;
    Ok(data)
}

pub fn send_request(stream: &mut TcpStream, req: &Request) -> io::Result<()> {
    let bytes = bincode::serialize(req).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    frame_write(stream, &bytes)
}

pub fn read_response(stream: &mut TcpStream) -> io::Result<Response> {
    let data = frame_read(stream)?;
    bincode::deserialize(&data).map_err(|e| io::Error::new(io::ErrorKind::Other, e))
}

fn main() {
    println!("=== Chapter 12: Client-Server — Solution ===");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    println!("Server listening on {addr}");

    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let data = frame_read(&mut stream).unwrap();
        let req: Request = bincode::deserialize(&data).unwrap();
        println!("Server got: {req:?}");
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
    println!("Client got: {resp:?}");
    server.join().unwrap();
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
        let result = frame_read(&mut io::Cursor::new(buf)).unwrap();
        assert_eq!(result, data);
    }

    #[test]
    fn test_ping_pong() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            let data = frame_read(&mut s).unwrap();
            let req: Request = bincode::deserialize(&data).unwrap();
            let resp = match req { Request::Ping => Response::Pong, Request::Query(q) => Response::Result(format!("OK: {q}")) };
            frame_write(&mut s, &bincode::serialize(&resp).unwrap()).unwrap();
        });
        let mut client = TcpStream::connect(addr).unwrap();
        send_request(&mut client, &Request::Ping).unwrap();
        match read_response(&mut client).unwrap() { Response::Pong => {} _ => panic!() }
        server.join().unwrap();
    }
}
