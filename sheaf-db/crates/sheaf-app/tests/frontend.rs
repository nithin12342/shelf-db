use sheaf_app::{model::{Aggregate, BinaryOp, DataType, Expr, Query, Statement, Value, MAX_REQUEST_BYTES}, sql, tql};

fn query(text: &str) -> Query {
    let statements = sql::parse(text).unwrap_or_else(|e| panic!("{text}: {e}"));
    let [Statement::Query(query)] = statements.as_slice() else { panic!("expected one query"); };
    query.clone()
}

fn roundtrip(text: &str) -> Query {
    let expected = query(text);
    let rendered = tql::render(&expected);
    assert!(rendered.starts_with("MATCH ROWS "));
    assert!(!rendered.contains("SOLVE"));
    let actual = tql::parse(&rendered).unwrap_or_else(|e| panic!("{rendered}: {e}"));
    assert_eq!(expected, actual, "{rendered}");
    assert_eq!(tql::render(&actual), rendered);
    actual
}

#[test]
fn create_types_and_nullability() {
    let statements = sql::parse("CREATE TABLE Items (ID INT NOT NULL, n BIGINT, x DOUBLE, y FLOAT, s TEXT NULL, v VARCHAR, b BOOLEAN)").unwrap();
    let Statement::Create { table, columns } = &statements[0] else { panic!(); };
    assert_eq!(table, "items");
    assert_eq!(columns.iter().map(|c| c.data_type).collect::<Vec<_>>(),
        vec![DataType::Int, DataType::Int, DataType::Float, DataType::Float, DataType::Text, DataType::Text, DataType::Bool]);
    assert_eq!(columns[0].name, "id");
    assert!(!columns[0].nullable);
    assert!(columns[1..].iter().all(|c| c.nullable));
}

#[test]
fn insert_typed_literals_columns_and_statement_order() {
    let statements = sql::parse("INSERT INTO T (ID, s, b, x) VALUES (-9223372036854775808, 'a, b''s; ()', TRUE, -1.25e2), (2, NULL, FALSE, +2.0); DELETE FROM T;").unwrap();
    let Statement::Insert { table, columns, rows } = &statements[0] else { panic!(); };
    assert_eq!(table, "t");
    assert_eq!(columns, &["id", "s", "b", "x"]);
    assert_eq!(rows[0], vec![Value::Int(i64::MIN), Value::Text("a, b's; ()".into()), Value::Bool(true), Value::Float(-125.0)]);
    assert_eq!(rows[1], vec![Value::Int(2), Value::Null, Value::Bool(false), Value::Float(2.0)]);
    assert!(matches!(statements[1], Statement::Delete { filter: None, .. }));
    assert!(matches!(&sql::parse("INSERT INTO t VALUES (1)").unwrap()[0], Statement::Insert { columns, .. } if columns.is_empty()));
}

#[test]
fn update_and_delete_filters() {
    let statements = sql::parse("UPDATE T SET x = x + 2 * 3, name = NULL WHERE NOT (id < 3 OR name IS NULL); DELETE FROM T WHERE id >= 7 AND name IS NOT NULL").unwrap();
    let Statement::Update { table, assignments, filter } = &statements[0] else { panic!(); };
    assert_eq!(table, "t");
    assert_eq!(assignments.len(), 2);
    assert!(matches!(assignments[0].1, Expr::Binary { op: BinaryOp::Add, .. }));
    assert_eq!(assignments[1], ("name".into(), Expr::Literal(Value::Null)));
    assert!(matches!(filter, Some(Expr::Not(_))));
    assert!(matches!(&statements[1], Statement::Delete { filter: Some(Expr::Binary { op: BinaryOp::And, .. }), .. }));
}

#[test]
fn roundtrip_all_query_clauses() {
    let q = roundtrip("SELECT DISTINCT a.team AS team, COUNT(*) AS n, COUNT(DISTINCT b.id) AS ids, SUM(b.x + 1) AS total, AVG(b.x), MIN(b.x), MAX(b.x) FROM Teams AS a INNER JOIN Scores b ON a.id = b.team_id WHERE (b.x >= 1.5 AND NOT b.deleted) OR b.x IS NULL GROUP BY a.team ORDER BY total DESC NULLS LAST, team ASC NULLS FIRST LIMIT 10 OFFSET 2");
    assert!(q.distinct);
    assert_eq!(q.limit, Some(10));
    assert_eq!(q.offset, 2);
    assert_eq!(q.group_by.len(), 1);
    assert_eq!(q.order_by.len(), 2);
    assert!(matches!(q.projection[1].expr, Expr::Aggregate { function: Aggregate::Count, expr: None, distinct: false }));
    assert!(matches!(q.projection[2].expr, Expr::Aggregate { distinct: true, .. }));
}

