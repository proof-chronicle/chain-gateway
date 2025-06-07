use async_trait::async_trait;
use std::error::Error;
use crate::proto::ContentRecord;

pub type BlockchainResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Debug, Clone)]
pub struct TransactionResult {
    pub transaction_id: String,
    pub block_height: Option<u64>,
    pub confirmation_time: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct ChainConfig {
    pub network_url: String,
    pub program_id: Option<String>,
    pub private_key_path: Option<String>,
    pub chain_type: ChainType,
}

#[derive(Debug, Clone)]
pub enum ChainType {
    Solana,
    Ethereum,
    // Add more chains as needed
}

/// General blockchain interface for content storage and retrieval
#[async_trait]
pub trait BlockchainProvider: Send + Sync {
    /// Store a content record on the blockchain
    async fn store_record(&self, record: &ContentRecord) -> BlockchainResult<TransactionResult>;
    
    /// Retrieve a content record by transaction ID
    async fn retrieve_record(&self, transaction_id: &str) -> BlockchainResult<Option<ContentRecord>>;
    
    /// Check if the blockchain connection is healthy
    async fn health_check(&self) -> BlockchainResult<bool>;
    
    /// Get the current network status
    async fn get_network_info(&self) -> BlockchainResult<NetworkInfo>;
}

#[derive(Debug)]
pub struct NetworkInfo {
    pub chain_id: String,
    pub block_height: u64,
    pub network_name: String,
} 