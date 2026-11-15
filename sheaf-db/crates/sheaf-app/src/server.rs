//! Local framed ingress. One thread owns the application and serves clients serially.
use crate::model::MAX_REQUEST_BYTES;
use crate::service::{Application, Request};
use sheaf_runtime::ingress::{write_frame, RequestHandler};
use std::io::{self, Read};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::time::Duration;

#[derive(Clone, Copy, Debug)]
pub struct ServerOptions {
    /// Count accepted connections, including rejected connections, then stop.
    pub connections: Option<usize>,
    pub timeout: Duration,
    /// False forces baseline execution. True respects each framed request.
    pub optimize: bool,
}

impl Default for ServerOptions {
    fn default() -> Self {
        Self { connections: None, timeout: Duration::from_secs(30), optimize: true }
    }
}

pub fn bind(address: SocketAddr) -> io::Result<TcpListener> {
    require_loopback(address)?;
    TcpListener::bind(address)
}

fn require_loopback(address: SocketAddr) -> io::Result<()> {
    if !address.ip().is_loopback() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput,
            "only a numeric loopback address is allowed, for example 127.0.0.1:7400"));
    }
    Ok(())
}

/// Both adapters use this serial accept loop. The database never crosses threads.
pub(crate) fn run(
    listener: TcpListener,
    options: ServerOptions,
    mut connection: impl FnMut(&mut TcpStream) -> io::Result<()>,
) -> io::Result<()> {
    require_loopback(listener.local_addr()?)?;
    if options.connections == Some(0) || options.timeout.is_zero() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput,
            "connection count and timeout must be greater than zero"));
    }
    let mut accepted = 0usize;
    while options.connections.is_none_or(|limit| accepted < limit) {
        let (mut stream, peer) = match listener.accept() {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            result => result?,
        };
        accepted = accepted.saturating_add(1);
        let result = (|| {
            require_loopback(peer)?;
            stream.set_read_timeout(Some(options.timeout))?;
            stream.set_write_timeout(Some(options.timeout))?;
            stream.set_nodelay(true)?;
            connection(&mut stream)
        })();
        // A bad connection cannot stop the listener or change its count limit.
        if let Err(error) = result { eprintln!("connection {peer}: {error}"); }
    }
    Ok(())
}

/// None means a clean disconnect between packets, not a truncated packet.
pub(crate) fn read_header<const N: usize>(stream: &mut TcpStream) -> io::Result<Option<[u8; N]>> {
    let mut header = [0u8; N];
    loop {
        match stream.read(&mut header[..1]) {
            Ok(0) => return Ok(None),
            Ok(_) => break,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
    stream.read_exact(&mut header[1..])?;
    Ok(Some(header))
}

fn read_request(stream: &mut TcpStream) -> io::Result<Option<Vec<u8>>> {
    let Some(header) = read_header::<4>(stream)? else { return Ok(None); };
    let length = u32::from_le_bytes(header) as usize;
    // The runtime reader allows 16 MiB. Apply the application limit before allocation.
    if length > MAX_REQUEST_BYTES {
        return Err(io::Error::new(io::ErrorKind::InvalidData,
            "request exceeds 65536 bytes"));
    }
    let mut request = vec![0u8; length];
    stream.read_exact(&mut request)?;
    Ok(Some(request))
}

pub fn serve(listener: TcpListener, application: &Application, options: ServerOptions) -> io::Result<()> {
    run(listener, options, |stream| {
        while let Some(mut request) = read_request(stream)? {
            if !options.optimize {
                if let Ok(mut decoded) = serde_json::from_slice::<Request>(&request) {
                    decoded.optimize = false;
                    request = serde_json::to_vec(&decoded)
                        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
                }
            }
            let response = application.handle(&request);
            // Runtime framing is four little-endian bytes followed by the payload.
            u32::try_from(response.len()).map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidData, "response length exceeds 32 bits")
            })?;
            write_frame(stream, &response)?;
        }
        Ok(())
    })
}
