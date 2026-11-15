use sheaf_app::model::*;
use sheaf_app::Database;
use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        Self(std::env::temp_dir().join(format!("sheaf-app-{}-{stamp}", std::process::id())))
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn create(db: &mut Database) {
    db.execute(
        &Statement::Create {
            table: "items".into(),
            columns: vec![
                Column {
                    name: "id".into(),
                    data_type: DataType::Int,
                    nullable: false,
                },
                Column {
                    name: "label".into(),
                    data_type: DataType::Text,
                    nullable: true,
                },
            ],
        },
        true,
    )
    .unwrap();
}

fn insert(db: &mut Database, rows: Vec<Row>) -> Result<QueryOutput> {
    db.execute(
        &Statement::Insert {
            table: "items".into(),
            columns: vec![],
            rows,
        },
        true,
    )
}

fn all(db: &mut Database) -> Vec<Row> {
    db.run_sql("SELECT id, label FROM items ORDER BY id", true)
        .unwrap()
        .remove(0)
        .rows
}

#[test]
fn durable_writes_replay_into_native_pages() {
    let directory = Directory::new();
    let expected = vec![
        vec![Value::Int(1), Value::Text("café 雪".into())],
        vec![Value::Int(2), Value::Null],
    ];
    let epoch;
    {
        let mut db = Database::open(&directory.0).unwrap();
        create(&mut db);
        insert(&mut db, expected.clone()).unwrap();
        epoch = db.epoch();
        assert_eq!(all(&mut db), expected);
    }
    let mut db = Database::open(&directory.0).unwrap();
    assert_eq!(db.epoch(), epoch);
    assert_eq!(all(&mut db), expected);
    assert!(db
        .backends()
        .iter()
        .any(|name| name.starts_with("storage=")));
}

#[test]
fn invalid_mutation_does_not_publish_or_commit() {
    let directory = Directory::new();
    {
        let mut db = Database::open(&directory.0).unwrap();
        create(&mut db);
        insert(
            &mut db,
            vec![vec![Value::Int(1), Value::Text("before".into())]],
        )
        .unwrap();
        let epoch = db.epoch();
        let length = std::fs::metadata(directory.0.join("mutations.wal"))
            .unwrap()
            .len();
        assert!(insert(
            &mut db,
            vec![
                vec![Value::Int(2), Value::Null],
                vec![Value::Null, Value::Text("invalid".into())]
            ]
        )
        .is_err());
        assert_eq!(db.epoch(), epoch);
        assert_eq!(
            std::fs::metadata(directory.0.join("mutations.wal"))
                .unwrap()
                .len(),
            length
        );
        assert_eq!(all(&mut db).len(), 1);
    }
    assert_eq!(all(&mut Database::open(&directory.0).unwrap()).len(), 1);
}

#[test]
fn durable_database_has_one_owner() {
    let directory = Directory::new();
    let first = Database::open(&directory.0).unwrap();
    assert!(Database::open(&directory.0).is_err());
    drop(first);
    assert!(Database::open(&directory.0).is_ok());
}

#[test]
fn uncommitted_tail_is_removed_on_restart() {
    let directory = Directory::new();
    {
        let mut db = Database::open(&directory.0).unwrap();
        create(&mut db);
        insert(&mut db, vec![vec![Value::Int(7), Value::Null]]).unwrap();
    }
    let log = directory.0.join("mutations.wal");
    let length = std::fs::metadata(&log).unwrap().len();
    {
        let mut file = OpenOptions::new().append(true).open(&log).unwrap();
        file.write_all(b"interrupted record").unwrap();
        file.sync_all().unwrap();
    }
    let mut db = Database::open(&directory.0).unwrap();
    assert_eq!(all(&mut db), vec![vec![Value::Int(7), Value::Null]]);
    assert_eq!(std::fs::metadata(&log).unwrap().len(), length);
}

#[test]
fn committed_payload_corruption_is_an_error() {
    let directory = Directory::new();
    {
        let mut db = Database::open(&directory.0).unwrap();
        create(&mut db);
        insert(&mut db, vec![vec![Value::Int(7), Value::Null]]).unwrap();
    }
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(directory.0.join("mutations.wal"))
        .unwrap();
    file.seek(SeekFrom::Start(48)).unwrap();
    let mut byte = [0];
    file.read_exact(&mut byte).unwrap();
    byte[0] ^= 1;
    file.seek(SeekFrom::Start(48)).unwrap();
    file.write_all(&byte).unwrap();
    file.sync_all().unwrap();
    drop(file);
    assert!(Database::open(&directory.0).is_err());
}

