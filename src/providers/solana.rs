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
    BlockchainProvider, BlockchainResult, SolanaConfig, TransactionResult,
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
    config: SolanaConfig,
}

impl SolanaProvider {
    pub fn new(config: SolanaConfig) -> BlockchainResult<Self> {
        let client = RpcClient::new_with_commitment(
            config.rpc_url.clone(),
            CommitmentConfig::confirmed(),
        );

        let program_id = Pubkey::from_str(&config.program_id)?;
        println!("🔗 Using program ID: {}", program_id);

        println!("🔑 Loading payer keypair from: {}", config.keypair_path);
        let payer = Self::load_keypair(&config.keypair_path)?;
        println!("🔑 Payer public key: {}", payer.pubkey());

        Ok(Self {
            client,
            program_id,
            payer,
            config,
        })
    }

    fn load_keypair(keypair_path: &str) -> BlockchainResult<Keypair> {
        let path = Path::new(keypair_path);
        if !path.exists() {
            return Err(format!("Keypair file not found at: {}", keypair_path).into());
        }

        let keypair_json = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read keypair file: {}", e))?;

        let keypair_bytes = serde_json::from_str::<Vec<u8>>(&keypair_json)
            .map_err(|e| format!("Failed to parse keypair JSON: {}", e))?;

        let keypair = Keypair::from_bytes(&keypair_bytes)
            .map_err(|e| format!("Failed to parse keypair bytes: {}", e))?;

        println!("🔑 Loaded existing keypair: {}", keypair.pubkey());
        Ok(keypair)
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

        // Generate a unique keypair for this proof record
        let proof_account = Keypair::new();
        
        // Verify they're different
        if proof_account.pubkey() == self.program_id {
            return Err("ERROR: Generated proof account matches program ID!".into());
        }
        if proof_account.pubkey() == self.payer.pubkey() {  
            return Err("ERROR: Generated proof account matches payer!".into());
        }

        // Create the instruction data
        let instruction_data = ProofInstruction::StoreProof {
            url: record.url.clone(),
            content_hash: record.content_hash.clone(),
            content_length: record.content_length,
        };

        // Serialize the instruction using Borsh
        let data = instruction_data.try_to_vec()?;

        // Calculate space needed for the account
        let space = data.len() as u64;
        let rent = self.client.get_minimum_balance_for_rent_exemption(space as usize)?;

        // Create account instruction
        let create_account_ix = solana_sdk::system_instruction::create_account(
            &self.payer.pubkey(),
            &proof_account.pubkey(),
            rent,
            space,
            &self.program_id,
        );
        println!("🏗️  Created create_account instruction");
        println!("   From: {} (payer)", self.payer.pubkey());
        println!("   To: {} (new account)", proof_account.pubkey());
        println!("   Owner: {} (program)", self.program_id);
        println!("   Lamports: {}", rent);
        println!("   Space: {}", space);

        // Store proof instruction
        let account_metas = vec![
            AccountMeta::new(self.payer.pubkey(), true),     // Payer (signer)
            AccountMeta::new(proof_account.pubkey(), true), // Proof account (writable, signer)
            AccountMeta::new_readonly(system_program::ID, false), // System program
        ];
        
        let store_proof_ix = Instruction::new_with_bytes(
            self.program_id,
            &data,
            account_metas,
        );

        // Get recent blockhash
        let recent_blockhash = self.client.get_latest_blockhash()?;

        // Create transaction with both instructions
        let instructions = [create_account_ix, store_proof_ix];
        let signers = [&self.payer, &proof_account];
        
        let transaction = Transaction::new_signed_with_payer(
            &instructions,
            Some(&self.payer.pubkey()),
            &signers,
            recent_blockhash,
        );

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