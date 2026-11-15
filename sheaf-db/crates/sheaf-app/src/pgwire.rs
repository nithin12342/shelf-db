//! Localhost trust-only PostgreSQL simple-query adapter. This is not a PostgreSQL server.
//! Startup user and database names are labels, not credentials or storage selectors.
//! Explicit transactions, cancellation, and the extended protocol are not supported.
#[path = "pg_codec.rs"]
mod codec;
#[path = "pg_sql.rs"]
mod sql;

use crate::server::{self, ServerOptions};
use crate::service::Application;
use codec::{Result, WireError};
use std::collections::HashSet;
use std::io::{self, Write};
use std::net::{TcpListener, TcpStream};

const VERSION_3: u32 = 196_608;
const SSL_REQUEST: u32 = 80_877_103;
const GSS_REQUEST: u32 = 80_877_104;
const CANCEL_REQUEST: u32 = 80_877_102;

pub fn serve(listener: TcpListener, application: &Application, options: ServerOptions) -> io::Result<()> {
    server::run(listener, options, |stream| {
        if let Err(error) = session(stream, application, options.optimize) {
            // Protocol failures end this connection. No buffered command runs after failure.
            let _ = codec::error(stream, &error, true);
            return Err(io::Error::new(io::ErrorKind::InvalidData, error.message));
        }
        Ok(())
    })
}

fn session(stream: &mut TcpStream, application: &Application, optimize: bool) -> Result<()> {
    if !handshake(stream)? { return Ok(()); }
    while let Some((kind, body)) = codec::message(stream)? {
        match kind {
            b'X' => {
                if !body.is_empty() { return Err(WireError::protocol("Terminate requires an empty body")); }
                return Ok(());
            }
            b'Q' => {
                let text = codec::query_text(&body)?;
                match sql::execute(application, text, optimize) {
                    Ok(Some(bytes)) => stream.write_all(&bytes)?,
                    Ok(None) => codec::send(stream, b'I', &[])?,
                    Err(error) => codec::error(stream, &error, false)?,
                }
                codec::ready(stream)?;
            }
            b'P' | b'B' | b'D' | b'E' | b'C' | b'H' | b'S' | b'F' | b'd' | b'c' | b'f' => {
                return Err(WireError::new("0A000",
                    "extended protocol, COPY, and function calls are not supported. Use simple Query in autocommit. Connection closed"));
            }
            _ => return Err(WireError::protocol("unknown frontend message type")),
        }
    }
    Ok(())
}

fn handshake(stream: &mut TcpStream) -> Result<bool> {
    let mut negotiated = HashSet::new();
    loop {
        let Some(body) = codec::startup(stream)? else { return Ok(false); };
        // The startup decoder requires at least four body bytes.
        let code = u32::from_be_bytes(body[..4].try_into().expect("bounded startup code"));
        match code {
            SSL_REQUEST | GSS_REQUEST => {
                if body.len() != 4 || !negotiated.insert(code) {
                    return Err(WireError::protocol("invalid or repeated encryption request"));
                }
                // This adapter has no transport encryption. A client can stop after N.
                stream.write_all(b"N")?;
                stream.flush()?;
            }
            CANCEL_REQUEST => {
                if body.len() != 12 { return Err(WireError::protocol("CancelRequest requires 16 bytes")); }
                return Err(WireError::new("0A000", "cancellation is not supported. No command was cancelled"));
            }
            VERSION_3 => {
                startup_parameters(&body[4..])?;
                codec::send(stream, b'R', &0u32.to_be_bytes())?; // Local trust, no password exchange.
                for (name, value) in [
                    ("server_version", "14.0 (SheafDB simple-query adapter)"),
                    ("server_encoding", "UTF8"),
                    ("client_encoding", "UTF8"),
                    ("DateStyle", "ISO, MDY"),
                    ("TimeZone", "UTC"),
                    ("integer_datetimes", "on"),
                    ("standard_conforming_strings", "on"),
                    ("application_name", "sheaf-app"),
                ] { codec::parameter(stream, name, value)?; }
                // No BackendKeyData is sent because this adapter cannot honor cancellation.
                codec::ready(stream)?;
                return Ok(true);
            }
            _ => return Err(WireError::new("0A000", "only PostgreSQL protocol version 3.0 is supported")),
        }
    }
}

fn startup_parameters(mut bytes: &[u8]) -> Result<()> {
    let mut names = HashSet::new();
    let mut user = false;
    loop {
        if bytes == [0] { break; }
        if bytes.is_empty() { return Err(WireError::protocol("startup parameters need a final zero byte")); }
        let name = take_string(&mut bytes)?;
        if name.is_empty() { return Err(WireError::protocol("startup packet contains trailing bytes")); }
        let value = take_string(&mut bytes)?;
        if names.len() >= 64 || !names.insert(name) {
            return Err(WireError::protocol("duplicate startup parameter or more than 64 parameters"));
        }
        match name {
            "user" => { user = !value.is_empty(); }
            "database" | "application_name" | "fallback_application_name" => {}
            "client_encoding" if value.eq_ignore_ascii_case("UTF8") || value.eq_ignore_ascii_case("UTF-8") => {}
            "options" if value.is_empty() => {}
            "replication" if ["false", "off", "0"].iter().any(|v| value.eq_ignore_ascii_case(v)) => {}
            _ => return Err(WireError::new("0A000", format!("unsupported startup parameter: {name}"))),
        }
    }
    if !user { return Err(WireError::protocol("startup requires a nonempty user label")); }
    Ok(())
}

fn take_string<'a>(bytes: &mut &'a [u8]) -> Result<&'a str> {
    let end = bytes.iter().position(|byte| *byte == 0)
        .ok_or_else(|| WireError::protocol("unterminated startup parameter"))?;
    let text = std::str::from_utf8(&bytes[..end])
        .map_err(|_| WireError::new("22021", "startup parameters must use UTF-8"))?;
    *bytes = &bytes[end + 1..];
    Ok(text)
}
