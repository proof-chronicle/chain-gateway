use async_trait::async_trait;
use borsh::BorshSerialize;
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

#[derive(BorshSerialize)]
pub enum ProofInstruction {
    StoreProof {
        url: String,
        hash: String,
        created_at: String,
    },
    GetProof,
}

impl ProofInstruction {
    pub fn try_to_vec(&self) -> Result<Vec<u8>, std::io::Error> {
        borsh::to_vec(self).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
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

        // Create the instruction data
        let instruction_data = ProofInstruction::StoreProof {
            url: record.url.clone(),
            hash: record.hash.clone(),
            created_at: record.created_at.clone(),
        };

        // Serialize the instruction using Borsh
        let data = instruction_data.try_to_vec()?;

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

        // Create transaction with both signers
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&self.payer.pubkey()),
            &[&self.payer, &proof_account], // Both payer and proof account need to sign
            recent_blockhash,
        );

        // Send transaction with confirmation
        let signature = self
            .client
            .send_and_confirm_transaction_with_spinner(&transaction)?;

        println!("✅ Solana transaction successful! Signature: {}", signature);
        println!("📄 Proof account: {}", proof_account.pubkey());

        Ok(TransactionResult {
            transaction_id: signature.to_string(),
            block_height: None, // Could fetch this if needed
            confirmation_time: None,
        })
    }

    async fn retrieve_record_impl(&self, transaction_id: &str) -> BlockchainResult<Option<ContentRecord>> {
        // TODO: Implement actual retrieval from Solana
        // This would involve deserializing account data
        println!("Retrieving record for transaction: {}", transaction_id);
        
        // Placeholder implementation
        Ok(Some(ContentRecord {
            uid: "retrieved_uid".into(),
            created_at: "2025-05-03T12:00:00Z".into(),
            hash: "retrieved_hash".into(),
            url: "https://retrieved.example.com".into(),
        }))
    }

    async fn health_check_impl(&self) -> BlockchainResult<bool> {
        match self.client.get_health() {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    async fn get_network_info_impl(&self) -> BlockchainResult<NetworkInfo> {
        let slot = self.client.get_slot()?;
        
        Ok(NetworkInfo {
            chain_id: "solana-localnet".to_string(),
            block_height: slot,
            network_name: "Solana Local Validator".to_string(),
        })
    }
}

#[async_trait]
impl BlockchainProvider for SolanaProvider {
    async fn store_record(&self, record: &ContentRecord) -> BlockchainResult<TransactionResult> {
        self.store_record_impl(record).await
    }

    async fn retrieve_record(&self, transaction_id: &str) -> BlockchainResult<Option<ContentRecord>> {
        self.retrieve_record_impl(transaction_id).await
    }

    async fn health_check(&self) -> BlockchainResult<bool> {
        self.health_check_impl().await
    }

    async fn get_network_info(&self) -> BlockchainResult<NetworkInfo> {
        self.get_network_info_impl().await
    }
}

impl SolanaProvider {
    pub async fn initialize(&self) -> BlockchainResult<()> {
        self.wait_for_connection().await
    }
} 