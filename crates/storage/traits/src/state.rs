//! State database traits for consensus-facing operations.

use alloy_primitives::{Address, B256, Bytes, U256};
use kora_qmdb::ChangeSet;

use crate::StateDbError;

/// Read-only access to blockchain state.
///
/// Provides account, storage, and code lookups without mutation.
pub trait StateDbRead: Clone + Send + Sync + 'static {
    /// Get account nonce.
    fn nonce(&self, address: &Address) -> Result<u64, StateDbError>;

    /// Get account balance.
    fn balance(&self, address: &Address) -> Result<U256, StateDbError>;

    /// Get account code hash.
    fn code_hash(&self, address: &Address) -> Result<B256, StateDbError>;

    /// Get account code by hash.
    fn code(&self, code_hash: &B256) -> Result<Bytes, StateDbError>;

    /// Get storage slot value.
    fn storage(&self, address: &Address, slot: &U256) -> Result<U256, StateDbError>;

    /// Check if an account exists.
    fn exists(&self, address: &Address) -> Result<bool, StateDbError> {
        match self.nonce(address) {
            Ok(nonce) => Ok(nonce > 0 || !self.balance(address)?.is_zero()),
            Err(StateDbError::AccountNotFound(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }
}

/// Write access to blockchain state.
///
/// Provides atomic state mutations through change sets.
pub trait StateDbWrite: Clone + Send + Sync + 'static {
    /// Commit a set of changes atomically.
    ///
    /// Returns the new state root after applying changes.
    fn commit(&self, changes: ChangeSet) -> Result<B256, StateDbError>;

    /// Compute the state root that would result from applying changes.
    ///
    /// Does not persist changes.
    fn compute_root(&self, changes: &ChangeSet) -> Result<B256, StateDbError>;

    /// Merge two change sets.
    ///
    /// The `newer` changes override `older` where they conflict.
    fn merge_changes(&self, older: ChangeSet, newer: ChangeSet) -> ChangeSet;
}

/// Full state database interface for consensus operations.
///
/// Combines read and write access with additional metadata operations.
pub trait StateDb: StateDbRead + StateDbWrite {
    /// Get the current state root.
    fn state_root(&self) -> Result<B256, StateDbError>;
}
