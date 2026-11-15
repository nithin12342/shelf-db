// ingress.rs - the Rust network ingress tier, spec Pillar 4.
//
// The spec assigns ingress to native Rust. This module provides a TCP server
// with length-prefixed framing and a handler port (trait). The concrete server
// is one adapter. A gRPC or Arrow Flight adapter can implement the same port
// later without changing the framing or the handler contract.
//
// Note: loopback TCP is correct here because this is the external network
// boundary. Internal query stages use the in-process nerve router instead
// (see nerve.rs), which never opens a socket.

use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};

/// Largest accepted frame payload. A larger length prefix is rejected so a
/// malformed or hostile header cannot force a huge allocation.
pub const MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;

/// The ingress handler port. An adapter turns request bytes into response
/// bytes. The server depends on this trait, not on a concrete query engine.
pub trait RequestHandler {
    /// Handle one request frame and return the response frame payload.
    fn handle(&self, request: &[u8]) -> Vec<u8>;
}

/// A handler that echoes the request. Used by tests and as a minimal adapter.
pub struct EchoHandler;

impl RequestHandler for EchoHandler {
    fn handle(&self, request: &[u8]) -> Vec<u8> {
        request.to_vec()
    }
}

/// Write one length-prefixed frame: a 4-byte little-endian length, then the
/// payload. Returns an error when the payload exceeds the frame limit.
pub fn write_frame(stream: &mut TcpStream, payload: &[u8]) -> io::Result<()> {
    if payload.len() > MAX_FRAME_BYTES {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "frame too large"));
    }
    let len = (payload.len() as u32).to_le_bytes();
    stream.write_all(&len)?;
    stream.write_all(payload)?;
    stream.flush()
}

/// Read one length-prefixed frame. Returns the payload bytes. A length over
/// the limit, or a closed connection mid-frame, is an error.
pub fn read_frame(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf)?;
    let len = u32::from_le_bytes(len_buf) as usize;
    if len > MAX_FRAME_BYTES {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "frame too large"));
    }
    let mut payload = vec![0u8; len];
    stream.read_exact(&mut payload)?;
    Ok(payload)
}

/// A bound TCP ingress server. It accepts connections and dispatches each
/// request frame to the handler port.
pub struct IngressServer {
    listener: TcpListener,
}

impl IngressServer {
    /// Bind to an address. Use port 0 to let the OS pick a free port, which
    /// makes tests deterministic without a fixed port reservation.
    pub fn bind<A: ToSocketAddrs>(addr: A) -> io::Result<Self> {
        Ok(Self { listener: TcpListener::bind(addr)? })
    }

    /// The local address the server is bound to.
    pub fn local_addr(&self) -> io::Result<std::net::SocketAddr> {
        self.listener.local_addr()
    }

    /// Accept one connection, read one request frame, call the handler, and
    /// write one response frame. This is the single-shot unit used by tests
    /// and by a per-connection worker loop.
    pub fn serve_one<H: RequestHandler>(&self, handler: &H) -> io::Result<()> {
        let (mut stream, _peer) = self.listener.accept()?;
        let request = read_frame(&mut stream)?;
        let response = handler.handle(&request);
        write_frame(&mut stream, &response)
    }
}

/// Client helper: connect, send one request frame, and read one response
/// frame. Used by tests and by simple callers of the ingress boundary.
pub fn round_trip<A: ToSocketAddrs>(addr: A, request: &[u8]) -> io::Result<Vec<u8>> {
    let mut stream = TcpStream::connect(addr)?;
    write_frame(&mut stream, request)?;
    read_frame(&mut stream)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn echo_round_trip_over_loopback() {
        let server = IngressServer::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        let handle = thread::spawn(move || server.serve_one(&EchoHandler));
        let payload = b"sheaf ingress".to_vec();
        let response = round_trip(addr, &payload).unwrap();
        assert_eq!(response, payload);
        handle.join().unwrap().unwrap();
    }

    #[test]
    fn handler_transforms_request() {
        struct Upper;
        impl RequestHandler for Upper {
            fn handle(&self, request: &[u8]) -> Vec<u8> {
                request.to_ascii_uppercase()
            }
        }
        let server = IngressServer::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        let handle = thread::spawn(move || server.serve_one(&Upper));
        let response = round_trip(addr, b"abc").unwrap();
        assert_eq!(response, b"ABC");
        handle.join().unwrap().unwrap();
    }

    #[test]
    fn empty_frame_round_trips() {
        let server = IngressServer::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        let handle = thread::spawn(move || server.serve_one(&EchoHandler));
        let response = round_trip(addr, b"").unwrap();
        assert!(response.is_empty());
        handle.join().unwrap().unwrap();
    }

    #[test]
    fn oversized_write_is_rejected() {
        let server = IngressServer::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        let mut client = TcpStream::connect(addr).unwrap();
        let big = vec![0u8; MAX_FRAME_BYTES + 1];
        assert!(write_frame(&mut client, &big).is_err());
    }
}