# `kora-executor`

<a href="https://github.com/refcell/kora/actions/workflows/ci.yml"><img src="https://github.com/refcell/kora/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
<a href="https://github.com/refcell/kora/blob/main/LICENSE"><img src="https://img.shields.io/badge/License-MIT-d1d1f6.svg" alt="License"></a>

Block execution abstractions and REVM-based implementation for Kora.

This crate provides:
- `BlockExecutor` trait for executing transactions against state
- `ExecutionOutcome` with receipts and state changes
- `RevmExecutor` implementing block execution via REVM

## Key Types

- `BlockExecutor` - trait defining block execution interface
- `RevmExecutor` - REVM-based executor implementation
- `ExecutionOutcome` - execution results with receipts and state changes
- `ExecutionReceipt` - individual transaction receipt
- `BlockContext` / `ParentBlock` - execution context types
- `ExecutionConfig` - configurable gas limits and base fee parameters
- `TxValidator` / `ValidatedTx` - transaction validation utilities
- `StateDbAdapter` - adapter for state database access

## Usage

```rust,ignore
use kora_executor::{RevmExecutor, BlockContext, ExecutionConfig};

// Create an executor with configuration
let config = ExecutionConfig::default();
let executor = RevmExecutor::new(config);

// Execute a block
let context = BlockContext::new(parent, timestamp, beneficiary);
let outcome = executor.execute(&state, &context, &transactions).await?;
```

## License

This project is licensed under the [MIT License](../../LICENSE).
