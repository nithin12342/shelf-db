// Timestamp: 2026-11-22_11-53-08 +05:30
// grpc_parity.rs - framed TCP and gRPC return the same response bytes (NF-10).
//
// The test drives both adapters with the same production RequestHandler and
// the same 100 request payloads. It compares the response bytes case by case.

#![cfg(feature = "grpc")]

use sheaf_runtime::grpc::pb::query_service_client::QueryServiceClient;
use sheaf_runtime::grpc::pb::QueryRequest;
use sheaf_runtime::grpc::serve;
use sheaf_runtime::ingress::{round_trip, IngressServer, RequestHandler};

#[derive(Clone)]
struct Upper;
impl RequestHandler for Upper {
    fn handle(&self, request: &[u8]) -> Vec<u8> {
        let mut out = request.to_ascii_uppercase();
        out.reverse();
        out
    }
}

fn payloads() -> Vec<Vec<u8>> {
    (0..100).map(|i| format!("request {i} with a payload").into_bytes()).collect()
}

#[tokio::test]
async fn grpc_and_framed_tcp_return_identical_bytes() {
    // Reserve a free port, then let Tonic bind it.
    let probe = std::net::TcpListener::bind("127.0.0.1:0").expect("probe");
    let address = probe.local_addr().expect("address");
    drop(probe);
    let server = tokio::spawn(serve(address, Upper));
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
    let mut client = client.expect("gRPC server did not start");

    let framed = IngressServer::bind("127.0.0.1:0").expect("framed bind");
    let framed_address = framed.local_addr().expect("framed address");
    let framed_worker = std::thread::spawn(move || {
        for _ in 0..payloads().len() {
            framed.serve_one(&Upper).expect("serve one");
        }
    });

    for request in payloads() {
        let grpc_reply = client
            .execute(QueryRequest { payload: request.clone() })
            .await
            .expect("grpc execute")
            .into_inner()
            .payload;
        let framed_reply = round_trip(framed_address, &request).expect("framed round trip");
        assert_eq!(grpc_reply, framed_reply);
        let mut expected = request.to_ascii_uppercase();
        expected.reverse();
        assert_eq!(grpc_reply, expected);
    }
    framed_worker.join().expect("framed worker");
    server.abort();
}
