use tonic::{transport::Server, Request, Response, Status};
use proto::chain_gateway_server::{ChainGateway, ChainGatewayServer};
use proto::{StoreRequest, StoreResponse, RetrieveRequest, RetrieveResponse, ContentRecord};

pub mod proto {
    tonic::include_proto!("chain_gateway");
}

#[derive(Debug, Default)]
pub struct MyChainGateway;

#[tonic::async_trait]
impl ChainGateway for MyChainGateway {
    async fn store(&self, request: Request<StoreRequest>) -> Result<Response<StoreResponse>, Status> {
        println!("Received StoreRequest: {:?}", request);

        // Example: simulate transaction_id generation
        let transaction_id = match &request.get_ref().record {
            Some(record) => format!("tx_{}", record.uid),
            None => return Err(Status::invalid_argument("Record is missing")),
        };

        let response = StoreResponse {
            success: true,
            transaction_id,
        };

        Ok(Response::new(response))
    }

    async fn retrieve(&self, request: Request<RetrieveRequest>) -> Result<Response<RetrieveResponse>, Status> {
        println!("Received RetrieveRequest: {:?}", request);

        // Example: return a dummy record
        let dummy_record = ContentRecord {
            uid: "dummy_uid".into(),
            created_at: "2025-05-03T12:00:00Z".into(),
            hash: "dummy_hash".into(),
            url: "https://example.com".into(),
        };

        let response = RetrieveResponse {
            record: Some(dummy_record),
        };

        Ok(Response::new(response))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "0.0.0.0:50051".parse()?;
    let service = MyChainGateway::default();

    println!("ChainGateway gRPC server listening on {}", addr);

    Server::builder()
        .add_service(ChainGatewayServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}