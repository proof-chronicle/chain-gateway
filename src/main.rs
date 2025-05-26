use tonic::{transport::Server, Request, Response, Status};
use proto::chain_gateway_server::{ChainGateway, ChainGatewayServer};
use proto::{StoreRequest, StoreResponse, RetrieveRequest, RetrieveResponse, ContentRecord};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    instruction::Instruction,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
    commitment_config::CommitmentConfig,
};
use std::str::FromStr;
use std::time::Duration;
use std::path::Path;

pub mod proto {
    tonic::include_proto!("chain_gateway");
}

pub struct MyChainGateway {
    solana_client: RpcClient,
    program_id: Pubkey,
    payer: Keypair,
}

// Manual Debug implementation since RpcClient doesn't implement Debug
impl std::fmt::Debug for MyChainGateway {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MyChainGateway")
            .field("program_id", &self.program_id)
            .field("payer_pubkey", &self.payer.pubkey())
            .finish()
    }
}

impl Default for MyChainGateway {
    fn default() -> Self {
        let solana_client = RpcClient::new_with_commitment(
            "http://solana-validator:8899".to_string(),
            CommitmentConfig::confirmed(),
        );
        let program_id = Pubkey::from_str("6F8VF9413BrwBYLPndCbKTB74bbzDCdv335jToYzCA3D")
            .expect("Invalid program ID");
        
        // Load the existing keypair from the mounted volume
        let keypair_path = Path::new("/root/.config/solana/id.json");
        let payer = if keypair_path.exists() {
            match std::fs::read(keypair_path) {
                Ok(keypair_bytes) => {
                    match Keypair::from_bytes(&keypair_bytes) {
                        Ok(keypair) => {
                            println!("🔑 Loaded existing keypair: {}", keypair.pubkey());
                            keypair
                        }
                        Err(e) => {
                            eprintln!("Failed to parse keypair: {}", e);
                            println!("🔑 Generating new keypair");
                            Keypair::new()
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Failed to read keypair file: {}", e);
                    println!("🔑 Generating new keypair");
                    Keypair::new()
                }
            }
        } else {
            println!("🔑 No keypair file found, generating new keypair");
            Keypair::new()
        };
        
        // Check balance before proceeding
        match solana_client.get_balance(&payer.pubkey()) {
            Ok(balance) => {
                println!("💰 Current balance: {} SOL", balance as f64 / 1_000_000_000.0);
                
                // If balance is 0, request airdrop
                if balance == 0 {
                    println!("🪂 Requesting airdrop...");
                    if let Err(e) = solana_client.request_airdrop(&payer.pubkey(), 10_000_000_000) {
                        eprintln!("Failed to request airdrop: {}", e);
                    } else {
                        println!("✅ Airdrop requested, waiting for confirmation...");
                        std::thread::sleep(std::time::Duration::from_secs(3));
                        
                        // Check balance again
                        match solana_client.get_balance(&payer.pubkey()) {
                            Ok(new_balance) => println!("💰 New balance: {} SOL", new_balance as f64 / 1_000_000_000.0),
                            Err(e) => eprintln!("Failed to get new balance: {}", e),
                        }
                    }
                }
            }
            Err(e) => eprintln!("Failed to get balance: {}", e),
        }
        
        Self {
            solana_client,
            program_id,
            payer,
        }
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

        // Call Solana program
        match self.call_solana_program(record).await {
            Ok(signature) => {
                let response = StoreResponse {
                    success: true,
                    transaction_id: signature,
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                eprintln!("Solana transaction failed: {}", e);
                Err(Status::internal("Failed to store on blockchain"))
            }
        }
    }

    async fn retrieve(&self, request: Request<RetrieveRequest>) -> Result<Response<RetrieveResponse>, Status> {
        println!("Received RetrieveRequest: {:?}", request);

        // TODO: Implement actual retrieval from Solana
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

impl MyChainGateway {
    async fn call_solana_program(&self, record: &ContentRecord) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        // Wait a bit to ensure airdrop is confirmed
        tokio::time::sleep(Duration::from_secs(2)).await;
        
        // Serialize the record data (you might want to use a more efficient format)
        let instruction_data = format!("{}:{}:{}", record.uid, record.hash, record.url).into_bytes();
        
        // Create instruction to call your Solana program
        let instruction = Instruction {
            program_id: self.program_id,
            accounts: vec![], // No accounts needed for your simple program
            data: instruction_data,
        };

        // Get recent blockhash
        let recent_blockhash = self.solana_client.get_latest_blockhash()?;

        // Create transaction
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&self.payer.pubkey()),
            &[&self.payer],
            recent_blockhash,
        );

        // Send transaction with confirmation
        let signature = self.solana_client.send_and_confirm_transaction_with_spinner(&transaction)?;
        
        println!("✅ Solana transaction successful! Signature: {}", signature);
        
        Ok(signature.to_string())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "0.0.0.0:50051".parse()?;
    let service = MyChainGateway::default();

    println!("ChainGateway gRPC server listening on {}", addr);
    println!("Connected to Solana program: {}", service.program_id);

    Server::builder()
        .add_service(ChainGatewayServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}