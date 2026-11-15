//! Local command entry point. Persistence always requires an explicit directory.
mod pgwire;
mod server;

// These aliases also let the adapters become library modules without import changes.
use sheaf_app::{model, service, sql, Database};
use model::MAX_REQUEST_BYTES;
use service::{Application, Request, Response};
use std::ffi::OsString;
use std::fs::File;
use std::io::{self, Read, Write};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const HELP: &str = "sheaf-app: local database command interface

Usage:
  sheaf-app [--db DIRECTORY] [--baseline] sql QUERY
  sheaf-app [--db DIRECTORY] [--baseline] tql PROGRAM
  sheaf-app [--db DIRECTORY] [--baseline] sql-file FILE
  sheaf-app [--db DIRECTORY] [--baseline] tql-file FILE
  sheaf-app [--db DIRECTORY] init
  sheaf-app [--db DIRECTORY] [--baseline] serve [--address ADDRESS] [--connections N]
  sheaf-app [--db DIRECTORY] [--baseline] pgserve [--address ADDRESS] [--connections N]
  sheaf-app --help

SQL means Structured Query Language. TQL means Topological Query Language.
Put global flags before the command. Quote each inline query as one argument.
Without --db, the database exists only in memory. No fixture data is loaded.
--db opens or creates the supplied directory. init opens an empty database or an existing database.
--baseline disables query optimization, including for network requests.
Query output uses JavaScript Object Notation (JSON). Each result includes rows,
rendered TQL, an executable plan, and measured execution statistics when applicable.
Input files must be regular files. Queries and file contents cannot exceed 65536 bytes.

serve uses four-byte little-endian length framing around JSON Request and Response objects.
Request fields: language (sql or tql), query (string), optimize (optional boolean).
The default serve address is 127.0.0.1:7400. The default pgserve address is 127.0.0.1:5433.
Only numeric loopback addresses are allowed. IPv6 loopback [::1]:PORT is also allowed.
--connections N stops after N accepted connections, including failed connections. N must be positive.
Both servers own the database serially. Connections support repeated requests.
Socket reads and writes use a 30-second timeout. An idle client can block other clients until timeout.

pgserve implements PostgreSQL wire protocol 3.0 without a PostgreSQL installation.
It uses localhost trust-only access, with no credentials or authentication guarantees.
User and database startup names are labels. Only --db selects persistent storage.
Encryption requests receive N. Do not forward or expose this local port to other hosts.
Use simple Query messages, one SQL command at a time, with client autocommit enabled.
Explicit transactions, extended/prepared queries, COPY, and cancellation are not supported.
Unsupported extended messages close the connection. SQL errors keep the connection usable.
Startup and message lengths, including their four-byte length field, cannot exceed 65536 bytes.
Results use text type identifier 25. Null values remain distinct from the text NULL.
Responses cannot exceed 4 MiB. Use LIMIT for large results.
";

