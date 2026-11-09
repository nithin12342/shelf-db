// Command-line parsing shared by the bench binaries.
//
// Every bench takes its data paths from the command line. No host path is
// compiled into a binary, so the same build runs on any machine.

/// The value of `--flag=value` or `--flag value`, or None when absent.
pub fn value(args: &[String], flag: &str) -> Option<String> {
    let with_eq = format!("{flag}=");
    for (i, a) in args.iter().enumerate() {
        if let Some(rest) = a.strip_prefix(&with_eq) {
            return Some(rest.to_string());
        }
        if a == flag {
            return args.get(i + 1).cloned();
        }
    }
    None
}

/// A required path flag. Exits 2 with a clear message when absent.
#[allow(dead_code)]
pub fn required_path(args: &[String], flag: &str) -> String {
    match value(args, flag) {
        Some(v) if !v.is_empty() => v,
        _ => {
            eprintln!("error: missing required flag {flag}=<path>");
            std::process::exit(2);
        }
    }
}

/// An optional flag with a default. A bad value exits 2.
#[allow(dead_code)]
pub fn opt<T: std::str::FromStr>(args: &[String], flag: &str, default: T) -> T
where
    T::Err: std::fmt::Display,
{
    match value(args, flag) {
        Some(v) => v.parse().unwrap_or_else(|e| {
            eprintln!("error: invalid {flag} value '{v}': {e}");
            std::process::exit(2);
        }),
        None => default,
    }
}
