// flight/mod.rs - the Arrow Flight daemon (NF-09).
//
// The service returns columnar result sets through Arrow Flight. The same
// byte rows that a SQL result holds are encoded into a RecordBatch and sent
// through do_get. do_put accepts batches and stores their rows for a later
// do_get, so the round trip is checked on the real gRPC path.

pub mod producer;
pub mod server;

pub use producer::RowStore;
pub use server::{serve, SheafFlightService};