#[test]
fn large_statement_replays_across_multiple_log_frames() {
    let directory = Directory::new();
    let expected: Vec<Row> = (0..120)
        .map(|id| {
            vec![
                Value::Int(id),
                Value::Text(format!("{id}:{}", "x".repeat(700))),
            ]
        })
        .collect();
    {
        let mut db = Database::open(&directory.0).unwrap();
        create(&mut db);
        insert(&mut db, expected.clone()).unwrap();
    }
    assert_eq!(all(&mut Database::open(&directory.0).unwrap()), expected);
}

#[test]
fn assignments_use_the_original_row() {
    let mut db = Database::memory();
    db.run_sql(
        "CREATE TABLE pairs (a BIGINT, b BIGINT); INSERT INTO pairs VALUES (1, 2);",
        true,
    )
    .unwrap();
    assert_eq!(
        db.run_sql("UPDATE pairs SET a = b, b = a", true).unwrap()[0].affected,
        1
    );
    assert_eq!(
        db.run_sql("SELECT a, b FROM pairs", true).unwrap()[0].rows,
        vec![vec![Value::Int(2), Value::Int(1)]]
    );
}

#[test]
fn null_predicates_and_failed_script_preserve_autocommit() {
    let mut db = Database::memory();
    create(&mut db);
    assert!(db
        .run_sql(
            "INSERT INTO items VALUES (1, NULL); INSERT INTO absent VALUES (2);",
            true
        )
        .is_err());
    assert_eq!(all(&mut db), vec![vec![Value::Int(1), Value::Null]]);
    assert_eq!(
        db.run_sql("DELETE FROM items WHERE label = NULL", true)
            .unwrap()[0]
            .affected,
        0
    );
    assert_eq!(
        db.run_sql("DELETE FROM items WHERE label IS NULL", true)
            .unwrap()[0]
            .affected,
        1
    );
    assert!(all(&mut db).is_empty());
}

#[test]
fn sql_translation_is_reusable_and_plan_paths_agree() {
    let mut db = Database::memory();
    db.run_sql(
        "CREATE TABLE a (id BIGINT, name TEXT); CREATE TABLE b (id BIGINT, amount BIGINT);
        INSERT INTO a VALUES (1, 'one'), (1, 'duplicate'), (2, 'two');
        INSERT INTO b VALUES (1, 10), (1, 20), (2, 30), (NULL, 40);",
        true,
    )
    .unwrap();
    let sql = "SELECT a.name, SUM(b.amount) AS total FROM a JOIN b ON a.id = b.id
        WHERE b.amount >= 20 GROUP BY a.name ORDER BY total, a.name";
    let fast = db.run_sql(sql, true).unwrap().remove(0);
    let baseline = db.run_sql(sql, false).unwrap().remove(0);
    let translated = db
        .run_tql(fast.tql.as_deref().unwrap(), true)
        .unwrap()
        .remove(0);
    assert_eq!(fast.rows, baseline.rows);
    assert_eq!(fast.rows, translated.rows);
    assert_eq!(fast.columns, translated.columns);
    assert_eq!(fast.rows.len(), 3);
    assert!(!fast.plan.is_empty());
}

#[test]
fn thousand_rows_fit_bounded_local_execution() {
    let mut db = Database::memory();
    create(&mut db);
    for start in (0..1000).step_by(100) {
        insert(
            &mut db,
            (start..start + 100)
                .map(|id| vec![Value::Int(id), Value::Text(format!("item-{id}"))])
                .collect(),
        )
        .unwrap();
    }
    let query = "SELECT COUNT(*) AS count, SUM(id) AS total FROM items WHERE id >= 500";
    let fast = db.run_sql(query, true).unwrap().remove(0);
    let baseline = db.run_sql(query, false).unwrap().remove(0);
    assert_eq!(fast.rows, vec![vec![Value::Int(500), Value::Int(374750)]]);
    assert_eq!(fast.rows, baseline.rows);
}

#[test]
fn update_and_delete_survive_restart() {
    let directory = Directory::new();
    {
        let mut db = Database::open(&directory.0).unwrap();
        create(&mut db);
        insert(
            &mut db,
            vec![
                vec![Value::Int(1), Value::Null],
                vec![Value::Int(2), Value::Null],
            ],
        )
        .unwrap();
        db.run_sql(
            "UPDATE items SET label = 'kept' WHERE id = 1; DELETE FROM items WHERE id = 2;",
            true,
        )
        .unwrap();
    }
    assert_eq!(
        all(&mut Database::open(&directory.0).unwrap()),
        vec![vec![Value::Int(1), Value::Text("kept".into())]]
    );
}

#[test]
fn unknown_columns_fail_on_empty_tables() {
    let mut db = Database::memory();
    create(&mut db);
    assert!(db.run_sql("SELECT missing FROM items", true).is_err());
    assert!(db
        .run_sql("UPDATE items SET label = missing", true)
        .is_err());
    assert!(db
        .run_sql("DELETE FROM items WHERE missing = 1", true)
        .is_err());
}
