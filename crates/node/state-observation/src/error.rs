//! Error types for the state observation module.

use alloy_primitives::Address;

/// Errors that can occur during state observation operations.
#[derive(Debug, Clone, thiserror::Error)]
pub enum StateObservationError {
    /// The state provider returned an error.
    #[error("provider error: {0}")]
    ProviderError(String),

    /// The queried account was not found.
    #[error("account not found: {0}")]
    AccountNotFound(Address),

    /// The query type is not supported by the current provider.
    #[error("unsupported query type: {0}")]
    UnsupportedQuery(String),

    /// Batch query exceeded maximum size.
    #[error("batch too large: {size} queries (max: {max})")]
    BatchTooLarge {
        /// Number of queries in the batch.
        size: usize,
        /// Maximum allowed batch size.
        max: usize,
    },
}

impl StateObservationError {
    /// Returns the error code for JSON-RPC responses.
    #[must_use]
    pub const fn code(&self) -> i32 {
        match self {
            Self::ProviderError(_) => -32850,
            Self::AccountNotFound(_) => -32851,
            Self::UnsupportedQuery(_) => -32852,
            Self::BatchTooLarge { .. } => -32853,
        }
    }
}
