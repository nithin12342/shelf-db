// grpc/service.rs - the Tonic service and server (NF-10).
//
// The service receives a byte payload, calls the production RequestHandler,
// and returns the response bytes. No query logic lives here.

use crate::ingress::RequestHandler;

#[allow(clippy::result_large_err)] // Generated Tonic client methods carry a large Status value.
pub mod pb {
    tonic::include_proto!("sheaf");
}

/// A gRPC adapter over any request handler.
#[derive(Clone)]
pub struct GrpcQueryService<H>
where
    H: RequestHandler + Clone + Send + Sync + 'static,
{
    handler: H,
}

impl<H> GrpcQueryService<H>
where
    H: RequestHandler + Clone + Send + Sync + 'static,
{
    pub fn new(handler: H) -> Self {
        Self { handler }
    }
}

#[tonic::async_trait]
impl<H> pb::query_service_server::QueryService for GrpcQueryService<H>
where
    H: RequestHandler + Clone + Send + Sync + 'static,
{
    async fn execute(
        &self,
        request: tonic::Request<pb::QueryRequest>,
    ) -> Result<tonic::Response<pb::QueryReply>, tonic::Status> {
        let response = self.handler.handle(&request.into_inner().payload);
        Ok(tonic::Response::new(pb::QueryReply { payload: response }))
    }
}

/// Serve one handler on the given address until the process ends.
pub async fn serve<H>(address: std::net::SocketAddr, handler: H) -> Result<(), tonic::transport::Error>
where
    H: RequestHandler + Clone + Send + Sync + 'static,
{
    let service = GrpcQueryService::new(handler);
    tonic::transport::Server::builder()
        .add_service(pb::query_service_server::QueryServiceServer::new(service))
        .serve(address)
        .await
}
