use tonic::{transport::Server, Request, Response, Status};
use proto::chain_gateway_server::{ChainGateway, ChainGatewayServer};
use proto::{StoreRequest, StoreResponse, ContentRecord};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
    commitment_config::CommitmentConfig,
};
use borsh::{BorshSerialize, BorshDeserialize};
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
        
        // Load the existing keypair from the mounted volume (JSON format)
        let keypair_path = Path::new("/root/.config/solana/id.json");
        let payer = if keypair_path.exists() {
            match std::fs::read_to_string(keypair_path) {
                Ok(keypair_json) => {
                    // Parse the JSON array format that Solana CLI uses
                    match serde_json::from_str::<Vec<u8>>(&keypair_json) {
                        Ok(keypair_bytes) => {
                            match Keypair::from_bytes(&keypair_bytes) {
                                Ok(keypair) => {
                                    println!("🔑 Loaded existing keypair: {}", keypair.pubkey());
                                    keypair
                                }
                                Err(e) => {
                                    eprintln!("Failed to parse keypair bytes: {}", e);
                                    println!("🔑 Generating new keypair");
                                    Keypair::new()
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to parse keypair JSON: {}", e);
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
        
        // Wait for Solana connection with retries
        println!("🔌 Connecting to Solana validator...");
        for attempt in 1..=10 {
            match solana_client.get_health() {
                Ok(_) => {
                    println!("✅ Connected to Solana validator");
                    break;
                }
                Err(e) => {
                    println!("❌ Connection attempt {}/10 failed: {}", attempt, e);
                    if attempt < 10 {
                        std::thread::sleep(std::time::Duration::from_secs(3));
                    }
                }
            }
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
            Ok((signature, account_address)) => {
                let response = StoreResponse {
                    success: true,
                    transaction_id: signature,
                    account_address,
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                eprintln!("Solana transaction failed: {}", e);
                Err(Status::internal("Failed to store on blockchain"))
            }
        }
    }
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub enum ProofInstruction {
    StoreProof {
        url: String,
        content_hash: String,
        content_length: u64,
    },
}

impl ProofInstruction {
    pub fn try_to_vec(&self) -> Result<Vec<u8>, std::io::Error> {
        borsh::to_vec(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }
}

impl MyChainGateway {
    async fn call_solana_program(&self, record: &ContentRecord) -> Result<(String, String), Box<dyn std::error::Error + Send + Sync>> {
        // Wait a bit to ensure airdrop is confirmed
        tokio::time::sleep(Duration::from_secs(2)).await;
        
        // Generate a new keypair for the proof account
        let proof_account = Keypair::new();
        
        // Create the instruction data in the format your Solana program expects
        let instruction_data = ProofInstruction::StoreProof {
            url: record.url.clone(),
            content_hash: record.content_hash.clone(),
            content_length: record.content_length,
        };
        
        // Serialize the instruction using Borsh
        let data = instruction_data.try_to_vec()?;
        
        // Debug: Print the serialized instruction data
        println!("🔍 Instruction data size: {} bytes", data.len());
        println!("🔍 Instruction data (first 32 bytes): {:?}", &data[..data.len().min(32)]);
        println!("🔍 Full instruction data: {:?}", data);
        println!("🔍 StoreProof params - URL: {}, Hash: {}, Length: {}", 
                record.url, record.content_hash, record.content_length);
        
        // Create instruction with the correct accounts
        let instruction = Instruction::new_with_bytes(
            self.program_id,
            &data,
            vec![
                AccountMeta::new(self.payer.pubkey(), true),     // Payer (signer)
                AccountMeta::new(proof_account.pubkey(), false), // Proof account (writable, not signer)
            ],
        );

        // Get recent blockhash
        let recent_blockhash = self.solana_client.get_latest_blockhash()?;

        // Create transaction with only payer as signer
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&self.payer.pubkey()),
            &[&self.payer], // Only payer needs to sign
            recent_blockhash,
        );

        // Send transaction with confirmation
        let signature = self.solana_client.send_and_confirm_transaction_with_spinner(&transaction)?;
        
        println!("✅ Solana transaction successful! Signature: {}", signature);
        println!("📄 Proof account: {}", proof_account.pubkey());
        
        Ok((signature.to_string(), proof_account.pubkey().to_string()))
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