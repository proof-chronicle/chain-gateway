use async_trait::async_trait;
use std::error::Error;
use crate::proto::ContentRecord;

pub type BlockchainResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Debug, Clone)]
pub struct TransactionResult {
    pub transaction_id: String,
}

#[derive(Debug, Clone)]
pub struct SolanaConfig {
    pub rpc_url: String,
    pub program_id: String,
    pub keypair_path: String,
    pub proof_account_keypair_path: String,
}

#[derive(Debug, Clone)]
pub enum ChainType {
    Solana,
    Ethereum,
    // Add more chains as needed
}

/// Simplified blockchain interface for content storage only
#[async_trait]
pub trait BlockchainProvider: Send + Sync {
    /// Store a content record on the blockchain
    async fn store_record(&self, record: &ContentRecord) -> BlockchainResult<TransactionResult>;
}
 