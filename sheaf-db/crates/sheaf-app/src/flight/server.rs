// flight/server.rs - the Tonic-backed Arrow Flight server (NF-09).
//
// do_get and do_put are implemented on the real gRPC path. The other Flight
// RPCs return Status::unimplemented with a named cause, so no caller receives a
// fake success for a method this adapter does not provide.
// Tonic's Status is a large error by design, and it is the generated contract.
#![allow(clippy::result_large_err)]

use super::producer::RowStore;
use arrow::array::{ArrayRef, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use arrow_flight::decode::{DecodedPayload, FlightDataDecoder};
use arrow_flight::encode::FlightDataEncoderBuilder;
use arrow_flight::error::FlightError;
use arrow_flight::flight_service_server::FlightService;
use arrow_flight::{
    Action, ActionType, Criteria, Empty, FlightData, FlightDescriptor, FlightInfo, HandshakeRequest,
    HandshakeResponse, PollInfo, PutResult, Result as FlightResult, SchemaResult, Ticket,
};
use futures::StreamExt;
use std::pin::Pin;
use std::sync::Arc;
use tonic::{Request, Response, Status, Streaming};

type HandshakeStream = Pin<Box<dyn futures::Stream<Item = Result<HandshakeResponse, Status>> + Send>>;
type FlightStream = Pin<Box<dyn futures::Stream<Item = Result<FlightData, Status>> + Send>>;
type InfoStream = Pin<Box<dyn futures::Stream<Item = Result<FlightInfo, Status>> + Send>>;
type PutStream = Pin<Box<dyn futures::Stream<Item = Result<PutResult, Status>> + Send>>;
type ActionStream = Pin<Box<dyn futures::Stream<Item = Result<FlightResult, Status>> + Send>>;
type ActionTypeStream = Pin<Box<dyn futures::Stream<Item = Result<ActionType, Status>> + Send>>;

/// A flight service backed by one row store.
pub struct SheafFlightService {
    store: RowStore,
}

impl SheafFlightService {
    pub fn new(store: RowStore) -> Self {
        Self { store }
    }

    fn batch(rows: &[String]) -> Result<RecordBatch, Status> {
        let schema = Arc::new(Schema::new(vec![Field::new("value", DataType::Utf8, false)]));
        let values: ArrayRef = Arc::new(StringArray::from(rows.to_vec()));
        RecordBatch::try_new(schema, vec![values]).map_err(|error| Status::internal(error.to_string()))
    }

    fn unsupported(name: &str) -> Status {
        Status::unimplemented(format!("Sheaf Flight does not implement {name}"))
    }
}

#[tonic::async_trait]
impl FlightService for SheafFlightService {
    type HandshakeStream = HandshakeStream;
    type ListFlightsStream = InfoStream;
    type DoGetStream = FlightStream;
    type DoPutStream = PutStream;
    type DoExchangeStream = FlightStream;
    type DoActionStream = ActionStream;
    type ListActionsStream = ActionTypeStream;

    async fn handshake(
        &self,
        _request: Request<Streaming<HandshakeRequest>>,
    ) -> Result<Response<Self::HandshakeStream>, Status> {
        Err(Self::unsupported("handshake"))
    }

    async fn list_flights(
        &self,
        _request: Request<Criteria>,
    ) -> Result<Response<Self::ListFlightsStream>, Status> {
        Err(Self::unsupported("list_flights"))
    }

    async fn get_flight_info(
        &self,
        _request: Request<FlightDescriptor>,
    ) -> Result<Response<FlightInfo>, Status> {
        Err(Self::unsupported("get_flight_info"))
    }

    async fn poll_flight_info(
        &self,
        _request: Request<FlightDescriptor>,
    ) -> Result<Response<PollInfo>, Status> {
        Err(Self::unsupported("poll_flight_info"))
    }

    async fn get_schema(
        &self,
        _request: Request<FlightDescriptor>,
    ) -> Result<Response<SchemaResult>, Status> {
        Err(Self::unsupported("get_schema"))
    }

    async fn do_get(&self, _request: Request<Ticket>) -> Result<Response<Self::DoGetStream>, Status> {
        let rows = self.store.snapshot();
        let batch = Self::batch(&rows)?;
        let stream = FlightDataEncoderBuilder::new()
            .build(futures::stream::once(async move { Ok(batch) }))
            .map(|item| item.map_err(|error| Status::internal(error.to_string())));
        Ok(Response::new(Box::pin(stream) as Self::DoGetStream))
    }

    async fn do_put(
        &self,
        request: Request<Streaming<FlightData>>,
    ) -> Result<Response<Self::DoPutStream>, Status> {
        let incoming = request.into_inner().map(|item| {
            item.map_err(|error| FlightError::DecodeError(format!("transport: {error}")))
        });
        let mut decoder = FlightDataDecoder::new(incoming);
        while let Some(item) = decoder.next().await {
            let decoded = item.map_err(|error| Status::internal(error.to_string()))?;
            if let DecodedPayload::RecordBatch(batch) = decoded.payload {
                let column = batch
                    .column(0)
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .ok_or_else(|| Status::invalid_argument("do_put requires a string column"))?;
                let rows: Vec<String> = column.iter().flatten().map(str::to_owned).collect();
                self.store.extend(rows);
            }
        }
        let ack = futures::stream::once(async { Ok(PutResult::default()) });
        Ok(Response::new(Box::pin(ack) as Self::DoPutStream))
    }

    async fn do_exchange(
        &self,
        _request: Request<Streaming<FlightData>>,
    ) -> Result<Response<Self::DoExchangeStream>, Status> {
        Err(Self::unsupported("do_exchange"))
    }

    async fn do_action(
        &self,
        _request: Request<Action>,
    ) -> Result<Response<Self::DoActionStream>, Status> {
        Err(Self::unsupported("do_action"))
    }

    async fn list_actions(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<Self::ListActionsStream>, Status> {
        Err(Self::unsupported("list_actions"))
    }
}

/// Serve the Flight service on the given address.
pub async fn serve(address: std::net::SocketAddr, store: RowStore) -> Result<(), tonic::transport::Error> {
    let service = SheafFlightService::new(store);
    tonic::transport::Server::builder()
        .add_service(arrow_flight::flight_service_server::FlightServiceServer::new(service))
        .serve(address)
        .await
}
