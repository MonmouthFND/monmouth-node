//! SVM execution error types.

use thiserror::Error;

/// Errors that can occur during SVM execution.
#[derive(Debug, Error)]
pub enum SvmError {
    /// Transaction deserialization failed.
    #[error("svm: failed to decode transaction: {0}")]
    TxDecode(String),

    /// Transaction execution failed.
    #[error("svm: transaction execution failed: {0}")]
    TxExecution(String),

    /// Account loading error.
    #[error("svm: account error: {0}")]
    Account(String),

    /// Sysvar population error.
    #[error("svm: sysvar error: {0}")]
    Sysvar(String),

    /// Compute budget exceeded.
    #[error("svm: compute budget exceeded: used {used}, limit {limit}")]
    ComputeBudgetExceeded {
        /// Compute units consumed.
        used: u64,
        /// Compute unit limit.
        limit: u64,
    },

    /// Program not deployed.
    #[error("svm: program not found: {0}")]
    ProgramNotFound(String),

    /// Internal processor error.
    #[error("svm: internal error: {0}")]
    Internal(String),

    /// A shared lock was poisoned (a thread panicked while holding it).
    #[error("svm: lock poisoned: {0}")]
    LockPoisoned(String),
}

/// JSON-RPC error codes for SVM-specific errors.
///
/// Range: -32890 to -32899 (reserved for SVM module).
impl SvmError {
    /// JSON-RPC error code for this error.
    pub const fn rpc_code(&self) -> i64 {
        match self {
            Self::TxDecode(_) => -32890,
            Self::TxExecution(_) => -32891,
            Self::Account(_) => -32892,
            Self::Sysvar(_) => -32893,
            Self::ComputeBudgetExceeded { .. } => -32894,
            Self::ProgramNotFound(_) => -32895,
            Self::Internal(_) | Self::LockPoisoned(_) => -32899,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display() {
        let err = SvmError::TxDecode("bad bytes".into());
        assert!(err.to_string().contains("bad bytes"));
    }

    #[test]
    fn compute_budget_display() {
        let err = SvmError::ComputeBudgetExceeded { used: 300_000, limit: 200_000 };
        let msg = err.to_string();
        assert!(msg.contains("300000"));
        assert!(msg.contains("200000"));
    }

    #[test]
    fn rpc_codes_in_range() {
        let errors: Vec<SvmError> = vec![
            SvmError::TxDecode("".into()),
            SvmError::TxExecution("".into()),
            SvmError::Account("".into()),
            SvmError::Sysvar("".into()),
            SvmError::ComputeBudgetExceeded { used: 0, limit: 0 },
            SvmError::ProgramNotFound("".into()),
            SvmError::Internal("".into()),
            SvmError::LockPoisoned("".into()),
        ];
        for err in &errors {
            let code = err.rpc_code();
            assert!((-32899..=-32890).contains(&code), "code {code} out of SVM range");
        }
    }

    #[test]
    fn error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<SvmError>();
    }
}