enum Input { Text(String), File(PathBuf) }
enum Command {
    Help,
    Init,
    Query { language: &'static str, input: Input },
    Serve { postgres: bool, address: SocketAddr, connections: Option<usize> },
}
struct Arguments { database: Option<PathBuf>, optimize: bool, command: Command }

fn value(args: &mut impl Iterator<Item = OsString>, flag: &str) -> Result<OsString, String> {
    let value = args.next().ok_or_else(|| format!("{flag} requires a value"))?;
    if value.is_empty() || value.to_string_lossy().starts_with("--") || value == "-h" {
        return Err(format!("{flag} requires a value"));
    }
    Ok(value)
}

fn utf8(value: OsString, name: &str) -> Result<String, String> {
    value.into_string().map_err(|_| format!("{name} must use UTF-8"))
}

fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Arguments, String> {
    let mut args = args.into_iter();
    let mut database = None;
    let mut optimize = true;
    let name = loop {
        let arg = args.next().ok_or("missing command")?;
        match arg.to_str() {
            Some("--db") => {
                if database.is_some() { return Err("--db can appear only once".into()); }
                database = Some(PathBuf::from(value(&mut args, "--db")?));
            }
            Some("--baseline") => {
                if !optimize { return Err("--baseline can appear only once".into()); }
                optimize = false;
            }
            _ => break utf8(arg, "command")?,
        }
    };
    let command = match name.as_str() {
        "--help" | "-h" => Command::Help,
        "init" => Command::Init,
        "sql" | "tql" | "sql-file" | "tql-file" => {
            let language = if name.starts_with("sql") { "sql" } else { "tql" };
            let argument = value(&mut args, &name)?;
            let input = if name.ends_with("-file") { Input::File(PathBuf::from(argument)) }
                else { Input::Text(utf8(argument, "query")?) };
            Command::Query { language, input }
        }
        "serve" | "pgserve" => {
            let postgres = name == "pgserve";
            let mut address = None;
            let mut connections = None;
            while let Some(arg) = args.next() {
                match arg.to_str() {
                    Some("--address") => {
                        if address.is_some() { return Err("--address can appear only once".into()); }
                        let text = utf8(value(&mut args, "--address")?, "address")?;
                        let parsed: SocketAddr = text.parse()
                            .map_err(|_| "--address requires a numeric address and port, for example 127.0.0.1:7400")?;
                        if !parsed.ip().is_loopback() { return Err("--address must be a loopback address".into()); }
                        address = Some(parsed);
                    }
                    Some("--connections") => {
                        if connections.is_some() { return Err("--connections can appear only once".into()); }
                        let number = utf8(value(&mut args, "--connections")?, "connection count")?
                            .parse::<usize>().map_err(|_| "--connections requires a positive integer")?;
                        if number == 0 { return Err("--connections must be greater than zero".into()); }
                        connections = Some(number);
                    }
                    _ => return Err(format!("unknown {name} argument: {}", arg.to_string_lossy())),
                }
            }
            let address = address.unwrap_or_else(|| SocketAddr::from(([127, 0, 0, 1], if postgres { 5433 } else { 7400 })));
            Command::Serve { postgres, address, connections }
        }
        _ => return Err(format!("unknown command or argument: {name}")),
    };
    if let Some(extra) = args.next() {
        return Err(format!("unexpected argument: {}. Put global flags before the command", extra.to_string_lossy()));
    }
    Ok(Arguments { database, optimize, command })
}

fn read_file(path: &Path) -> Result<String, String> {
    let file = File::open(path).map_err(|error| format!("cannot open '{}': {error}", path.display()))?;
    let metadata = file.metadata().map_err(|error| error.to_string())?;
    if !metadata.is_file() { return Err("query input must be a regular file".into()); }
    if metadata.len() > MAX_REQUEST_BYTES as u64 { return Err("input file exceeds 65536 bytes".into()); }
    // A bounded reader also handles a file that grows after the metadata examination.
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(MAX_REQUEST_BYTES as u64 + 1).read_to_end(&mut bytes).map_err(|error| error.to_string())?;
    if bytes.len() > MAX_REQUEST_BYTES { return Err("input file exceeds 65536 bytes".into()); }
    String::from_utf8(bytes).map_err(|_| "input file must use UTF-8".into())
}

fn print_json(value: &impl serde::Serialize) -> Result<(), String> {
    let mut output = io::stdout().lock();
    serde_json::to_writer(&mut output, value).map_err(|error| error.to_string())?;
    output.write_all(b"\n").map_err(|error| error.to_string())
}

fn run(arguments: Arguments) -> Result<(), String> {
    if matches!(&arguments.command, Command::Help) {
        return io::stdout().lock().write_all(HELP.as_bytes()).map_err(|error| error.to_string());
    }
    // Reject invalid input before opening a persistent database.
    let request = if let Command::Query { language, input } = &arguments.command {
        let query = match input { Input::Text(text) => text.clone(), Input::File(path) => read_file(path)? };
        if query.len() > MAX_REQUEST_BYTES { return Err("query exceeds 65536 bytes".into()); }
        Some(Request { language: (*language).into(), query, optimize: arguments.optimize })
    } else { None };
    let database = match &arguments.database {
        Some(path) => Database::open(path)
            .map_err(|error| format!("cannot open database '{}': {error}", path.display()))?,
        None => Database::memory(),
    };
    match arguments.command {
        Command::Help => unreachable!(),
        Command::Init => print_json(&serde_json::json!({
            "ok": true, "results": [], "error": null,
            "storage": arguments.database.as_ref().map(|path| path.to_string_lossy()),
            "memory_only": arguments.database.is_none(),
            "backends": database.backends(), "epoch": database.epoch(),
        })),
        Command::Query { .. } => {
            let application = Application::new(database);
            let results = application.execute(&request.ok_or("missing query request")?)?;
            print_json(&Response { ok: true, results, error: None })
        }
        Command::Serve { postgres, address, connections } => {
            let listener = server::bind(address).map_err(|error| error.to_string())?;
            eprintln!("{} listening on {} (localhost trust-only, serial database owner)",
                if postgres { "pgserve" } else { "serve" }, listener.local_addr().map_err(|error| error.to_string())?);
            let application = Application::new(database);
            let options = server::ServerOptions { connections, optimize: arguments.optimize, ..Default::default() };
            if postgres { pgwire::serve(listener, &application, options) }
                else { server::serve(listener, &application, options) }.map_err(|error| error.to_string())
        }
    }
}

fn main() -> ExitCode {
    match parse(std::env::args_os().skip(1)).and_then(run) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("sheaf-app: {error}\nUse sheaf-app --help for usage.");
            let _ = print_json(&Response { ok: false, results: Vec::new(), error: Some(error) });
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arguments(words: &[&str]) -> Result<Arguments, String> {
        parse(words.iter().map(OsString::from))
    }

    #[test]
    fn memory_is_the_default_and_persistence_is_explicit() {
        let parsed = arguments(&["sql", "SELECT id FROM t"]).unwrap();
        assert!(parsed.database.is_none());
        assert!(parsed.optimize);
        let parsed = arguments(&["--db", "local-data", "--baseline", "init"]).unwrap();
        assert_eq!(parsed.database, Some(PathBuf::from("local-data")));
        assert!(!parsed.optimize);
    }

    #[test]
    fn argument_errors_are_explicit() {
        for words in [vec![], vec!["--db"], vec!["sql"], vec!["init", "extra"],
            vec!["serve", "--connections", "0"], vec!["serve", "--connections", "-1"],
            vec!["serve", "--address", "0.0.0.0:9000"], vec!["pgserve", "--unknown"],
            vec!["tql-file"], vec!["--baseline", "--baseline", "init"],
            vec!["--db", "--baseline", "init"], vec!["--mystery"],
            vec!["serve", "--address"], vec!["sql", "x", "--baseline"]] {
            assert!(arguments(&words).is_err(), "accepted {words:?}");
        }
    }
}
