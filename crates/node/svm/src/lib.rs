// solana-svm 3.1.9 marks all public APIs as #[deprecated] pending agave-unstable-api feature in v4.0.
#![allow(deprecated)]

//! Monmouth SVM Module — native Solana Virtual Machine execution.
//!
//! Provides a second execution environment alongside REVM, enabling
//! dual-VM block building where agents can target either EVM or SVM
//! via the Transaction Envelope.
//!
//! # Architecture
//!
//! - [`SvmExecutor`] wraps Solana's `TransactionBatchProcessor` with
//!   Monmouth-specific configuration (sysvars, builtins, fork graph).
//! - [`SvmAccountBridge`] implements `TransactionProcessingCallback` to
//!   feed account state into the SVM during execution.
//! - [`SvmChangeSet`] captures account mutations in 32-byte pubkey space,
//!   separate from EVM's 20-byte address `ChangeSet`.
//!
//! # Phases
//!
//! - **Phase 1** (this): In-memory account bridge, standalone executor
//! - **Phase 2**: QMDB-backed storage, `svm_state_root` in block header
//! - **Phase 3**: Runner integration, dual-VM block building
//! - **Phase 4**: RPC endpoints, Prometheus metrics

pub mod account_bridge;
pub mod builtins;
pub mod changeset;
pub mod deserialize;
pub mod error;
pub mod executor;
pub mod fork_graph;
pub mod store;
pub mod sysvars;

pub use account_bridge::SvmAccountBridge;
pub use changeset::{SvmAccountUpdate, SvmChangeSet};
pub use deserialize::deserialize_svm_tx;
pub use error::SvmError;
pub use executor::{SvmExecutionOutcome, SvmExecutor, SvmExecutorConfig, SvmTxResult};
pub use fork_graph::MonmouthForkGraph;
pub use store::SvmStateStore;
