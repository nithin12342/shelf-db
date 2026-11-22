// Timestamp: 2026-11-22_11-53-08 +05:30
// lean_drift.rs - the AOT rule-table drift gate (NF-01).
//
// The test reads the header emitted by `lake exe emitRules`, compares it with
// the Rust mirror, recomputes the checksum, compiles the header with gcc, and
// checks the declared fallback. Every expected result comes from the emitted
// table or from the Rust mirror. No expected value is a literal in this file.

use sheaf_proofs::generated::{self, EmittedTable};
use sheaf_proofs::{
    FallbackReason, GeneratedRules, HandwrittenRules, RuleOracle, VERIFIED_RULES,
};

fn emitted() -> EmittedTable {
    generated::parse_header(generated::HEADER).expect("parse the emitted header")
}

#[test]
fn the_emitted_header_matches_the_rust_mirror() {
    let table = emitted();
    assert_eq!(table.count, VERIFIED_RULES.len());
    assert_eq!(table.rules.len(), VERIFIED_RULES.len());
    for (left, right) in table.rules.iter().zip(VERIFIED_RULES.iter()) {
        assert_eq!(left.id, right.id, "rule id drift");
        assert_eq!(left.name, right.name, "rule name drift");
        assert_eq!(left.theorem, right.lean_theorem, "theorem name drift");
    }
}

#[test]
fn the_checksum_matches_the_rule_rows() {
    let table = emitted();
    assert_eq!(generated::recompute_digest(&table.rules), table.digest);
    assert_eq!(table.digest, GeneratedRules::load().expect("load").digest());
}

#[test]
fn the_manifest_matches_the_header() {
    let table = emitted();
    let lines: Vec<&str> = generated::MANIFEST.lines().filter(|line| !line.is_empty()).collect();
    assert_eq!(lines.len(), table.count);
    for (line, rule) in lines.iter().zip(&table.rules) {
        assert_eq!(*line, format!("{} {} {}", rule.id, rule.name, rule.theorem));
    }
}

#[test]
fn the_emitted_header_compiles_with_gcc() {
    let gcc = std::process::Command::new("gcc")
        .arg("--version")
        .output()
        .expect("unsupported: gcc is not installed on this host");
    assert!(gcc.status.success(), "unsupported: gcc --version failed");

    let table = emitted();
    let first = table.rules.first().expect("at least one rule");
    let source = format!(
        "#include \"sheaf_verified_rules.generated.h\"\n\
         int main(void) {{\n\
           if (SHEAF_RULE_COUNT != {}u) return 1;\n\
           if (SHEAF_VERIFIED_RULES[0].id != {}u) return 2;\n\
           if (SHEAF_RULE_DIGEST != {}u) return 3;\n\
           return 0;\n\
         }}\n",
        table.count, first.id, table.digest
    );
    let dir = std::env::temp_dir().join(format!(
        "sheaf_rule_header_{}_{}",
        std::process::id(),
        table.digest
    ));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let header = dir.join("sheaf_verified_rules.generated.h");
    let code = dir.join("check.c");
    let exe = dir.join("check.exe");
    std::fs::write(&header, generated::HEADER).expect("write header");
    std::fs::write(&code, source).expect("write source");

    let build = std::process::Command::new("gcc")
        .arg("-std=c11")
        .arg("-Wall")
        .arg("-Wextra")
        .arg("-Werror")
        .arg(&code)
        .arg("-o")
        .arg(&exe)
        .output()
        .expect("run gcc");
    assert!(
        build.status.success(),
        "gcc rejected the emitted header: {}",
        String::from_utf8_lossy(&build.stderr)
    );
    let run = std::process::Command::new(&exe).output().expect("run the check");
    assert!(run.status.success(), "the compiled header check returned {:?}", run.status.code());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_fallback_reports_why_it_is_in_use() {
    let generated = GeneratedRules::load().expect("load");
    assert_eq!(generated.source(), "lean-emitted-header");
    assert_eq!(generated.fallback_reason(), None);

    let fallback = HandwrittenRules;
    assert_eq!(fallback.source(), "handwritten-mirror");
    assert_eq!(
        fallback.fallback_reason(),
        Some(FallbackReason::LeanNotBuilt.text())
    );
    assert!(!fallback.fallback_reason().unwrap_or_default().is_empty());
}

#[test]
fn the_oracle_answers_only_for_emitted_rules() {
    let oracle = GeneratedRules::load().expect("load");
    for rule in &oracle.table().rules {
        assert!(oracle.is_verified(rule.id), "rule {} must be verified", rule.id);
    }
    let unknown = oracle.table().rules.iter().map(|r| r.id).max().unwrap_or(0) + 1;
    assert!(!oracle.is_verified(unknown));
}
