use async_trait::async_trait;
use borsh::{BorshSerialize, BorshDeserialize};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_program,
    transaction::Transaction,
};
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use crate::blockchain::{
    BlockchainProvider, BlockchainResult, ChainConfig, NetworkInfo, TransactionResult,
};
use crate::proto::ContentRecord;

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

pub struct SolanaProvider {
    client: RpcClient,
    program_id: Pubkey,
    payer: Keypair,
    config: ChainConfig,
}

impl SolanaProvider {
    pub fn new(config: ChainConfig) -> BlockchainResult<Self> {
        let client = RpcClient::new_with_commitment(
            config.network_url.clone(),
            CommitmentConfig::confirmed(),
        );

        let program_id = Pubkey::from_str(
            config
                .program_id
                .as_ref()
                .ok_or("Program ID is required for Solana provider")?,
        )?;
        
        println!("🔗 Using program ID: {}", program_id);

        let payer = Self::load_keypair(&config)?;

        Ok(Self {
            client,
            program_id,
            payer,
            config,
        })
    }

    fn load_keypair(config: &ChainConfig) -> BlockchainResult<Keypair> {
        if let Some(keypair_path) = &config.private_key_path {
            let path = Path::new(keypair_path);
            if path.exists() {
                match std::fs::read_to_string(path) {
                    Ok(keypair_json) => {
                        match serde_json::from_str::<Vec<u8>>(&keypair_json) {
                            Ok(keypair_bytes) => {
                                match Keypair::from_bytes(&keypair_bytes) {
                                    Ok(keypair) => {
                                        println!("🔑 Loaded existing keypair: {}", keypair.pubkey());
                                        return Ok(keypair);
                                    }
                                    Err(e) => {
                                        eprintln!("Failed to parse keypair bytes: {}", e);
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("Failed to parse keypair JSON: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to read keypair file: {}", e);
                    }
                }
            }
        }

        println!("🔑 Generating new keypair");
        Ok(Keypair::new())
    }

    async fn wait_for_connection(&self) -> BlockchainResult<()> {
        println!("🔌 Connecting to Solana validator...");
        for attempt in 1..=10 {
            match self.client.get_health() {
                Ok(_) => {
                    println!("✅ Connected to Solana validator");
                    return Ok(());
                }
                Err(e) => {
                    println!("❌ Connection attempt {}/10 failed: {}", attempt, e);
                    if attempt < 10 {
                        tokio::time::sleep(Duration::from_secs(3)).await;
                    }
                }
            }
        }
        Err("Failed to connect to Solana validator after 10 attempts".into())
    }

    async fn store_record_impl(&self, record: &ContentRecord) -> BlockchainResult<TransactionResult> {
        // Wait a bit to ensure airdrop is confirmed
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Generate a new keypair for the proof account
        let proof_account = Keypair::new();
        println!("🔑 Generated proof account: {}", proof_account.pubkey());

        // Create the instruction data
        let instruction_data = ProofInstruction::StoreProof {
            url: record.url.clone(),
            content_hash: record.content_hash.clone(),
            content_length: record.content_length,
        };

        // Serialize the instruction using Borsh
        let data = instruction_data.try_to_vec()?;
        
        // Debug: Print detailed instruction data information
        println!("🔍 Instruction data size: {} bytes", data.len());
        println!("🔍 Instruction data (first 32 bytes): {:?}", &data[..data.len().min(32)]);
        println!("🔍 Full instruction data: {:?}", data);
        println!("🔍 Variant tag (first byte): {:?}", data.first());
        println!("🔍 StoreProof params:");
        println!("   URL: {}", record.url);
        println!("   Hash: {}", record.content_hash);
        println!("   Length: {}", record.content_length);

        // Create instruction with the correct accounts
        let instruction = Instruction::new_with_bytes(
            self.program_id,
            &data,
            vec![
                AccountMeta::new(self.payer.pubkey(), true),     // Payer (signer)
                AccountMeta::new(proof_account.pubkey(), true), // Proof account (writable + signer)
                AccountMeta::new_readonly(system_program::ID, false), // System program
            ],
        );

        // Get recent blockhash
        let recent_blockhash = self.client.get_latest_blockhash()?;
        println!("🔗 Recent blockhash: {}", recent_blockhash);

        // Create transaction with both signers
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&self.payer.pubkey()),
            &[&self.payer, &proof_account], // Both payer and proof account need to sign
            recent_blockhash,
        );

        println!("📝 Sending transaction...");
        // Send transaction with confirmation
        let signature = self
            .client
            .send_and_confirm_transaction_with_spinner(&transaction)?;

        println!("✅ Solana transaction successful!");
        println!("📄 Transaction signature: {}", signature);
        println!("📄 Proof account: {}", proof_account.pubkey());
        println!("🔗 UID: {}", record.uid);

        Ok(TransactionResult {
            transaction_id: signature.to_string(),
            block_height: None, // Could fetch this if needed
            confirmation_time: None,
        })
    }
}

#[async_trait]
impl BlockchainProvider for SolanaProvider {
    async fn store_record(&self, record: &ContentRecord) -> BlockchainResult<TransactionResult> {
        self.store_record_impl(record).await
    }
}

impl SolanaProvider {
    pub async fn initialize(&self) -> BlockchainResult<()> {
        self.wait_for_connection().await
    }
} 