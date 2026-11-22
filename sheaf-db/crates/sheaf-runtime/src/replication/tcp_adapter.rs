// tcp_adapter.rs - the loopback TCP replication adapter (NF-05).
//
// Each send writes one length-prefixed frame and waits for an acknowledgement
// byte. Each receive reads one frame and acknowledges it. The frame boundary
// is explicit, so a truncated delivery is an error instead of a partial delta.

use super::frame;
use super::port::{Ack, ReplicationError, ReplicationTransport, Result};
use super::SectionDelta;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};

/// The sending half of a replication link.
pub struct TcpSender {
    stream: TcpStream,
}

impl TcpSender {
    pub fn connect<A: ToSocketAddrs>(addr: A) -> Result<Self> {
        let stream = TcpStream::connect(addr).map_err(|e| ReplicationError::Io(e.to_string()))?;
        Ok(Self { stream })
    }
}

impl ReplicationTransport for TcpSender {
    fn send(&self, delta: &SectionDelta) -> Result<Ack> {
        let mut stream = self.stream.try_clone().map_err(|e| ReplicationError::Io(e.to_string()))?;
        let bytes = frame::encode(delta)?;
        let outer = (bytes.len() as u32).to_le_bytes();
        stream.write_all(&outer).map_err(|e| ReplicationError::Io(e.to_string()))?;
        stream.write_all(&bytes).map_err(|e| ReplicationError::Io(e.to_string()))?;
        stream.flush().map_err(|e| ReplicationError::Io(e.to_string()))?;
        let mut ack = [0u8; 8];
        stream.read_exact(&mut ack).map_err(|e| ReplicationError::Io(e.to_string()))?;
        let sequence = u64::from_le_bytes(ack);
        if sequence != delta.sequence {
            return Err(ReplicationError::Framing("acknowledgement sequence mismatch".into()));
        }
        Ok(Ack { sequence })
    }

    fn receive(&self) -> Result<SectionDelta> {
        Err(ReplicationError::Unsupported("a TCP sender does not receive deltas".into()))
    }
}

/// The receiving half of a replication link.
pub struct TcpReceiver {
    listener: TcpListener,
    stream: Option<TcpStream>,
}

impl TcpReceiver {
    pub fn bind<A: ToSocketAddrs>(addr: A) -> Result<Self> {
        let listener = TcpListener::bind(addr).map_err(|e| ReplicationError::Io(e.to_string()))?;
        Ok(Self { listener, stream: None })
    }

    pub fn local_addr(&self) -> Result<std::net::SocketAddr> {
        self.listener.local_addr().map_err(|e| ReplicationError::Io(e.to_string()))
    }
}

impl ReplicationTransport for TcpReceiver {
    fn send(&self, _delta: &SectionDelta) -> Result<Ack> {
        Err(ReplicationError::Unsupported("a TCP receiver does not send deltas".into()))
    }

    fn receive(&self) -> Result<SectionDelta> {
        // The receiver is single-connection. Bind the accepted stream on the
        // first call and reuse it for the rest of the session.
        let stream = if let Some(stream) = &self.stream {
            stream
        } else {
            return Err(ReplicationError::Io("the receiver has no accepted connection".into()));
        };
        read_one(stream)
    }
}

/// Accept the next connection and bind it to the receiver.
pub fn accept(receiver: &mut TcpReceiver) -> Result<()> {
    let (stream, _) = receiver.listener.accept().map_err(|e| ReplicationError::Io(e.to_string()))?;
    receiver.stream = Some(stream);
    Ok(())
}

fn read_one(mut stream: &TcpStream) -> Result<SectionDelta> {
    let mut length = [0u8; 4];
    stream.read_exact(&mut length).map_err(|e| ReplicationError::Io(e.to_string()))?;
    let size = u32::from_le_bytes(length) as usize;
    if size > frame::MAX_PATCH_BYTES + 24 {
        return Err(ReplicationError::Framing("frame exceeds the receiver limit".into()));
    }
    let mut bytes = vec![0u8; size];
    stream.read_exact(&mut bytes).map_err(|e| ReplicationError::Io(e.to_string()))?;
    let delta = frame::decode(&bytes)?;
    let mut ack = stream.try_clone().map_err(|e| ReplicationError::Io(e.to_string()))?;
    ack.write_all(&delta.sequence.to_le_bytes()).map_err(|e| ReplicationError::Io(e.to_string()))?;
    ack.flush().map_err(|e| ReplicationError::Io(e.to_string()))?;
    Ok(delta)
}
