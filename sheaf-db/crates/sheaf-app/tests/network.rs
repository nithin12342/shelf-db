//! Protocol tests use standard sockets, not a PostgreSQL installation or client crate.
#[path = "../src/server.rs"]
mod server;
#[path = "../src/pgwire.rs"]
mod pgwire;

use sheaf_app::{model, service, sql, Database};
use service::{Application, Request, Response};
use std::io::{Read, Write};
use std::net::{Shutdown, SocketAddr, TcpStream};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const CREATE: &str = "CREATE TABLE items (id INT, label TEXT)";
const INSERT: &str = "INSERT INTO items VALUES (1, 'repeat'), (1, 'repeat'), (2, NULL), (3, 'NULL')";
const SELECT: &str = "SELECT id AS id, label AS label FROM items ORDER BY id";
type Packet = (u8, Vec<u8>);

fn start(postgres: bool, connections: usize, timeout: Duration) -> (SocketAddr, JoinHandle<()>) {
    let listener = server::bind("127.0.0.1:0".parse().unwrap()).unwrap();
    let address = listener.local_addr().unwrap();
    let handle = thread::spawn(move || {
        // Storage trait objects are not Send. Construct and own the database here.
        let application = Application::new(Database::memory());
        let options = server::ServerOptions { connections: Some(connections), timeout, optimize: true };
        if postgres { pgwire::serve(listener, &application, options).unwrap(); }
        else { server::serve(listener, &application, options).unwrap(); }
    });
    (address, handle)
}

fn connect(address: SocketAddr) -> TcpStream {
    let stream = TcpStream::connect_timeout(&address, Duration::from_secs(3)).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    stream.set_write_timeout(Some(Duration::from_secs(3))).unwrap();
    stream.set_nodelay(true).unwrap();
    stream
}

fn startup_packet(stream: &mut TcpStream, body: &[u8]) {
    stream.write_all(&u32::try_from(body.len() + 4).unwrap().to_be_bytes()).unwrap();
    stream.write_all(body).unwrap();
}

fn packet(stream: &mut TcpStream, kind: u8, body: &[u8]) {
    stream.write_all(&[kind]).unwrap();
    stream.write_all(&u32::try_from(body.len() + 4).unwrap().to_be_bytes()).unwrap();
    stream.write_all(body).unwrap();
}

fn receive(stream: &mut TcpStream) -> Packet {
    let mut kind = [0u8; 1];
    stream.read_exact(&mut kind).unwrap();
    let mut header = [0u8; 4];
    stream.read_exact(&mut header).unwrap();
    let length = i32::from_be_bytes(header);
    assert!((4..=4 * 1024 * 1024).contains(&length));
    let mut body = vec![0u8; length as usize - 4];
    stream.read_exact(&mut body).unwrap();
    (kind[0], body)
}

fn until_ready(stream: &mut TcpStream) -> Vec<Packet> {
    let mut packets = Vec::new();
    loop {
        let reply = receive(stream);
        let ready = reply.0 == b'Z';
        if ready { assert_eq!(reply.1, b"I"); }
        packets.push(reply);
        if ready { return packets; }
        assert!(packets.len() < 100, "missing ReadyForQuery");
    }
}

fn handshake(stream: &mut TcpStream) {
    startup_packet(stream, &80_877_103u32.to_be_bytes());
    let mut answer = [0u8; 1];
    stream.read_exact(&mut answer).unwrap();
    assert_eq!(&answer, b"N");
    let mut body = 196_608u32.to_be_bytes().to_vec();
    body.extend_from_slice(b"user\0test-user\0database\0test-label\0client_encoding\0UTF8\0\0");
    startup_packet(stream, &body);
    let replies = until_ready(stream);
    assert_eq!(replies[0], (b'R', 0u32.to_be_bytes().to_vec()));
    assert!(replies.iter().any(|(tag, body)| *tag == b'S' && body == b"client_encoding\0UTF8\0"));
    assert!(!replies.iter().any(|(tag, _)| *tag == b'E'));
}

fn query(stream: &mut TcpStream, text: &str) -> Vec<Packet> {
    let mut body = text.as_bytes().to_vec();
    body.push(0);
    packet(stream, b'Q', &body);
    until_ready(stream)
}

