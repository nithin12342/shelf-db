//! Bounded PostgreSQL protocol version 3 packet encoding and decoding.
use crate::model::{QueryOutput, Value, MAX_REQUEST_BYTES};
use crate::server::read_header;
use std::io::{self, Read, Write};
use std::net::TcpStream;

const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
pub(super) type Result<T> = std::result::Result<T, WireError>;

#[derive(Debug)]
pub(super) struct WireError {
    pub code: &'static str,
    pub message: String,
}

impl WireError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }
    pub fn protocol(message: impl Into<String>) -> Self { Self::new("08P01", message) }
}

impl From<io::Error> for WireError {
    fn from(error: io::Error) -> Self {
        let code = match error.kind() {
            io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock => "57014",
            io::ErrorKind::UnexpectedEof | io::ErrorKind::InvalidData => "08P01",
            _ => "08006",
        };
        Self::new(code, error.to_string())
    }
}

fn read_body(stream: &mut TcpStream, header: [u8; 4], minimum: i32) -> Result<Vec<u8>> {
    let length = i32::from_be_bytes(header);
    if length < minimum || length as usize > MAX_REQUEST_BYTES {
        return Err(WireError::protocol("packet length must fit within 65536 bytes"));
    }
    // The signed length includes itself. No allocation occurs before the bound.
    let mut body = vec![0u8; length as usize - 4];
    stream.read_exact(&mut body)?;
    Ok(body)
}

pub(super) fn startup(stream: &mut TcpStream) -> Result<Option<Vec<u8>>> {
    read_header::<4>(stream)?.map(|header| read_body(stream, header, 8)).transpose()
}

pub(super) fn message(stream: &mut TcpStream) -> Result<Option<(u8, Vec<u8>)>> {
    let Some([kind]) = read_header::<1>(stream)? else { return Ok(None); };
    let mut header = [0u8; 4];
    stream.read_exact(&mut header)?;
    Ok(Some((kind, read_body(stream, header, 4)?)))
}

pub(super) fn query_text(body: &[u8]) -> Result<&str> {
    let Some((&0, text)) = body.split_last() else {
        return Err(WireError::protocol("Query requires a trailing zero byte"));
    };
    if text.contains(&0) { return Err(WireError::protocol("Query contains an embedded zero byte")); }
    std::str::from_utf8(text).map_err(|_| WireError::new("22021", "Query must use UTF-8"))
}

fn length32(length: usize) -> Result<i32> {
    length.checked_add(4).and_then(|n| i32::try_from(n).ok())
        .ok_or_else(|| WireError::new("54000", "output packet length exceeds signed 32 bits"))
}

pub(super) fn send(stream: &mut TcpStream, kind: u8, body: &[u8]) -> Result<()> {
    let length = length32(body.len())?;
    stream.write_all(&[kind])?;
    stream.write_all(&length.to_be_bytes())?;
    stream.write_all(body)?;
    Ok(())
}

pub(super) fn ready(stream: &mut TcpStream) -> Result<()> {
    send(stream, b'Z', b"I")?;
    stream.flush()?;
    Ok(())
}

pub(super) fn error(stream: &mut TcpStream, error: &WireError, fatal: bool) -> Result<()> {
    let mut body = Vec::new();
    body.push(b'S');
    cstring(&mut body, if fatal { "FATAL" } else { "ERROR" })?;
    body.push(b'V');
    cstring(&mut body, if fatal { "FATAL" } else { "ERROR" })?;
    body.push(b'C');
    cstring(&mut body, error.code)?;
    body.push(b'M');
    // Error text is a protocol string, never a packet delimiter.
    cstring(&mut body, &error.message.replace('\0', "?"))?;
    body.push(0);
    send(stream, b'E', &body)?;
    stream.flush()?;
    Ok(())
}

