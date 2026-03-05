//! Error types for the memory anchor registry.

use alloy_primitives::B256;
use monmouth_agent_types::AgentId;

/// Errors that can occur during memory anchor operations.
#[derive(Debug, Clone, thiserror::Error)]
pub enum MemoryAnchorError {
    /// No anchor found for the given agent and sequence number.
    #[error("anchor not found for agent {agent} at sequence {sequence}")]
    NotFound {
        /// The agent that was queried.
        agent: AgentId,
        /// The sequence number that was queried.
        sequence: u64,
    },

    /// The per-agent anchor capacity has been exceeded.
    #[error("anchor capacity exceeded for agent {agent} (max: {max})")]
    CapacityExceeded {
        /// The agent that exceeded capacity.
        agent: AgentId,
        /// The maximum number of anchors per agent.
        max: usize,
    },

    /// Content hash verification failed.
    #[error(
        "verification failed for agent {agent} at sequence {sequence}: expected {expected}, got {actual}"
    )]
    VerificationFailed {
        /// The agent whose anchor was verified.
        agent: AgentId,
        /// The sequence number of the anchor.
        sequence: u64,
        /// The expected content hash.
        expected: B256,
        /// The actual content hash provided.
        actual: B256,
    },
}

impl MemoryAnchorError {
    /// Returns the error code for JSON-RPC responses.
    #[must_use]
    pub const fn code(&self) -> i32 {
        match self {
            Self::NotFound { .. } => -32900,
            Self::CapacityExceeded { .. } => -32901,
            Self::VerificationFailed { .. } => -32902,
        }
    }
}