fn command(packets: &[Packet], expected: &str) {
    let (_, body) = packets.iter().find(|(tag, _)| *tag == b'C')
        .unwrap_or_else(|| panic!("missing CommandComplete: {packets:?}"));
    assert_eq!(body, format!("{expected}\0").as_bytes());
    assert!(!packets.iter().any(|(tag, _)| *tag == b'E'));
}

fn string(bytes: &[u8], offset: &mut usize) -> String {
    let length = bytes[*offset..].iter().position(|b| *b == 0).unwrap();
    let text = String::from_utf8(bytes[*offset..*offset + length].to_vec()).unwrap();
    *offset += length + 1;
    text
}

fn error_code(reply: &Packet) -> String {
    assert_eq!(reply.0, b'E');
    let mut offset = 0;
    let mut code = None;
    while reply.1[offset] != 0 {
        let field = reply.1[offset];
        offset += 1;
        let value = string(&reply.1, &mut offset);
        if field == b'C' { code = Some(value); }
    }
    assert_eq!(offset + 1, reply.1.len());
    let code = code.unwrap();
    assert_eq!(code.len(), 5);
    code
}

fn columns(packets: &[Packet]) -> Vec<String> {
    let body = &packets.iter().find(|(tag, _)| *tag == b'T').unwrap().1;
    let count = i16::from_be_bytes(body[..2].try_into().unwrap());
    let mut offset = 2;
    let mut names = Vec::new();
    for _ in 0..count {
        names.push(string(body, &mut offset));
        let fields = &body[offset..offset + 18];
        assert_eq!(u32::from_be_bytes(fields[6..10].try_into().unwrap()), 25);
        assert_eq!(i16::from_be_bytes(fields[10..12].try_into().unwrap()), -1);
        assert_eq!(i16::from_be_bytes(fields[16..18].try_into().unwrap()), 0);
        offset += 18;
    }
    assert_eq!(offset, body.len());
    names
}

fn rows(packets: &[Packet]) -> Vec<Vec<Option<String>>> {
    packets.iter().filter(|(tag, _)| *tag == b'D').map(|(_, body)| {
        let count = i16::from_be_bytes(body[..2].try_into().unwrap());
        let mut offset = 2;
        let mut row = Vec::new();
        for _ in 0..count {
            let length = i32::from_be_bytes(body[offset..offset + 4].try_into().unwrap());
            offset += 4;
            if length == -1 { row.push(None); }
            else {
                assert!(length >= 0);
                let end = offset + length as usize;
                row.push(Some(String::from_utf8(body[offset..end].to_vec()).unwrap()));
                offset = end;
            }
        }
        assert_eq!(offset, body.len());
        row
    }).collect()
}

fn expected_rows() -> Vec<Vec<Option<String>>> {
    vec![vec![Some("1".into()), Some("repeat".into())],
        vec![Some("1".into()), Some("repeat".into())],
        vec![Some("2".into()), None], vec![Some("3".into()), Some("NULL".into())]]
}

fn closed(stream: &mut TcpStream) {
    let mut byte = [0u8; 1];
    match stream.read(&mut byte) {
        Ok(0) => {}
        Err(error) if matches!(error.kind(), std::io::ErrorKind::ConnectionReset
            | std::io::ErrorKind::ConnectionAborted | std::io::ErrorKind::BrokenPipe) => {}
        result => panic!("expected closed connection, got {result:?}"),
    }
}

#[test]
fn postgres_create_insert_select_preserves_duplicates_and_null() {
    let (address, handle) = start(true, 1, Duration::from_secs(2));
    let mut stream = connect(address);
    handshake(&mut stream);
    command(&query(&mut stream, CREATE), "CREATE TABLE");
    command(&query(&mut stream, INSERT), "INSERT 0 4");
    let selected = query(&mut stream, SELECT);
    assert_eq!(columns(&selected), vec!["id", "label"]);
    assert_eq!(rows(&selected), expected_rows());
    command(&selected, "SELECT 4");
    let empty = query(&mut stream, "SELECT id FROM items WHERE id = 99");
    assert_eq!(columns(&empty).len(), 1);
    assert!(rows(&empty).is_empty());
    command(&empty, "SELECT 0");
    command(&query(&mut stream, "INSERT INTO items VALUES (4, 'a;b')"), "INSERT 0 1");
    command(&query(&mut stream, "UPDATE items SET label = 'changed' WHERE id = 4"), "UPDATE 1");
    command(&query(&mut stream, "DELETE FROM items WHERE id = 4"), "DELETE 1");
    for text in ["EXPLAIN SELECT id FROM items", "EXPLAIN ANALYZE SELECT id FROM items"] {
        let plan = query(&mut stream, text);
        assert_eq!(columns(&plan), vec!["QUERY PLAN"]);
        assert!(!rows(&plan).is_empty());
        command(&plan, "EXPLAIN");
    }
    packet(&mut stream, b'X', &[]);
    closed(&mut stream);
    handle.join().unwrap();
}

