//! Error types for the simulation module.

/// Errors that can occur during transaction simulation.
#[derive(Debug, Clone, thiserror::Error)]
pub enum SimulationError {
    /// The simulated transaction execution failed.
    #[error("execution failed: {0}")]
    ExecutionFailed(String),

    /// The transaction is malformed or invalid.
    #[error("invalid transaction: {0}")]
    InvalidTransaction(String),

    /// The bundle exceeds the maximum allowed size.
    #[error("bundle too large: {size} transactions (max: {max})")]
    BundleTooLarge {
        /// Actual number of transactions in the bundle.
        size: usize,
        /// Maximum allowed number of transactions.
        max: usize,
    },

    /// The transaction exceeded the gas limit.
    #[error("gas limit exceeded: used {used}, limit {limit}")]
    GasLimitExceeded {
        /// Gas actually used.
        used: u64,
        /// Gas limit that was exceeded.
        limit: u64,
    },

    /// The simulation timed out.
    #[error("simulation timed out")]
    Timeout,

    /// Required state is not available for simulation.
    #[error("state unavailable: {0}")]
    StateUnavailable(String),
}

impl SimulationError {
    /// Returns the error code for JSON-RPC responses.
    #[must_use]
    pub const fn code(&self) -> i32 {
        match self {
            Self::ExecutionFailed(_) => -32800,
            Self::InvalidTransaction(_) => -32801,
            Self::BundleTooLarge { .. } => -32802,
            Self::GasLimitExceeded { .. } => -32803,
            Self::Timeout => -32804,
            Self::StateUnavailable(_) => -32805,
        }
    }
}
