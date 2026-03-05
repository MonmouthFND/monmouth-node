//! Error types for the delegation registry.

use alloy_primitives::Address;
use monmouth_agent_types::SessionId;

/// Errors that can occur during delegation operations.
#[derive(Debug, Clone, thiserror::Error)]
pub enum DelegationError {
    /// No session found with the given ID.
    #[error("session not found: {0}")]
    SessionNotFound(SessionId),

    /// The session has expired.
    #[error("session expired: {0}")]
    SessionExpired(SessionId),

    /// The caller is not authorised for this session operation.
    #[error("unauthorised for session {session_id}: expected owner {expected_owner}, got {actual}")]
    Unauthorized {
        /// The session that was accessed.
        session_id: SessionId,
        /// The expected owner address.
        expected_owner: Address,
        /// The actual caller address.
        actual: Address,
    },

    /// The session's spending limit would be exceeded.
    #[error(
        "spending limit exceeded for session {session_id}: limit {limit}, attempted {attempted}"
    )]
    SpendingLimitExceeded {
        /// The session whose limit would be exceeded.
        session_id: SessionId,
        /// The maximum spending limit in wei.
        limit: alloy_primitives::U256,
        /// The amount that was attempted.
        attempted: alloy_primitives::U256,
    },

    /// The requested capability is not granted to this session.
    #[error("capability not granted for session {session_id}: {capability_id}")]
    CapabilityNotGranted {
        /// The session that lacks the capability.
        session_id: SessionId,
        /// The capability ID that was requested.
        capability_id: String,
    },

    /// The registry has reached its maximum session capacity.
    #[error("registry capacity exceeded (max: {0})")]
    CapacityExceeded(usize),

    /// The cryptographic signature is invalid.
    #[error("invalid signature: {0}")]
    InvalidSignature(String),

    /// A session with this ID already exists.
    #[error("duplicate session: {0}")]
    DuplicateSession(SessionId),
}

impl DelegationError {
    /// Returns the error code for JSON-RPC responses.
    #[must_use]
    pub const fn code(&self) -> i32 {
        match self {
            Self::SessionNotFound(_) => -32600,
            Self::SessionExpired(_) => -32601,
            Self::Unauthorized { .. } => -32602,
            Self::SpendingLimitExceeded { .. } => -32603,
            Self::CapabilityNotGranted { .. } => -32604,
            Self::CapacityExceeded(_) => -32605,
            Self::InvalidSignature(_) => -32606,
            Self::DuplicateSession(_) => -32607,
        }
    }
}
