// flight.rs - integration test for the Arrow Flight daemon (NF-09).
//
// The test loads rows through the production SQL path, serves them through
// Arrow Flight, and checks do_get and do_put against the SQL rows. Every
// expected result comes from the SQL output or from the rows sent by do_put.

#![cfg(feature = "flight")]
#![allow(clippy::result_large_err)] // The generated Flight client returns Tonic Status.

use arrow::array::{ArrayRef, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use arrow_flight::decode::{DecodedPayload, FlightDataDecoder};
use arrow_flight::encode::FlightDataEncoderBuilder;
use arrow_flight::error::FlightError;
use arrow_flight::flight_service_client::FlightServiceClient;
use arrow_flight::Ticket;
use futures::StreamExt;
use sheaf_app::flight::{serve, RowStore};
use sheaf_app::model::Value;
use sheaf_app::Database;
use std::sync::Arc;

fn sql_rows() -> Vec<String> {
    let mut db = Database::memory();
    db.run_sql("CREATE TABLE names (name TEXT)", true).expect("create");
    db.run_sql(
        "INSERT INTO names VALUES ('gamma'); INSERT INTO names VALUES ('alpha'); INSERT INTO names VALUES ('beta');",
        true,
    )
    .expect("insert");
    let output = db.run_sql("SELECT name FROM names ORDER BY name", true).expect("select");
    output[0]
        .rows
        .iter()
        .map(|row| match &row[0] {
            Value::Text(text) => text.clone(),
            other => panic!("unexpected value {other:?}"),
        })
        .collect()
}

fn batch(rows: &[String]) -> RecordBatch {
    let schema = Arc::new(Schema::new(vec![Field::new("value", DataType::Utf8, false)]));
    let values: ArrayRef = Arc::new(StringArray::from(rows.to_vec()));
    RecordBatch::try_new(schema, vec![values]).expect("batch")
}

async fn fetch(client: &mut FlightServiceClient<tonic::transport::Channel>) -> Vec<String> {
    let reply = client
        .do_get(Ticket { ticket: "names".into() })
        .await
        .expect("do_get");
    let incoming = reply.into_inner().map(|item| {
        item.map_err(|error| FlightError::DecodeError(format!("transport: {error}")))
    });
    let mut decoder = FlightDataDecoder::new(incoming);
    let mut rows = Vec::new();
    while let Some(item) = decoder.next().await {
        let decoded = item.expect("decode");
        if let DecodedPayload::RecordBatch(record) = decoded.payload {
            let column = record.column(0).as_any().downcast_ref::<StringArray>().expect("string column");
            rows.extend(column.iter().flatten().map(str::to_owned));
        }
    }
    rows
}

#[tokio::test]
async fn do_get_returns_the_sql_rows_and_do_put_round_trips() {
    let expected = sql_rows();
    assert_eq!(expected, vec!["alpha", "beta", "gamma"]);

    let store = RowStore::new();
    store.set(expected.clone());
    let probe = std::net::TcpListener::bind("127.0.0.1:0").expect("probe");
    let address = probe.local_addr().expect("address");
    drop(probe);
    let server = tokio::spawn(serve(address, store.clone()));
    let mut client = None;
    for _ in 0..50 {
        match FlightServiceClient::connect(format!("http://{address}")).await {
            Ok(connected) => {
                client = Some(connected);
                break;
            }
            Err(_) => tokio::time::sleep(std::time::Duration::from_millis(20)).await,
        }
    }
    let mut client = client.expect("flight server did not start");

    assert_eq!(fetch(&mut client).await, expected);

    let uploaded = vec!["delta".to_string(), "epsilon".to_string()];
    let upload_rows = uploaded.clone();
    let stream = FlightDataEncoderBuilder::new()
        .build(futures::stream::once(async move { Ok(batch(&upload_rows)) }))
        .map(|item| item.expect("encode in memory"));
    client.do_put(tonic::Request::new(stream)).await.expect("do_put");

    let mut wanted = expected.clone();
    wanted.extend(uploaded);
    assert_eq!(fetch(&mut client).await, wanted);
    assert_eq!(store.snapshot(), wanted);
    server.abort();
}
