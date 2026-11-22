// morphism_ddl.rs - integration test for morphism declaration DDL (NF-15).
//
// The test calls the production path: Database::run_sql parses the statement,
// Database::execute validates the declaration against the catalog, and the
// journal records it. A bad cell or column reference is an explicit error.

use sheaf_app::Database;
use std::path::PathBuf;

fn ddl(name: &str, from: &str, to: &str) -> String {
    format!(
        "DEFINE MORPHISM {name} FROM {from} TO {to} PROJ (id -> ref_id) MATRIX [[1.0, 0.0], [0.0, 1.0]];"
    )
}

fn database() -> Database {
    let mut db = Database::memory();
    db.run_sql("CREATE TABLE Account (id BIGINT, name TEXT)", true).expect("account");
    db.run_sql("CREATE TABLE Payment (ref_id BIGINT, amount DOUBLE)", true).expect("payment");
    db
}

fn directory() -> PathBuf {
    let path = std::env::temp_dir().join(format!("sheaf_morphism_ddl_{}_{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let _ = std::fs::remove_dir_all(&path);
    path
}

#[test]
fn a_valid_declaration_is_parsed_stored_and_reported() {
    let mut db = database();
    let outputs = db.run_sql(&ddl("AccountToPayment", "Account", "Payment"), true).expect("ddl");
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].affected, 1);
    assert_eq!(outputs[0].rows[0][0], sheaf_app::model::Value::Text("AccountToPayment".into()));
    assert!(outputs[0].plan[0].contains("1 projections, 2x2 matrix"));

    let morphism = db.morphism("AccountToPayment").expect("stored morphism");
    assert_eq!(morphism.from, "account");
    assert_eq!(morphism.to, "payment");
    assert_eq!(morphism.projection, vec![("id".into(), "ref_id".into())]);
    assert_eq!(morphism.matrix, vec![vec![1.0, 0.0], vec![0.0, 1.0]]);
    assert_eq!(db.morphism_count(), 1);
}

#[test]
fn an_unknown_cell_reference_is_rejected() {
    let mut db = database();
    let error = db.run_sql(&ddl("BadSource", "Missing", "Payment"), true).unwrap_err();
    assert!(error.contains("unknown source cell missing"), "{error}");
    let error = db.run_sql(&ddl("BadTarget", "Account", "Missing"), true).unwrap_err();
    assert!(error.contains("unknown target cell missing"), "{error}");
    assert_eq!(db.morphism_count(), 0);
}

#[test]
fn an_unknown_column_reference_is_rejected() {
    let mut db = database();
    let text = "DEFINE MORPHISM BadColumn FROM Account TO Payment PROJ (missing -> ref_id) MATRIX [[1.0]];";
    let error = db.run_sql(text, true).unwrap_err();
    assert!(error.contains("unknown source column missing"), "{error}");
    assert_eq!(db.morphism_count(), 0);
}

#[test]
fn incompatible_column_types_are_rejected() {
    let mut db = database();
    let text = "DEFINE MORPHISM BadType FROM Account TO Payment PROJ (name -> ref_id) MATRIX [[1.0]];";
    let error = db.run_sql(text, true).unwrap_err();
    assert!(error.contains("morphism column types differ"), "{error}");
}

#[test]
fn a_duplicate_name_is_rejected() {
    let mut db = database();
    db.run_sql(&ddl("M", "Account", "Payment"), true).expect("first");
    let error = db.run_sql(&ddl("M", "Account", "Payment"), true).unwrap_err();
    assert!(error.contains("morphism already exists: M"), "{error}");
    assert_eq!(db.morphism_count(), 1);
}

#[test]
fn a_mixed_request_is_rejected() {
    let mut db = database();
    let text = format!("CREATE TABLE Extra (id BIGINT); {}", ddl("M", "Account", "Payment"));
    let error = db.run_sql(&text, true).unwrap_err();
    assert!(error.contains("must be the only statement"), "{error}");
}

#[test]
fn a_morphism_survives_journal_recovery() {
    let path = directory();
    {
        let mut db = Database::open(&path).expect("open");
        db.run_sql("CREATE TABLE Account (id BIGINT, name TEXT)", true).expect("account");
        db.run_sql("CREATE TABLE Payment (ref_id BIGINT, amount DOUBLE)", true).expect("payment");
        db.run_sql(&ddl("AccountToPayment", "Account", "Payment"), true).expect("ddl");
    }
    {
        let db = Database::open(&path).expect("reopen");
        let morphism = db.morphism("AccountToPayment").expect("recovered morphism");
        assert_eq!(morphism.from, "account");
        assert_eq!(morphism.to, "payment");
        assert_eq!(morphism.projection.len(), 1);
        assert_eq!(db.morphism_count(), 1);
    }
    let _ = std::fs::remove_dir_all(&path);
}
