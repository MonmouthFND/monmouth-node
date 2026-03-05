//! Error types for the conditional subscriptions module.

use monmouth_agent_types::{AgentId, SubscriptionId};

/// Errors that can occur during subscription operations.
#[derive(Debug, Clone, thiserror::Error)]
pub enum SubscriptionError {
    /// No subscription found with the given ID.
    #[error("subscription not found: {0}")]
    NotFound(SubscriptionId),

    /// A subscription with this ID already exists.
    #[error("duplicate subscription: {0}")]
    Duplicate(SubscriptionId),

    /// The subscription has reached its maximum trigger count.
    #[error("subscription {0} has exhausted all triggers")]
    TriggersExhausted(SubscriptionId),

    /// The subscription is not active.
    #[error("subscription {0} is not active")]
    NotActive(SubscriptionId),

    /// The caller is not the owner of this subscription.
    #[error("unauthorised: agent {agent} does not own subscription {subscription_id}")]
    Unauthorized {
        /// The subscription being acted upon.
        subscription_id: SubscriptionId,
        /// The agent that attempted the action.
        agent: AgentId,
    },

    /// The registry is at capacity.
    #[error("subscription registry capacity exceeded (max: {0})")]
    CapacityExceeded(usize),
}

impl SubscriptionError {
    /// Returns the error code for JSON-RPC responses.
    #[must_use]
    pub const fn code(&self) -> i32 {
        match self {
            Self::NotFound(_) => -32870,
            Self::Duplicate(_) => -32871,
            Self::TriggersExhausted(_) => -32872,
            Self::NotActive(_) => -32873,
            Self::Unauthorized { .. } => -32874,
            Self::CapacityExceeded(_) => -32875,
        }
    }
}
