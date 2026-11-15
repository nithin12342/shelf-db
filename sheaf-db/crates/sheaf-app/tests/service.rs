use sheaf_app::model::{Value, MAX_REQUEST_BYTES};
use sheaf_app::service::{Application, Response};
use sheaf_app::Database;
use sheaf_runtime::ingress::RequestHandler;

fn request(app: &Application, language: &str, query: &str) -> Response {
    let bytes =
        serde_json::to_vec(&serde_json::json!({"language": language, "query": query})).unwrap();
    serde_json::from_slice(&app.handle(&bytes)).unwrap()
}

#[test]
fn requests_reach_the_same_database() {
    let app = Application::new(Database::memory());
    assert!(request(&app, "sql", "CREATE TABLE sample (id BIGINT, name TEXT)").ok);
    assert!(
        request(
            &app,
            "sql",
            "INSERT INTO sample VALUES (1, 'first'), (2, NULL)"
        )
        .ok
    );
    let result = request(&app, "sql", "SELECT id, name FROM sample ORDER BY id");
    assert!(result.ok, "{:?}", result.error);
    assert_eq!(
        result.results[0].rows,
        vec![
            vec![Value::Int(1), Value::Text("first".into())],
            vec![Value::Int(2), Value::Null]
        ]
    );
    let translated = request(&app, "tql", result.results[0].tql.as_ref().unwrap());
    assert!(translated.ok, "{:?}", translated.error);
    assert_eq!(result.results[0].rows, translated.results[0].rows);
    let spaced = result.results[0].tql.as_ref().unwrap().replacen("MATCH ROWS", "-- query\nMATCH\n ROWS", 1);
    let translated = request(&app, "tql", &spaced);
    assert!(translated.ok, "{:?}", translated.error);
    assert_eq!(result.results[0].rows, translated.results[0].rows);
}

#[test]
fn invalid_requests_return_errors_without_poisoning_service() {
    let app = Application::new(Database::memory());
    let malformed: Response = serde_json::from_slice(&app.handle(b"{")).unwrap();
    assert!(!malformed.ok);
    assert!(!request(&app, "javascript", "anything").ok);
    assert!(!request(&app, "sql", "SELECT missing FROM absent").ok);
    assert!(request(&app, "sql", "CREATE TABLE alive (id BIGINT)").ok);
}

#[test]
fn large_packets_fail_before_json_parsing() {
    let app = Application::new(Database::memory());
    let result: Response =
        serde_json::from_slice(&app.handle(&vec![b' '; MAX_REQUEST_BYTES + 1])).unwrap();
    assert!(!result.ok);
    assert!(result.error.unwrap().contains("byte limit"));
}

#[test]
fn failed_write_leaves_existing_rows_available() {
    let app = Application::new(Database::memory());
    assert!(request(&app, "sql", "CREATE TABLE required (id BIGINT NOT NULL)").ok);
    assert!(request(&app, "sql", "INSERT INTO required VALUES (1)").ok);
    assert!(!request(&app, "sql", "INSERT INTO required VALUES (NULL)").ok);
    let result = request(&app, "sql", "SELECT id FROM required");
    assert!(result.ok);
    assert_eq!(result.results[0].rows, vec![vec![Value::Int(1)]]);
}
