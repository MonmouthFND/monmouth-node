//! Error types for the intent receipt store.

use alloy_primitives::B256;

/// Errors that can occur during intent receipt operations.
#[derive(Debug, Clone, thiserror::Error)]
pub enum IntentReceiptError {
    /// No receipt found for the given transaction hash.
    #[error("receipt not found for tx hash: {0}")]
    NotFound(B256),

    /// The store is at capacity.
    #[error("receipt store capacity exceeded (max: {0})")]
    CapacityExceeded(usize),

    /// A receipt already exists for the given transaction hash.
    #[error("duplicate receipt for tx hash: {0}")]
    DuplicateReceipt(B256),
}

impl IntentReceiptError {
    /// Returns the error code for JSON-RPC responses.
    #[must_use]
    pub const fn code(&self) -> i32 {
        match self {
            Self::NotFound(_) => -32800,
            Self::CapacityExceeded(_) => -32801,
            Self::DuplicateReceipt(_) => -32802,
        }
    }
}
