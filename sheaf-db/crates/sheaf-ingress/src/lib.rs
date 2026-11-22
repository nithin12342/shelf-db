// lib.rs - unified ingress crate (C-09).
//
// Plan phase D names one crate for Tokio, Tonic, and Arrow Flight ingress.
// This crate is the composition root. It re-exports both adapters behind
// opt-in features. The adapters keep their own parity tests.

/// gRPC ingress from sheaf-runtime.
#[cfg(feature = "grpc")]
pub mod grpc {
    pub use sheaf_runtime::grpc::{pb, serve, GrpcQueryService};
}

/// Arrow Flight ingress from sheaf-app.
#[cfg(feature = "flight")]
pub mod flight {
    pub use sheaf_app::flight::{serve, RowStore, SheafFlightService};
}

/// The source of each adapter for an audit line.
pub fn sources() -> &'static [&'static str] {
    &["sheaf-runtime::grpc", "sheaf-app::flight"]
}
