//! Error types for the capability registry.

/// Errors that can occur during capability registry operations.
#[derive(Debug, Clone, thiserror::Error)]
pub enum CapabilityError {
    /// A capability with this ID already exists.
    #[error("capability already exists: {0}")]
    AlreadyExists(String),

    /// No capability found with this ID.
    #[error("capability not found: {0}")]
    NotFound(String),

    /// The capability ID is invalid.
    #[error("invalid capability ID: {0}")]
    InvalidId(String),

    /// The registry is at capacity.
    #[error("registry capacity exceeded (max: {0})")]
    CapacityExceeded(usize),
}

impl CapabilityError {
    /// Returns the error code for JSON-RPC responses.
    #[must_use]
    pub const fn code(&self) -> i32 {
        match self {
            Self::AlreadyExists(_) => -32600,
            Self::NotFound(_) => -32601,
            Self::InvalidId(_) => -32602,
            Self::CapacityExceeded(_) => -32603,
        }
    }
}
