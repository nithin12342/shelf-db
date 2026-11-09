// crates/sheaf-engine/src/bin/shared/bench_config.rs
// A small key=value reader for benchmark inputs. Standard library only.
#![allow(dead_code)]
use std::collections::HashMap;

use sheaf_query::ir::optimizer::CostModel;

/// Read a key=value file. Blank lines and lines that start with # are ignored.
pub fn load(path: &str) -> HashMap<String, String> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("error: cannot read {path}: {e}");
        std::process::exit(2);
    });
    let mut map = HashMap::new();
    for (n, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            eprintln!("error: {path}:{}: expected key=value", n + 1);
            std::process::exit(2);
        };
        map.insert(key.trim().to_string(), value.trim().to_string());
    }
    map
}

/// A required string value. Exits 2 with a clear message when absent.
pub fn get<'a>(m: &'a HashMap<String, String>, key: &str) -> &'a str {
    m.get(key).map(String::as_str).unwrap_or_else(|| {
        eprintln!("error: missing config key {key}");
        std::process::exit(2);
    })
}

/// A required integer value. A bad value exits 2.
pub fn get_usize(m: &HashMap<String, String>, key: &str) -> usize {
    let v = get(m, key);
    v.parse().unwrap_or_else(|e| {
        eprintln!("error: invalid config value {key}={v}: {e}");
        std::process::exit(2);
    })
}

/// A required floating-point value. A bad value exits 2.
pub fn get_f64(m: &HashMap<String, String>, key: &str) -> f64 {
    let v = get(m, key);
    v.parse().unwrap_or_else(|e| {
        eprintln!("error: invalid config value {key}={v}: {e}");
        std::process::exit(2);
    })
}

/// Build the cost model from the cost section of the config.
pub fn cost_model(m: &HashMap<String, String>) -> CostModel {
    CostModel {
        io_cost_per_row: get_f64(m, "cost.io_cost_per_row"),
        cpu_cost_per_row: get_f64(m, "cost.cpu_cost_per_row"),
        probe_cost_per_slice: get_f64(m, "cost.probe_cost_per_slice"),
        solve_cost_per_dim: get_f64(m, "cost.solve_cost_per_dim"),
    }
}

/// The default config path, relative to the sheaf-db directory.
pub fn default_path() -> String {
    "tests/e2e/realtime_bench/tql_workloads.conf".to_string()
}