#[test]
fn quoting_preserves_case_delimiters_and_unicode() {
    let q = roundtrip(r#"SELECT "a"."Mixed" AS "q""uote", 'EMIT ROWS { a, b }; O''Brien', 'line
break', "a"."雪" FROM "Table: JOIN" AS "a" WHERE "a"."Mixed" <> 'WHERE; GROUP BY (x)'"#);
    assert_eq!(q.from.table, "Table: JOIN");
    assert_eq!(q.projection[0].alias.as_deref(), Some("q\"uote"));
    assert_eq!(q.projection[0].expr, Expr::Column { relation: Some("a".into()), name: "Mixed".into() });
    let q = roundtrip("SELECT MiXeD AS UPPER FROM MyTable AS ALIAS");
    assert_eq!(q.from.table, "mytable");
    assert_eq!(q.from.alias, "alias");
    assert_eq!(q.projection[0].alias.as_deref(), Some("upper"));
}

#[test]
fn wildcard_and_join_multiplicity_are_explicit() {
    let q = roundtrip("SELECT *, a.*, b.id, c.id FROM t a JOIN t b ON a.id = b.id JOIN t c ON b.id = c.id AND b.kind = c.kind");
    assert!(!q.distinct);
    assert_eq!(q.joins.len(), 2);
    assert_eq!(q.joins[0].relation.alias, "b");
    assert_eq!(q.joins[1].relation.alias, "c");
    assert_eq!(q.projection[0].expr, Expr::Column { relation: None, name: "*".into() });
    assert_eq!(q.projection[1].expr, Expr::Column { relation: Some("a".into()), name: "*".into() });
}

#[test]
fn null_logic_and_arithmetic_roundtrip() {
    for text in [
        "SELECT NULL, TRUE, FALSE, -9223372036854775808, 9223372036854775807 FROM t",
        "SELECT -0.0, 1.0, 1e30, 1e-30, 5e-324 FROM t",
        "SELECT a - (b - c), a / (b * c), -a, +a, -(a + 2), a + b * c FROM t",
        "SELECT a IS NULL, a IS NOT NULL, NOT (a = NULL OR b <> NULL) FROM t WHERE a = NULL",
        "SELECT a FROM t WHERE a <= 1 AND b > 2 OR NOT c = 3 ORDER BY a LIMIT 0 OFFSET 0",
        "SELECT a FROM t OFFSET 4",
    ] { roundtrip(text); }
}

#[test]
fn explain_preserves_analyze() {
    for (text, expected) in [("EXPLAIN SELECT * FROM t", false), ("EXPLAIN ANALYZE SELECT * FROM t", true)] {
        let statements = sql::parse(text).unwrap();
        let Statement::Explain { query, analyze } = &statements[0] else { panic!(); };
        assert_eq!(*analyze, expected);
        assert_eq!(tql::parse(&tql::render(query)).unwrap(), *query);
    }
}

#[test]
fn handwritten_tql_is_not_a_sql_wrapper() {
    let text = r#"match rows ("u": "users")
        join rows ("p": "posts") on "u"."id" = "p"."user"
        where "p"."title" <> 'a, b; } EMIT' and "p"."deleted" is null
        group by "u"."name"
        emit distinct rows { "u"."name" as "Name", COUNT(*) AS "n" }
        order by "n" desc nulls last limit 3 offset 1;"#;
    let q = tql::parse(text).unwrap();
    assert_eq!(q.from.table, "users");
    assert_eq!(q.joins[0].relation.table, "posts");
    assert_eq!(q, tql::parse(&tql::render(&q)).unwrap());
    assert!(tql::parse("SELECT * FROM users").is_err());
}

#[test]
fn reject_unsupported_sql_query_shapes() {
    for text in [
        "SELECT * FROM a LEFT JOIN b ON a.id = b.id",
        "SELECT * FROM a RIGHT JOIN b ON a.id = b.id",
        "SELECT * FROM a FULL JOIN b ON a.id = b.id",
        "SELECT * FROM a CROSS JOIN b", "SELECT * FROM a NATURAL JOIN b",
        "SELECT * FROM a JOIN b USING (id)", "SELECT * FROM a JOIN b ON a.id > b.id",
        "SELECT * FROM a, b", "SELECT * FROM (SELECT * FROM t) q",
        "SELECT (SELECT id FROM t) FROM t", "SELECT * FROM t UNION SELECT * FROM t",
        "WITH q AS (SELECT * FROM t) SELECT * FROM q", "SELECT 1",
        "SELECT * FROM t FOR UPDATE", "SELECT * FROM t FETCH FIRST 1 ROW ONLY",
        "SELECT TOP 1 * FROM t", "SELECT DISTINCT ON (a) a FROM t",
        "SELECT a INTO other FROM t", "SELECT SUM(a) FROM t HAVING SUM(a) > 1",
        "SELECT a FROM t GROUP BY ALL", "SELECT a FROM t GROUP BY a WITH ROLLUP",
        "SELECT a FROM t ORDER BY ALL", "SELECT a FROM t ORDER BY a WITH FILL",
        "SELECT * EXCLUDE (a) FROM t", "SELECT * EXCEPT (a) FROM t",
        "SELECT * FROM t AS x(a)", "SELECT * FROM schema.t", "SELECT * FROM generate_series(1, 2)",
        "SELECT a FROM t ORDER BY 1", "SELECT a FROM t GROUP BY 1",
        "SELECT a FROM t LIMIT 1 BY a", "SELECT a FROM t LIMIT 1, 2",
        "SELECT a FROM t LIMIT -1", "SELECT a FROM t LIMIT 1.5", "SELECT a FROM t OFFSET -2",
        "SELECT a FROM t LIMIT 184467440737095516160", "SELECT * FROM t x JOIN t x ON x.a = x.a",
        "EXPLAIN VERBOSE SELECT * FROM t", "EXPLAIN (ANALYZE) SELECT * FROM t",
        "EXPLAIN DELETE FROM t", "EXPLAIN QUERY PLAN SELECT * FROM t", "DROP TABLE t",
    ] { assert!(sql::parse(text).is_err(), "accepted {text}"); }
}

#[test]
fn reject_unsupported_expressions_and_aggregates() {
    for text in [
        "SELECT SUM(a) OVER () FROM t", "SELECT SUM(a) FILTER (WHERE a > 1) FROM t",
        "SELECT SUM(a ORDER BY b) FROM t", "SELECT SUM(SUM(a)) FROM t",
        "SELECT COUNT(DISTINCT *) FROM t", "SELECT SUM(*) FROM t", "SELECT COUNT(t.*) FROM t",
        "SELECT COUNT(a, b) FROM t", "SELECT COUNT() FROM t", "SELECT ABS(a) FROM t",
        "SELECT CAST(a AS TEXT) FROM t", "SELECT CASE WHEN a THEN b ELSE c END FROM t",
        "SELECT a IN (1, 2) FROM t", "SELECT a BETWEEN 1 AND 2 FROM t", "SELECT a LIKE 'x' FROM t",
        "SELECT a % 2 FROM t", "SELECT a || b FROM t", "SELECT ? FROM t",
        "SELECT a FROM t WHERE COUNT(*) > 1", "SELECT a FROM t GROUP BY SUM(a)",
        "SELECT 1e999 FROM t", "SELECT -1e999 FROM t", "SELECT 9223372036854775808 FROM t",
        "SELECT -9223372036854775809 FROM t", "SELECT SUM(a) IGNORE NULLS FROM t",
    ] { assert!(sql::parse(text).is_err(), "accepted {text}"); }
}

#[test]
fn reject_write_features_that_model_cannot_represent() {
    for text in [
        "CREATE TABLE IF NOT EXISTS t (a INT)", "CREATE TEMP TABLE t (a INT)",
        "CREATE TABLE t (a INT PRIMARY KEY)", "CREATE TABLE t (a INT, PRIMARY KEY(a))",
        "CREATE TABLE t (a INT DEFAULT 1)", "CREATE TABLE t (a INT CHECK (a > 0))",
        "CREATE TABLE t (a INT UNIQUE)", "CREATE TABLE t (a INT REFERENCES other(id))",
        "CREATE TABLE t (a TEXT COLLATE en)", "CREATE TABLE t (a INT NOT NULL NULL)",
        "CREATE TABLE t (a INT, A INT)", "CREATE TABLE t (a DECIMAL(10,2))",
        "CREATE TABLE t (a VARCHAR(20))", "CREATE TABLE t (a FLOAT(20))",
        "CREATE TABLE t AS SELECT * FROM x", "CREATE TABLE t (a INT) WITHOUT ROWID",
        "INSERT INTO t SELECT * FROM x", "INSERT INTO t DEFAULT VALUES",
        "INSERT INTO t VALUES (1 + 2)", "INSERT INTO t VALUES (a)",
        "INSERT INTO t (a, a) VALUES (1, 2)", "INSERT INTO t (a, b) VALUES (1)",
        "INSERT INTO t VALUES (1), (1, 2)", "INSERT INTO t VALUES (1e999)",
        "INSERT INTO t VALUES (1) RETURNING a", "INSERT OR IGNORE INTO t VALUES (1)",
        "INSERT INTO t VALUES (1) ON CONFLICT DO NOTHING", "UPDATE t SET a = 1 RETURNING a",
        "UPDATE t SET a = x.a FROM x", "UPDATE t SET a = 1, a = 2",
        "UPDATE t x SET a = 1", "UPDATE t SET (a, b) = (1, 2)",
        "DELETE FROM t RETURNING a", "DELETE FROM t USING x", "DELETE FROM t LIMIT 1",
        "DELETE FROM t ORDER BY a", "DELETE FROM t x", "DELETE FROM t, x",
    ] { assert!(sql::parse(text).is_err(), "accepted {text}"); }
}

#[test]
fn reject_unknown_or_injected_tql() {
    for text in [
        r#"MATCH ROWS ("t": "t") EMIT ROWS { "a" }"#,
        r#"MATCH ROWS (t: "t") EMIT ROWS { "a" };"#,
        r#"MATCH ROWS ("t": "t") EMIT ROWS { a };"#,
        r#"MATCH ROWS ("t": "t") EMIT ROWS { "a" "alias" };"#,
        r#"MATCH ROWS ("t": "t") EMIT ROWS { "a" FROM "other" };"#,
        r#"MATCH ROWS ("t": "t") EMIT ROWS { "a", };"#,
        r#"MATCH ROWS ("t": "t") EMIT ROWS { };"#,
        r#"MATCH ROWS ("t": "t") WHERE EMIT ROWS { * };"#,
        r#"MATCH ROWS ("t": "t") EMIT ROWS { * } WHERE TRUE;"#,
        r#"MATCH ROWS ("t": "t") EMIT ROWS { COUNT(*) OVER () };"#,
        r#"MATCH ROWS ("t": "t") EMIT ROWS { * } LIMIT -1;"#,
        r#"MATCH ROWS ("t": "t") EMIT ROWS { * } OFFSET 1 LIMIT 2;"#,
        r#"MATCH ROWS ("t": "t") EMIT ROWS { * }; DROP TABLE t;"#,
        r#"MATCH ROWS ("t": "t") LEFT JOIN ROWS ("b": "b") ON "t"."x" = "b"."x" EMIT ROWS { * };"#,
        r#"MATCH ROWS ("t": "t") SOLVE HARMONIC EMIT ROWS { * };"#,
    ] { assert!(tql::parse(text).is_err(), "accepted {text}"); }
}

#[test]
fn request_and_nesting_limits_ignore_quoted_text() {
    assert!(sql::parse(&"x".repeat(MAX_REQUEST_BYTES + 1)).unwrap_err().contains("bytes"));
    assert!(tql::parse(&"x".repeat(MAX_REQUEST_BYTES + 1)).unwrap_err().contains("bytes"));
    let deep = format!("SELECT {}a{} FROM t", "(".repeat(65), ")".repeat(65));
    assert!(sql::parse(&deep).unwrap_err().contains("nesting"));
    let shallow = format!("SELECT {}a{} FROM t", "(".repeat(64), ")".repeat(64));
    assert!(sql::parse(&shallow).is_ok());
    assert!(sql::parse(&format!("SELECT {}a FROM t", "NOT ".repeat(65))).is_err());
    assert!(sql::parse(&format!("SELECT {}1 FROM t", "1 + ".repeat(5000))).is_err());
    let text = format!("SELECT '{}', \"{}\" FROM t", "(".repeat(100), ")".repeat(100));
    roundtrip(&text);
    roundtrip("SELECT a /* ignored ( */ FROM t -- ignored )\n WHERE a = '/* ; */'");
    assert!(sql::parse(" ").is_err());
    assert!(sql::parse("SELECT a FROM t; UNKNOWN command").is_err());
}
