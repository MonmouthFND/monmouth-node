//! Error types for the envelope codec.

/// Errors that can occur during envelope encoding or decoding.
#[derive(Debug, Clone, thiserror::Error)]
pub enum EnvelopeError {
    /// The first byte is not the expected Monmouth magic byte (0x4d).
    #[error("invalid magic byte: expected 0x4d, got 0x{0:02x}")]
    InvalidMagicByte(u8),

    /// Failed to decode the envelope (RLP or JSON parse failure).
    #[error("decode failed: {0}")]
    DecodeFailed(String),

    /// A required field is missing from the envelope header.
    #[error("missing required field: {0}")]
    MissingField(String),

    /// The specified VM target is not recognised.
    #[error("invalid VM target: {0}")]
    InvalidVmTarget(String),
}

impl EnvelopeError {
    /// Returns the error code for JSON-RPC responses.
    #[must_use]
    pub const fn code(&self) -> i32 {
        match self {
            Self::InvalidMagicByte(_) => -32600,
            Self::DecodeFailed(_) => -32601,
            Self::MissingField(_) => -32602,
            Self::InvalidVmTarget(_) => -32603,
        }
    }
}
