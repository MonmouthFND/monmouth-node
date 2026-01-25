//! Error types for state database operations.

use alloy_primitives::B256;
use thiserror::Error;

/// Error type for state database operations.
#[derive(Debug, Error)]
pub enum StateDbError {
    /// Account not found.
    #[error("account not found: {0}")]
    AccountNotFound(alloy_primitives::Address),

    /// Code not found for hash.
    #[error("code not found: {0}")]
    CodeNotFound(B256),

    /// Storage error from underlying store.
    #[error("storage error: {0}")]
    Storage(String),

    /// Lock was poisoned.
    #[error("lock poisoned")]
    LockPoisoned,

    /// State root computation failed.
    #[error("root computation failed: {0}")]
    RootComputation(String),
}
