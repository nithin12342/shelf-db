// grpc/mod.rs - the gRPC ingress adapter (NF-10).
//
// This adapter implements the same request and response contract as the framed
// TCP ingress. A caller depends on ingress::RequestHandler only; the transport
// changes without changing the handler.

pub mod service;

pub use service::{pb, serve, GrpcQueryService};
