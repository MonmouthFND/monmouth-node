//! Execution outcome types.

use alloy_primitives::B256;
use kora_qmdb::ChangeSet;

/// Result of executing a block's transactions.
#[derive(Debug, Clone)]
pub struct ExecutionOutcome {
    /// State changes to be persisted.
    pub changes: ChangeSet,
    /// Computed state root after execution.
    pub state_root: B256,
    /// Total gas used.
    pub gas_used: u64,
}

impl ExecutionOutcome {
    /// Create a new execution outcome.
    pub const fn new(changes: ChangeSet, state_root: B256, gas_used: u64) -> Self {
        Self { changes, state_root, gas_used }
    }
}

impl Default for ExecutionOutcome {
    fn default() -> Self {
        Self { changes: ChangeSet::new(), state_root: B256::ZERO, gas_used: 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn execution_outcome_default() {
        let outcome = ExecutionOutcome::default();
        assert_eq!(outcome.gas_used, 0);
        assert_eq!(outcome.state_root, B256::ZERO);
    }
}