#[test]
fn sql_errors_recover_and_multi_commands_never_partly_execute() {
    let (address, handle) = start(true, 1, Duration::from_secs(2));
    let mut stream = connect(address);
    handshake(&mut stream);
    command(&query(&mut stream, CREATE), "CREATE TABLE");
    for text in ["BEGIN", "START TRANSACTION", "COMMIT", "ROLLBACK", "SAVEPOINT x",
        "INSERT INTO items VALUES (9, 'bad'); INSERT INTO items VALUES (10, 'bad')"] {
        let replies = query(&mut stream, text);
        assert_eq!(error_code(&replies[0]), "0A000", "{text}");
        assert_eq!(replies.len(), 2);
    }
    assert!(rows(&query(&mut stream, SELECT)).is_empty());
    assert_eq!(error_code(&query(&mut stream, "SELECT id FROM absent")[0]), "42P01");
    assert_eq!(error_code(&query(&mut stream, "SELECT FROM")[0]), "42601");
    let empty = query(&mut stream, " -- empty query\n ");
    assert_eq!(empty, vec![(b'I', vec![]), (b'Z', b"I".to_vec())]);
    command(&query(&mut stream, INSERT), "INSERT 0 4");
    assert_eq!(rows(&query(&mut stream, SELECT)), expected_rows());
    packet(&mut stream, b'X', &[]);
    handle.join().unwrap();
}

#[test]
fn unsupported_extended_protocol_closes_without_running_queued_sql() {
    let (address, handle) = start(true, 2, Duration::from_secs(2));
    let mut stream = connect(address);
    handshake(&mut stream);
    packet(&mut stream, b'P', b"\0CREATE TABLE hidden (id INT)\0\0\0");
    assert_eq!(error_code(&receive(&mut stream)), "0A000");
    closed(&mut stream);
    let mut next = connect(address);
    handshake(&mut next);
    assert_eq!(error_code(&query(&mut next, "SELECT id FROM hidden")[0]), "42P01");
    packet(&mut next, b'X', &[]);
    handle.join().unwrap();
}

#[test]
fn startup_rejects_lengths_cancellation_versions_and_bad_parameters() {
    let mut cases: Vec<(Vec<u8>, &str)> = [0u32, 7, 65_537, u32::MAX].into_iter()
        .map(|length| (length.to_be_bytes().to_vec(), "08P01")).collect();
    for (body, code) in [
        (80_877_102u32.to_be_bytes().into_iter().chain([0; 8]).collect::<Vec<_>>(), "0A000"),
        (131_072u32.to_be_bytes().to_vec(), "0A000"),
        ([196_608u32.to_be_bytes().as_slice(), b"user\0x\0"].concat(), "08P01"),
        ([196_608u32.to_be_bytes().as_slice(), b"user\0x\0user\0y\0\0"].concat(), "08P01"),
        ([196_608u32.to_be_bytes().as_slice(), b"user\0x\0client_encoding\0LATIN1\0\0"].concat(), "0A000"),
    ] {
        cases.push(([u32::try_from(body.len() + 4).unwrap().to_be_bytes().as_slice(), &body].concat(), code));
    }
    let (address, handle) = start(true, cases.len(), Duration::from_secs(2));
    for (bytes, code) in cases {
        let mut stream = connect(address);
        stream.write_all(&bytes).unwrap();
        assert_eq!(error_code(&receive(&mut stream)), code);
        closed(&mut stream);
    }
    handle.join().unwrap();
}

