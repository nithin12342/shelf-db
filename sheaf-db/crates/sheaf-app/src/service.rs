use crate::model::MAX_REQUEST_BYTES;
use crate::{Database, QueryOutput, Result};
use serde::{Deserialize, Serialize};
use sheaf_runtime::ingress::RequestHandler;
use std::cell::RefCell;

fn enabled() -> bool {
    true
}

#[derive(Serialize, Deserialize)]
pub struct Request {
    pub language: String,
    pub query: String,
    #[serde(default = "enabled")]
    pub optimize: bool,
}

#[derive(Serialize, Deserialize)]
pub struct Response {
    pub ok: bool,
    pub results: Vec<QueryOutput>,
    pub error: Option<String>,
}

pub struct Application {
    database: RefCell<Database>,
}

impl Application {
    pub fn new(database: Database) -> Self {
        Self {
            database: RefCell::new(database),
        }
    }

    pub fn execute(&self, request: &Request) -> Result<Vec<QueryOutput>> {
        if request.query.len() > MAX_REQUEST_BYTES {
            return Err("query exceeds request limit".into());
        }
        let mut database = self
            .database
            .try_borrow_mut()
            .map_err(|_| "database request already active")?;
        match request.language.as_str() {
            "sql" => database.run_sql(&request.query, request.optimize),
            "tql" => database.run_tql(&request.query, request.optimize),
            _ => Err("language must be sql or tql".into()),
        }
    }
}

impl RequestHandler for Application {
    fn handle(&self, bytes: &[u8]) -> Vec<u8> {
        let answer = if bytes.len() > MAX_REQUEST_BYTES {
            Err("request exceeds byte limit".into())
        } else {
            serde_json::from_slice::<Request>(bytes)
                .map_err(|e| e.to_string())
                .and_then(|request| self.execute(&request))
        };
        let response = match answer {
            Ok(results) => Response {
                ok: true,
                results,
                error: None,
            },
            Err(error) => Response {
                ok: false,
                results: Vec::new(),
                error: Some(error),
            },
        };
        let encoded = serde_json::to_vec(&response).unwrap_or_else(|_| {
            b"{\"ok\":false,\"results\":[],\"error\":\"result encoding failed\"}".to_vec()
        });
        if encoded.len() > 4 * 1024 * 1024 {
            b"{\"ok\":false,\"results\":[],\"error\":\"response exceeds 4 MiB. Use LIMIT\"}"
                .to_vec()
        } else {
            encoded
        }
    }
}
