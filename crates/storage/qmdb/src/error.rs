//! Error types for QMDB operations.

use alloy_primitives::B256;
use thiserror::Error;

/// Error type for QMDB store operations.
#[derive(Debug, Error)]
pub enum QmdbError {
    /// Storage backend error.
    #[error("storage error: {0}")]
    Storage(String),

    /// Stores unavailable during update.
    #[error("stores unavailable")]
    StoreUnavailable,

    /// Account decoding failed.
    #[error("account decode failed")]
    DecodeError,

    /// Code not found for hash.
    #[error("code not found: {0}")]
    CodeNotFound(B256),
}