#[test]
fn malformed_messages_and_truncated_packets_are_rejected() {
    let cases = vec![
        vec![b'Q', 0, 0, 0, 3],
        [vec![b'Q'], 65_537u32.to_be_bytes().to_vec()].concat(),
        vec![b'Q', 0, 0, 0, 5, b'x'],
        vec![b'Q', 0, 0, 0, 7, b'x', 0, 0],
        vec![b'?', 0, 0, 0, 4],
        vec![b'X', 0, 0, 0, 5, 0],
        vec![b'Q', 0, 0, 0, 8, b'x'],
    ];
    let (address, handle) = start(true, cases.len(), Duration::from_secs(2));
    for bytes in cases {
        let mut stream = connect(address);
        handshake(&mut stream);
        stream.write_all(&bytes).unwrap();
        stream.shutdown(Shutdown::Write).unwrap();
        assert_eq!(error_code(&receive(&mut stream)), "08P01");
        closed(&mut stream);
    }
    handle.join().unwrap();
}

fn framed(stream: &mut TcpStream, language: &str, query: &str) -> Response {
    let bytes = serde_json::to_vec(&Request { language: language.into(), query: query.into(), optimize: true }).unwrap();
    let header = u32::try_from(bytes.len()).unwrap().to_le_bytes();
    // Separate header writes exercise partial transport delivery.
    for byte in header { stream.write_all(&[byte]).unwrap(); }
    stream.write_all(&bytes).unwrap();
    let mut header = [0u8; 4];
    stream.read_exact(&mut header).unwrap();
    let length = u32::from_le_bytes(header) as usize;
    assert!(length <= 4 * 1024 * 1024);
    let mut bytes = vec![0u8; length];
    stream.read_exact(&mut bytes).unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[test]
fn framed_json_roundtrip_includes_tql_plan_stats_and_nulls() {
    let (address, handle) = start(false, 1, Duration::from_secs(2));
    let mut stream = connect(address);
    assert!(framed(&mut stream, "sql", CREATE).ok);
    assert!(framed(&mut stream, "sql", INSERT).ok);
    assert!(!framed(&mut stream, "unknown", SELECT).ok);
    let response = framed(&mut stream, "sql", SELECT);
    assert!(response.ok, "{:?}", response.error);
    let output = &response.results[0];
    assert_eq!(output.rows.len(), 4);
    assert_eq!(output.rows[0], output.rows[1]);
    assert_eq!(output.rows[2][1], model::Value::Null);
    assert_eq!(output.rows[3][1], model::Value::Text("NULL".into()));
    assert!(!output.plan.is_empty());
    assert_eq!(output.stats.output_rows, 4);
    assert!(serde_json::to_value(&output.stats).unwrap().get("elapsed_us").is_some());
    let translated = framed(&mut stream, "tql", output.tql.as_deref().unwrap());
    assert!(translated.ok, "{:?}", translated.error);
    assert_eq!(translated.results[0].rows, output.rows);
    drop(stream);
    handle.join().unwrap();
}

#[test]
fn framed_oversized_header_is_rejected_before_body_arrives() {
    let (address, handle) = start(false, 2, Duration::from_secs(2));
    let mut oversized = connect(address);
    oversized.write_all(&65_537u32.to_le_bytes()).unwrap();
    closed(&mut oversized);
    let mut next = connect(address);
    assert!(framed(&mut next, "sql", CREATE).ok);
    drop(next);
    handle.join().unwrap();
}

#[test]
fn idle_connections_time_out_and_remote_bind_is_rejected() {
    assert!(server::bind("0.0.0.0:0".parse().unwrap()).is_err());
    assert!(server::bind("[::]:0".parse().unwrap()).is_err());
    for postgres in [true, false] {
        let (address, handle) = start(postgres, 1, Duration::from_millis(150));
        let mut stream = connect(address);
        if postgres { assert_eq!(error_code(&receive(&mut stream)), "57014"); }
        closed(&mut stream);
        handle.join().unwrap();
    }
}

#[test]
fn command_entrypoint_reports_errors_and_memory_only_init() {
    let binary = env!("CARGO_BIN_EXE_sheaf-app");
    for args in [vec![], vec!["--unknown"], vec!["sql"], vec!["serve", "--connections", "0"],
        vec!["pgserve", "--address", "0.0.0.0:5555"]] {
        let output = std::process::Command::new(binary).args(args).output().unwrap();
        assert!(!output.status.success());
        let error: Response = serde_json::from_slice(&output.stdout).unwrap();
        assert!(!error.ok);
        assert!(error.error.is_some());
    }
    let output = std::process::Command::new(binary).arg("init").output().unwrap();
    assert!(output.status.success());
    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(response["memory_only"], true);
    assert!(response["storage"].is_null());
}
