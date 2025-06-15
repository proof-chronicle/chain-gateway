use tonic::{transport::Server, Request, Response, Status};
use proto::chain_gateway_server::{ChainGateway, ChainGatewayServer};
use proto::{StoreRequest, StoreResponse};
use std::env;

pub mod proto {
    tonic::include_proto!("chain_gateway");
}

pub mod providers;
mod blockchain;

use providers::SolanaProvider;
use blockchain::{BlockchainProvider, SolanaConfig};

pub struct MyChainGateway {
    provider: SolanaProvider,
}

impl Default for MyChainGateway {
    fn default() -> Self {
        let config = SolanaConfig {
            rpc_url: env::var("SOLANA_RPC_URL")
                .expect("SOLANA_RPC_URL must be set"),
            program_id: env::var("SOLANA_PROGRAM_ID")
                .expect("SOLANA_PROGRAM_ID must be set"),
            keypair_path: env::var("SOLANA_KEYPAIR_PATH")
                .expect("SOLANA_KEYPAIR_PATH must be set"),
            proof_account_keypair_path: env::var("SOLANA_PROOF_ACCOUNT_KEYPAIR_PATH")
                .expect("SOLANA_PROOF_ACCOUNT_KEYPAIR_PATH must be set"),
        };

        let provider = SolanaProvider::new(config)
            .expect("Failed to initialize Solana provider");

        Self { provider }
    }
}

#[tonic::async_trait]
impl ChainGateway for MyChainGateway {
    async fn store(&self, request: Request<StoreRequest>) -> Result<Response<StoreResponse>, Status> {
        println!("Received StoreRequest: {:?}", request);

        let record = match &request.get_ref().record {
            Some(record) => record,
            None => return Err(Status::invalid_argument("Record is missing")),
        };

        // Call provider to store the record
        match self.provider.store_record(record).await {
            Ok(result) => {
                let response = StoreResponse {
                    success: true,
                    transaction_id: result.transaction_id.clone(),
                    account_address: result.transaction_id, // Using transaction ID as account address for now
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                eprintln!("Blockchain transaction failed: {}", e);
                Err(Status::internal("Failed to store on blockchain"))
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables from .env file
    dotenv::dotenv().ok();

    let addr = "0.0.0.0:50051".parse()?;
    let service = MyChainGateway::default();

    println!("ChainGateway gRPC server listening on {}", addr);

    Server::builder()
        .add_service(ChainGatewayServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}