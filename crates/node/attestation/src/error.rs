//! Error types for the attestation module.

use monmouth_agent_types::{AttestationId, AttestationType};

/// Errors that can occur during attestation operations.
#[derive(Debug, Clone, thiserror::Error)]
pub enum AttestationError {
    /// No attestation found with the given ID.
    #[error("attestation not found: {0}")]
    NotFound(AttestationId),

    /// An attestation with this ID already exists.
    #[error("duplicate attestation: {0}")]
    Duplicate(AttestationId),

    /// Verification of the attestation failed.
    #[error("verification failed for attestation {id}: {reason}")]
    VerificationFailed {
        /// The attestation that failed verification.
        id: AttestationId,
        /// Human-readable reason for the failure.
        reason: String,
    },

    /// The attestation type is not yet supported for verification.
    #[error("unsupported attestation type for verification: {0:?}")]
    UnsupportedType(AttestationType),

    /// The attestation payload is malformed.
    #[error("malformed payload: {0}")]
    MalformedPayload(String),

    /// The registry is at capacity.
    #[error("attestation registry capacity exceeded (max: {0})")]
    CapacityExceeded(usize),
}

impl AttestationError {
    /// Returns the error code for JSON-RPC responses.
    #[must_use]
    pub const fn code(&self) -> i32 {
        match self {
            Self::NotFound(_) => -32880,
            Self::Duplicate(_) => -32881,
            Self::VerificationFailed { .. } => -32882,
            Self::UnsupportedType(_) => -32883,
            Self::MalformedPayload(_) => -32884,
            Self::CapacityExceeded(_) => -32885,
        }
    }
}
