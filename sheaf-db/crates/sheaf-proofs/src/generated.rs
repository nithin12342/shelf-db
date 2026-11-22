// Timestamp: 2026-11-22_11-53-08 +05:30
// generated.rs - parse the rule header emitted by `lake exe emitRules` (NF-01).
//
// The header is the authority. This module reads it at runtime, so a change to
// the Lean-emitted table fails the drift test unless the Rust mirror is also
// updated. No external dependencies: the parser is strict and reports errors.

use std::fmt;

/// The header text emitted by the Lean tier.
pub const HEADER: &str = include_str!("../../../native/proofs-generated/sheaf_verified_rules.generated.h");
/// The text manifest emitted by the same run.
pub const MANIFEST: &str = include_str!("../../../proofs/generated/verified_rules.txt");

/// One rule as written by the emitter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmittedRule {
    pub id: u16,
    pub name: String,
    pub theorem: String,
}

/// The table as written by the emitter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmittedTable {
    pub rules: Vec<EmittedRule>,
    pub count: usize,
    pub digest: u64,
}

/// The checksum the Lean tier computes over ids, names, and theorem names.
pub fn recompute_digest(rules: &[EmittedRule]) -> u64 {
    rules.iter().fold(0u64, |sum, rule| {
        sum + rule.id as u64 + rule.name.len() as u64 + rule.theorem.len() as u64
    })
}

/// Parse the generated header. A missing field or a malformed row is an error.
pub fn parse_header(text: &str) -> Result<EmittedTable, String> {
    let mut count = None;
    let mut digest = None;
    let mut rules = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if let Some(value) = line.strip_prefix("#define SHEAF_RULE_COUNT ") {
            count = Some(parse_define(value, "SHEAF_RULE_COUNT")?);
        } else if let Some(value) = line.strip_prefix("#define SHEAF_RULE_DIGEST ") {
            digest = Some(parse_define(value, "SHEAF_RULE_DIGEST")?);
        } else if line.starts_with("{ ") && line.ends_with("},") {
            rules.push(parse_rule(line)?);
        }
    }
    let count = count.ok_or("the header has no SHEAF_RULE_COUNT")? as usize;
    let digest = digest.ok_or("the header has no SHEAF_RULE_DIGEST")?;
    if rules.len() != count {
        return Err(format!("the header declares {count} rules but contains {}", rules.len()));
    }
    for pair in rules.windows(2) {
        if pair[0].id >= pair[1].id {
            return Err("the emitted rule ids are not strictly increasing".into());
        }
    }
    Ok(EmittedTable { rules, count, digest })
}

fn parse_define(value: &str, name: &str) -> Result<u64, String> {
    let value = value.trim().trim_end_matches('u');
    value.parse::<u64>().map_err(|_| format!("invalid {name} in the header"))
}

fn parse_rule(line: &str) -> Result<EmittedRule, String> {
    let body = line.trim_start_matches("{ ").trim_end_matches("},").trim();
    let fields: Vec<&str> = body.split(',').map(str::trim).collect();
    if fields.len() != 3 {
        return Err(format!("a rule row has {} fields, not 3", fields.len()));
    }
    let id = fields[0].parse::<u16>().map_err(|_| "invalid rule id in the header")?;
    let name = unquote(fields[1], "rule name")?;
    let theorem = unquote(fields[2], "theorem name")?;
    Ok(EmittedRule { id, name, theorem })
}

fn unquote(field: &str, label: &str) -> Result<String, String> {
    if field.len() < 2 || !field.starts_with('"') || !field.ends_with('"') {
        return Err(format!("invalid {label} in the header"));
    }
    Ok(field[1..field.len() - 1].to_owned())
}

impl fmt::Display for EmittedTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} rules, digest {}", self.count, self.digest)
    }
}
