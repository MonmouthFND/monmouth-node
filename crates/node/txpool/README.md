# `monmouth-txpool`

<a href="https://github.com/MonmouthFND/monmouth-node/actions/workflows/ci.yml"><img src="https://github.com/MonmouthFND/monmouth-node/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
<a href="https://github.com/MonmouthFND/monmouth-node/blob/main/LICENSE"><img src="https://img.shields.io/badge/License-MIT-d1d1f6.svg" alt="License"></a>

Production-ready transaction pool for monmouth-node with validation, nonce ordering, and fee prioritization.

## Features

- **Per-sender nonce-ordered queues**: Transactions are organized by sender with proper nonce ordering
- **Pending vs queued separation**: Executable transactions (pending) are separated from future nonce transactions (queued)
- **Fee-based ordering**: Transactions with higher effective gas prices are prioritized
- **Transaction validation**: Signature recovery, chain ID validation, intrinsic gas calculation, balance checks
- **Configurable limits**: Max pool size, per-sender limits, max transaction size, minimum gas price

## Usage

```rust,ignore
use monmouth_txpool::{TransactionPool, PoolConfig, TransactionValidator};

// Create a pool with default configuration
let config = PoolConfig::default();
let pool = TransactionPool::new(config);

// Create a validator with chain ID, state access, and config
let validator = TransactionValidator::new(chain_id, state_db, config);

// Validate and add a transaction
let validated = validator.validate(raw_tx).await?;
let ordered = validated.into_ordered(timestamp);
pool.add(ordered)?;

// Get pending transactions for block building
let pending = pool.pending(100);
```

## License

This project is licensed under the [MIT License](../../LICENSE).
