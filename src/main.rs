use tonic::{transport::Server, Request, Response, Status};
use proto::chain_gateway_server::{ChainGateway, ChainGatewayServer};
use proto::{StoreRequest, StoreResponse, RetrieveRequest, RetrieveResponse};
use std::env;
use std::sync::Arc;

mod blockchain;
mod providers;

use blockchain::{BlockchainProvider, ChainConfig, ChainType};
use providers::SolanaProvider;

pub mod proto {
    tonic::include_proto!("chain_gateway");
}

pub struct ChainGatewayService {
    blockchain: Arc<dyn BlockchainProvider>,
}

impl ChainGatewayService {
    pub fn new(blockchain: Arc<dyn BlockchainProvider>) -> Self {
        Self { blockchain }
    }
}

#[tonic::async_trait]
impl ChainGateway for ChainGatewayService {
    async fn store(&self, request: Request<StoreRequest>) -> Result<Response<StoreResponse>, Status> {
        println!("Received StoreRequest: {:?}", request);

        let record = match &request.get_ref().record {
            Some(record) => record,
            None => return Err(Status::invalid_argument("Record is missing")),
        };

        // Use the abstracted blockchain interface
        match self.blockchain.store_record(record).await {
            Ok(result) => {
                let response = StoreResponse {
                    success: true,
                    transaction_id: result.transaction_id,
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                eprintln!("Blockchain transaction failed: {}", e);
                Err(Status::internal("Failed to store on blockchain"))
            }
        }
    }

    async fn retrieve(&self, request: Request<RetrieveRequest>) -> Result<Response<RetrieveResponse>, Status> {
        println!("Received RetrieveRequest: {:?}", request);

        let transaction_id = &request.get_ref().transaction_id;

        match self.blockchain.retrieve_record(transaction_id).await {
            Ok(Some(record)) => {
                let response = RetrieveResponse {
                    record: Some(record),
                };
                Ok(Response::new(response))
            }
            Ok(None) => {
                Err(Status::not_found("Record not found"))
            }
            Err(e) => {
                eprintln!("Failed to retrieve record: {}", e);
                Err(Status::internal("Failed to retrieve from blockchain"))
            }
        }
    }
}

async fn create_blockchain_provider() -> Result<Arc<dyn BlockchainProvider>, Box<dyn std::error::Error>> {
    // Read configuration from environment variables
    let chain_type = env::var("CHAIN_TYPE").unwrap_or_else(|_| "solana".to_string());
    let network_url = env::var("BLOCKCHAIN_URL").unwrap_or_else(|_| "http://solana-validator:8899".to_string());
    let program_id = env::var("PROGRAM_ID").ok().or_else(|| {
        Some("6F8VF9413BrwBYLPndCbKTB74bbzDCdv335jToYzCA3D".to_string())
    });
    let private_key_path = env::var("PRIVATE_KEY_PATH").ok().or_else(|| {
        Some("/root/.config/solana/id.json".to_string())
    });

    let config = ChainConfig {
        network_url,
        program_id,
        private_key_path,
        chain_type: match chain_type.to_lowercase().as_str() {
            "solana" => ChainType::Solana,
            "ethereum" => ChainType::Ethereum,
            _ => return Err("Unsupported chain type".into()),
        },
    };

    println!("🔧 Initializing blockchain provider: {:?}", config.chain_type);

    match config.chain_type {
        ChainType::Solana => {
            let provider = SolanaProvider::new(config).map_err(|e| -> Box<dyn std::error::Error> { 
                format!("Failed to create Solana provider: {}", e).into() 
            })?;
            provider.initialize().await.map_err(|e| -> Box<dyn std::error::Error> { 
                format!("Failed to initialize Solana provider: {}", e).into() 
            })?;
            Ok(Arc::new(provider))
        }
        _ => {
            return Err("Unsupported chain type".into());
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables
    dotenv::dotenv().ok();

    let addr = "0.0.0.0:50051".parse()?;
    
    // Initialize blockchain provider
    let blockchain = create_blockchain_provider().await?;
    
    // Perform health check
    match blockchain.health_check().await {
        Ok(true) => println!("✅ Blockchain provider is healthy"),
        Ok(false) => println!("⚠️  Blockchain provider health check failed"),
        Err(e) => println!("❌ Blockchain provider health check error: {}", e),
    }

    // Get network info
    if let Ok(network_info) = blockchain.get_network_info().await {
        println!("🌐 Connected to: {} (Block: {})", 
                 network_info.network_name, 
                 network_info.block_height);
    }

    let service = ChainGatewayService::new(blockchain);

    println!("🚀 ChainGateway gRPC server listening on {}", addr);

    Server::builder()
        .add_service(ChainGatewayServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}