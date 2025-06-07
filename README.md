# Chain Gateway

A blockchain-agnostic gRPC service for storing and retrieving content records on various blockchain networks.

## Architecture

The Chain Gateway service has been refactored to support multiple blockchain networks through a general interface:

- **Blockchain Interface**: Generic trait for blockchain operations
- **Provider Pattern**: Separate implementations for each blockchain
- **Configuration-Driven**: Switch between blockchains via environment variables

## Supported Blockchains

- ✅ **Solana** - Fully implemented
- 📋 **Others** - Easily extensible

## Configuration

Configure the service using environment variables:

```bash
# Blockchain type
CHAIN_TYPE=solana

# Network connection
BLOCKCHAIN_URL=http://solana-validator:8899

# Blockchain-specific settings
PROGRAM_ID=6F8VF9413BrwBYLPndCbKTB74bbzDCdv335jToYzCA3D
PRIVATE_KEY_PATH=/root/.config/solana/id.json
```

## Quick Start

1. Copy environment configuration:
   ```bash
   cp example.env .env
   ```

2. Build the service:
   ```bash
   cargo build --release
   ```

3. Run the service:
   ```bash
   cargo run
   ```

## Adding New Blockchain Support

To add support for a new blockchain:

1. Create a new provider in `src/providers/`
2. Implement the `BlockchainProvider` trait
3. Add the chain type to `ChainType` enum
4. Update the provider factory in `main.rs`

Example provider structure:
```rust
use async_trait::async_trait;
use crate::blockchain::{BlockchainProvider, BlockchainResult, ChainConfig, NetworkInfo, TransactionResult};

pub struct NewChainProvider {
    // Chain-specific fields
}

#[async_trait]
impl BlockchainProvider for NewChainProvider {
    async fn store_record(&self, record: &ContentRecord) -> BlockchainResult<TransactionResult> {
        // Implementation
    }
    
    // ... other trait methods
}
```

## API

The service exposes a gRPC interface defined in `proto/chain_gateway.proto`:

- `Store(StoreRequest) -> StoreResponse`: Store a content record
- `Retrieve(RetrieveRequest) -> RetrieveResponse`: Retrieve a content record

## Development

Build for development:
```bash
cargo build
```

Run tests:
```bash
cargo test
```

Check code:
```bash
cargo check
```