pub(super) fn parameter(stream: &mut TcpStream, name: &str, value: &str) -> Result<()> {
    let mut body = Vec::new();
    cstring(&mut body, name)?;
    cstring(&mut body, value)?;
    send(stream, b'S', &body)
}

fn extend(buffer: &mut Vec<u8>, bytes: &[u8]) -> Result<()> {
    if buffer.len().checked_add(bytes.len()).is_none_or(|n| n > MAX_RESPONSE_BYTES) {
        return Err(WireError::new("54000", "response exceeds 4 MiB. Use LIMIT"));
    }
    buffer.extend_from_slice(bytes);
    Ok(())
}

fn cstring(buffer: &mut Vec<u8>, text: &str) -> Result<()> {
    if text.as_bytes().contains(&0) {
        return Err(WireError::new("22021", "PostgreSQL text cannot contain a zero byte"));
    }
    extend(buffer, text.as_bytes())?;
    extend(buffer, &[0])
}

fn packet(buffer: &mut Vec<u8>, kind: u8, body: &[u8]) -> Result<()> {
    let length = length32(body.len())?;
    extend(buffer, &[kind])?;
    extend(buffer, &length.to_be_bytes())?;
    extend(buffer, body)
}

fn column_count(length: usize) -> Result<i16> {
    i16::try_from(length).map_err(|_| WireError::new("54000", "too many result columns"))
}

fn description(output: &QueryOutput) -> Result<Vec<u8>> {
    let mut body = column_count(output.columns.len())?.to_be_bytes().to_vec();
    for name in &output.columns {
        cstring(&mut body, name)?;
        extend(&mut body, &0u32.to_be_bytes())?; // No source table identifier.
        extend(&mut body, &0i16.to_be_bytes())?; // No source attribute number.
        extend(&mut body, &25u32.to_be_bytes())?; // Every result type is text.
        extend(&mut body, &(-1i16).to_be_bytes())?;
        extend(&mut body, &(-1i32).to_be_bytes())?;
        extend(&mut body, &0i16.to_be_bytes())?; // Text, not binary, format.
    }
    Ok(body)
}

/// Encode the whole response before writing, so an encoding error emits no partial rows.
pub(super) fn result(output: &QueryOutput, tag: &str) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    if !output.columns.is_empty() { packet(&mut bytes, b'T', &description(output)?)?; }
    for row in &output.rows {
        if row.len() != output.columns.len() {
            return Err(WireError::new("XX000", "result row width does not match its columns"));
        }
        let mut body = column_count(row.len())?.to_be_bytes().to_vec();
        for value in row {
            if matches!(value, Value::Null) {
                extend(&mut body, &(-1i32).to_be_bytes())?;
            } else {
                let text = value.text();
                if text.as_bytes().contains(&0) {
                    return Err(WireError::new("22021", "PostgreSQL text cannot contain a zero byte"));
                }
                let length = i32::try_from(text.len())
                    .map_err(|_| WireError::new("54000", "result value exceeds signed 32 bits"))?;
                extend(&mut body, &length.to_be_bytes())?;
                extend(&mut body, text.as_bytes())?;
            }
        }
        packet(&mut bytes, b'D', &body)?;
    }
    let mut command = Vec::new();
    cstring(&mut command, tag)?;
    packet(&mut bytes, b'C', &command)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_lengths_and_text_are_bounded() {
        assert!(length32(i32::MAX as usize).is_err());
        assert!(column_count(i16::MAX as usize + 1).is_err());
        let output = QueryOutput { columns: vec!["v".into()],
            rows: vec![vec![Value::Text("\0".into())]], ..Default::default() };
        assert_eq!(result(&output, "SELECT 1").unwrap_err().code, "22021");
        let output = QueryOutput { columns: vec!["v".into()],
            rows: vec![vec![Value::Text("x".repeat(MAX_RESPONSE_BYTES))]], ..Default::default() };
        assert_eq!(result(&output, "SELECT 1").unwrap_err().code, "54000");
    }
}
