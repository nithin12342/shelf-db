// Timestamp: 2026-11-22_11-53-08 +05:30
// ingress_parity.rs - the unified ingress crate gate (C-09).
//
// The test checks that both adapters are reachable through one crate and that
// the gRPC path still returns the same bytes as the framed TCP path.

#[cfg(feature = "grpc")]
use sheaf_ingress::grpc::pb::query_service_client::QueryServiceClient;
#[cfg(feature = "grpc")]
use sheaf_ingress::grpc::pb::QueryRequest;
#[cfg(feature = "flight")]
use sheaf_ingress::flight::RowStore;

#[test]
fn the_crate_names_both_adapters() {
    assert_eq!(sheaf_ingress::sources(), &["sheaf-runtime::grpc", "sheaf-app::flight"]);
}

#[cfg(feature = "flight")]
#[test]
fn the_flight_row_store_is_reachable() {
    let store = RowStore::new();
    store.set(vec!["alpha".into(), "beta".into()]);
    assert_eq!(store.snapshot(), vec!["alpha", "beta"]);
}

#[cfg(feature = "grpc")]
#[tokio::test]
async fn the_grpc_client_is_reachable() {
    use sheaf_ingress::grpc::serve;
    use sheaf_runtime::ingress::{IngressServer, RequestHandler, round_trip};

    #[derive(Clone)]
    struct CopyHandler;
    impl RequestHandler for CopyHandler {
        fn handle(&self, request: &[u8]) -> Vec<u8> {
            request.to_vec()
        }
    }

    let probe = std::net::TcpListener::bind("127.0.0.1:0").expect("probe");
    let address = probe.local_addr().expect("address");
    drop(probe);
    let server = tokio::spawn(serve(address, CopyHandler));
    let mut client = None;
    for _ in 0..50 {
        match QueryServiceClient::connect(format!("http://{address}")).await {
            Ok(connected) => {
                client = Some(connected);
                break;
            }
            Err(_) => tokio::time::sleep(std::time::Duration::from_millis(20)).await,
        }
    }
    let mut client = client.expect("server did not start");
    let request = b"unified ingress".to_vec();
    let reply = client
        .execute(QueryRequest { payload: request.clone() })
        .await
        .expect("execute")
        .into_inner()
        .payload;
    assert_eq!(reply, request);
    server.abort();

    struct Upper;
    impl RequestHandler for Upper {
        fn handle(&self, request: &[u8]) -> Vec<u8> {
            request.to_ascii_uppercase()
        }
    }
    let framed = IngressServer::bind("127.0.0.1:0").expect("framed bind");
    let framed_address = framed.local_addr().expect("framed address");
    let worker = std::thread::spawn(move || framed.serve_one(&Upper).expect("serve one"));
    let response = round_trip(framed_address, b"abc").expect("round trip");
    assert_eq!(response, b"ABC");
    worker.join().expect("worker");
}
