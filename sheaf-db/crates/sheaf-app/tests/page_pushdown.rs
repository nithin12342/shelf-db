// page_pushdown.rs - integration test for page-level scan pruning (NF-12).
//
// The test drives the production path. Database::run_sql parses the SQL,
// builds the plan, and the optimizer pushes the predicate into the scan. The
// scan tests each page summary and skips the page before any row is decoded.
//
// Every case states its expected result. The skipped count is compared with
// the page layout, so a false skip fails the test.

use sheaf_app::model::Value;
use sheaf_app::{Database, QueryOutput};

const ROWS: usize = 1_600;
/// One page holds SPINE_CELLS rows.
const PAGE: usize = 16;

fn create(db: &mut Database) {
    db.run_sql(
        "CREATE TABLE events (id BIGINT, bucket BIGINT, score DOUBLE, label TEXT, flag BIGINT)",
        true,
    )
    .expect("create");
}

fn load(db: &mut Database) {
    let mut batch = String::new();
    for id in 0..ROWS {
        let flag = if id < ROWS / 2 { "NULL".to_string() } else { id.to_string() };
        batch.push_str(&format!(
            "INSERT INTO events VALUES ({id}, {}, {id}.5, 'g{}', {flag});",
            id / 100,
            id % 3
        ));
        if batch.len() > 20_000 || id + 1 == ROWS {
            db.run_sql(&batch, true).expect("insert batch");
            batch.clear();
        }
    }
}

fn query(db: &mut Database, sql: &str, optimize: bool) -> QueryOutput {
    db.run_sql(sql, optimize).expect("query").pop().expect("one result")
}

fn count_of(output: &QueryOutput) -> i64 {
    let value = &output.rows[0][0];
    match value {
        Value::Int(value) => *value,
        other => panic!("COUNT returned {other:?}"),
    }
}

/// The pages that hold at least one id below the limit.
fn pages_below(limit: usize) -> usize {
    (0..ROWS / PAGE).filter(|page| page * PAGE < limit).count()
}

#[test]
fn an_id_range_skips_every_page_above_the_limit() {
    let mut db = Database::memory();
    create(&mut db);
    load(&mut db);

    let optimized = query(&mut db, "SELECT COUNT(*) FROM events WHERE id < 160", true);
    assert_eq!(count_of(&optimized), 160);
    assert_eq!(optimized.stats.pages_total, ROWS / PAGE);
    assert_eq!(optimized.stats.page_clauses, 1, "the filter is compiled to one clause");
    assert_eq!(
        optimized.stats.pages_total - optimized.stats.pages_skipped,
        pages_below(160),
        "the kept pages must be exactly the pages that hold a matching id"
    );
    assert_eq!(optimized.stats.pages_skipped, 90);
    assert_eq!(
        optimized.stats.scanned_rows, 160,
        "the decoder must run for the kept pages only"
    );

    let baseline = query(&mut db, "SELECT COUNT(*) FROM events WHERE id < 160", false);
    assert_eq!(count_of(&baseline), 160);
    assert_eq!(baseline.stats.pages_skipped, 0);
    assert_eq!(baseline.stats.page_clauses, 0);
    assert_eq!(baseline.stats.scanned_rows, ROWS);
}

#[test]
fn a_null_predicate_skips_pages_with_no_null() {
    let mut db = Database::memory();
    create(&mut db);
    load(&mut db);

    let nulls = query(&mut db, "SELECT COUNT(*) FROM events WHERE flag IS NULL", true);
    assert_eq!(count_of(&nulls), 800);
    assert_eq!(nulls.stats.pages_skipped, 50, "the upper half holds no null");

    let present = query(&mut db, "SELECT COUNT(*) FROM events WHERE flag IS NOT NULL", true);
    assert_eq!(count_of(&present), 800);
    assert_eq!(present.stats.pages_skipped, 50, "the lower half is all null");
}

#[test]
fn a_text_range_beyond_every_page_skips_the_whole_table() {
    let mut db = Database::memory();
    create(&mut db);
    load(&mut db);

    let output = query(&mut db, "SELECT COUNT(*) FROM events WHERE label = 'g9'", true);
    assert_eq!(count_of(&output), 0);
    assert_eq!(output.stats.pages_total, ROWS / PAGE);
    assert_eq!(output.stats.pages_skipped, ROWS / PAGE);
    assert_eq!(output.stats.scanned_rows, 0);
}

#[test]
fn a_compound_predicate_keeps_every_page_and_the_row_filter_decides() {
    let mut db = Database::memory();
    create(&mut db);
    load(&mut db);

    let output = query(&mut db, "SELECT COUNT(*) FROM events WHERE id + 0 < 160", true);
    assert_eq!(count_of(&output), 160, "the row filter still returns the right rows");
    assert_eq!(output.stats.page_clauses, 0, "the summary cannot test an arithmetic expression");
    assert_eq!(output.stats.pages_skipped, 0, "an untested clause keeps every page");
    assert_eq!(output.stats.scanned_rows, ROWS);
}

#[test]
fn the_same_rows_come_back_on_both_paths() {
    let mut db = Database::memory();
    create(&mut db);
    load(&mut db);

    let sql = "SELECT id, score FROM events WHERE id >= 1580 ORDER BY id";
    let optimized = query(&mut db, sql, true);
    let baseline = query(&mut db, sql, false);
    assert_eq!(optimized.rows, baseline.rows);
    assert_eq!(optimized.rows.len(), 20);
    assert_eq!(optimized.stats.pages_skipped, 98);
    assert_eq!(baseline.stats.pages_skipped, 0);
}
