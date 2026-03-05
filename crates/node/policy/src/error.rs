//! Error types for the policy engine.

use monmouth_agent_types::PolicyRuleId;

/// Errors that can occur during policy engine operations.
#[derive(Debug, Clone, thiserror::Error)]
pub enum PolicyError {
    /// No rule found with this ID.
    #[error("policy rule not found: {0}")]
    RuleNotFound(PolicyRuleId),

    /// A rule with this ID already exists.
    #[error("duplicate policy rule: {0}")]
    DuplicateRule(PolicyRuleId),

    /// The registry is at capacity.
    #[error("policy registry capacity exceeded (max: {0})")]
    CapacityExceeded(usize),

    /// The action was denied by policy.
    #[error("denied for agent {agent}: {reason}")]
    Denied {
        /// The agent whose action was denied.
        agent: monmouth_agent_types::AgentId,
        /// Human-readable reason for the denial.
        reason: String,
    },
}

impl PolicyError {
    /// Returns the error code for JSON-RPC responses.
    #[must_use]
    pub const fn code(&self) -> i32 {
        match self {
            Self::RuleNotFound(_) => -32700,
            Self::DuplicateRule(_) => -32701,
            Self::CapacityExceeded(_) => -32702,
            Self::Denied { .. } => -32703,
        }
    }
}
