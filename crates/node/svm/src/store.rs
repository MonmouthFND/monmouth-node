//! SVM account state store.
//!
//! Provides a persistent-ready SVM account store that tracks account state
//! across blocks. Initially backed by an in-memory `BTreeMap`; designed to
//! be swapped for a QMDB-backed implementation in Phase 3+.

use std::{
    collections::BTreeMap,
    sync::{Arc, RwLock},
};

use alloy_primitives::{B256, keccak256};

use crate::{
    SvmError,
    changeset::{SvmAccountUpdate, SvmChangeSet},
};

/// Default maximum number of accounts the SVM store will hold.
pub const DEFAULT_MAX_ACCOUNTS: usize = 1_000_000;

/// In-memory SVM account store.
///
/// Tracks Solana account state keyed by 32-byte public keys.
/// Thread-safe via `Arc<RwLock<...>>` for use from consensus/runner.
#[derive(Clone, Debug)]
pub struct SvmStateStore {
    accounts: Arc<RwLock<BTreeMap<[u8; 32], SvmAccountUpdate>>>,
    max_accounts: usize,
}

impl SvmStateStore {
    /// Create a new empty store with default capacity limits.
    #[must_use]
    pub fn new() -> Self {
        Self {
            accounts: Arc::new(RwLock::new(BTreeMap::new())),
            max_accounts: DEFAULT_MAX_ACCOUNTS,
        }
    }

    /// Set the maximum number of accounts the store will accept.
    #[must_use]
    pub const fn with_max_accounts(mut self, max: usize) -> Self {
        self.max_accounts = max;
        self
    }

    /// Get an account by public key.
    pub fn get_account(&self, pubkey: &[u8; 32]) -> Option<SvmAccountUpdate> {
        self.accounts.read().ok()?.get(pubkey).cloned()
    }

    /// Apply a changeset to the store.
    ///
    /// New accounts that would push the store past `max_accounts` are
    /// logged and skipped. Updates to existing accounts are always applied.
    ///
    /// Returns the number of accounts actually written.
    ///
    /// # Errors
    ///
    /// Returns `SvmError::LockPoisoned` if the internal lock is poisoned.
    pub fn apply_changes(&self, changes: &SvmChangeSet) -> Result<usize, SvmError> {
        let mut accounts = self
            .accounts
            .write()
            .map_err(|e| SvmError::LockPoisoned(format!("apply_changes: {e}")))?;
        let mut written = 0usize;
        let mut dropped = 0usize;
        for (pubkey, update) in &changes.accounts {
            if accounts.contains_key(pubkey) || accounts.len() < self.max_accounts {
                accounts.insert(*pubkey, update.clone());
                written += 1;
            } else {
                dropped += 1;
            }
        }
        if dropped > 0 {
            tracing::warn!(
                dropped,
                max = self.max_accounts,
                "apply_changes: new accounts dropped (at capacity)"
            );
        }
        Ok(written)
    }

    /// Compute the SVM state root from the current account state.
    ///
    /// Hashes all accounts deterministically (BTreeMap guarantees sorted order)
    /// to produce a single 32-byte accounts root, then wraps it with the
    /// SVM namespace prefix via `StateRoot::compute_svm`.
    ///
    /// # Errors
    ///
    /// Returns `SvmError::LockPoisoned` if the internal lock is poisoned.
    pub fn compute_root(&self, pending_changes: &SvmChangeSet) -> Result<B256, SvmError> {
        let accounts = self
            .accounts
            .read()
            .map_err(|e| SvmError::LockPoisoned(format!("compute_root: {e}")))?;

        // Merge current state with pending changes
        let mut merged = accounts.clone();
        for (pubkey, update) in &pending_changes.accounts {
            merged.insert(*pubkey, update.clone());
        }

        if merged.is_empty() {
            return Ok(B256::ZERO);
        }

        // Hash all accounts in sorted order to produce the accounts root
        let accounts_root = Self::hash_accounts(&merged);

        // Wrap with SVM namespace
        Ok(monmouth_qmdb::StateRoot::compute_svm(accounts_root))
    }

    /// Hash all accounts deterministically.
    fn hash_accounts(accounts: &BTreeMap<[u8; 32], SvmAccountUpdate>) -> B256 {
        let mut hasher_input = Vec::new();
        for (pubkey, update) in accounts {
            hasher_input.extend_from_slice(pubkey);
            hasher_input.extend_from_slice(&update.lamports.to_le_bytes());
            hasher_input.extend_from_slice(&(update.data.len() as u64).to_le_bytes());
            hasher_input.extend_from_slice(&update.data);
            hasher_input.extend_from_slice(&update.owner);
            hasher_input.push(u8::from(update.executable));
            hasher_input.extend_from_slice(&update.rent_epoch.to_le_bytes());
        }
        keccak256(hasher_input)
    }

    /// Convert the current store state into an `SvmAccountBridge` for the executor.
    ///
    /// Translates `[u8; 32]` keys to `Pubkey` and `SvmAccountUpdate` to
    /// `AccountSharedData`. Also injects builtin program accounts (system
    /// program, BPF loader, compute budget) required by the processor.
    ///
    /// # Errors
    ///
    /// Returns `SvmError::LockPoisoned` if the internal lock is poisoned.
    pub fn to_bridge(&self) -> Result<crate::SvmAccountBridge, SvmError> {
        use solana_account::{Account, AccountSharedData};
        use solana_pubkey::Pubkey;

        let accounts =
            self.accounts.read().map_err(|e| SvmError::LockPoisoned(format!("to_bridge: {e}")))?;
        let mut map = BTreeMap::new();

        for (key, update) in accounts.iter() {
            let pubkey = Pubkey::new_from_array(*key);
            let account = AccountSharedData::from(Account {
                lamports: update.lamports,
                data: update.data.clone(),
                owner: Pubkey::new_from_array(update.owner),
                executable: update.executable,
                rent_epoch: update.rent_epoch,
            });
            map.insert(pubkey, account);
        }

        // Inject builtin program accounts so the processor can find them.
        let native_loader = solana_sdk_ids::native_loader::id();
        let builtins = [
            solana_system_program::id(),
            solana_sdk_ids::bpf_loader_upgradeable::id(),
            solana_sdk_ids::compute_budget::id(),
        ];
        for program_id in builtins {
            map.entry(program_id).or_insert_with(|| {
                AccountSharedData::from(Account {
                    lamports: 1,
                    data: vec![],
                    owner: native_loader,
                    executable: true,
                    rent_epoch: 0,
                })
            });
        }

        Ok(crate::SvmAccountBridge::new(map))
    }

    /// Number of accounts in the store.
    pub fn len(&self) -> usize {
        self.accounts.read().map(|a| a.len()).unwrap_or(0)
    }

    /// Returns true if the store has no accounts.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for SvmStateStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_update(lamports: u64) -> SvmAccountUpdate {
        SvmAccountUpdate {
            lamports,
            data: vec![],
            owner: [0u8; 32],
            executable: false,
            rent_epoch: 0,
        }
    }

    #[test]
    fn empty_store() {
        let store = SvmStateStore::new();
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        assert!(store.get_account(&[1u8; 32]).is_none());
    }

    #[test]
    fn apply_and_get() {
        let store = SvmStateStore::new();
        let mut changes = SvmChangeSet::new();
        changes.insert([1u8; 32], dummy_update(100));
        changes.insert([2u8; 32], dummy_update(200));

        store.apply_changes(&changes).unwrap();

        assert_eq!(store.len(), 2);
        assert_eq!(store.get_account(&[1u8; 32]).unwrap().lamports, 100);
        assert_eq!(store.get_account(&[2u8; 32]).unwrap().lamports, 200);
    }

    #[test]
    fn apply_overwrites() {
        let store = SvmStateStore::new();

        let mut changes1 = SvmChangeSet::new();
        changes1.insert([1u8; 32], dummy_update(100));
        store.apply_changes(&changes1).unwrap();

        let mut changes2 = SvmChangeSet::new();
        changes2.insert([1u8; 32], dummy_update(999));
        store.apply_changes(&changes2).unwrap();

        assert_eq!(store.get_account(&[1u8; 32]).unwrap().lamports, 999);
    }

    #[test]
    fn compute_root_empty() {
        let store = SvmStateStore::new();
        let root = store.compute_root(&SvmChangeSet::new()).unwrap();
        assert_eq!(root, B256::ZERO);
    }

    #[test]
    fn compute_root_deterministic() {
        let store = SvmStateStore::new();
        let mut changes = SvmChangeSet::new();
        changes.insert([1u8; 32], dummy_update(100));

        let root1 = store.compute_root(&changes).unwrap();
        let root2 = store.compute_root(&changes).unwrap();
        assert_eq!(root1, root2);
        assert_ne!(root1, B256::ZERO);
    }

    #[test]
    fn compute_root_different_changes() {
        let store = SvmStateStore::new();

        let mut changes1 = SvmChangeSet::new();
        changes1.insert([1u8; 32], dummy_update(100));

        let mut changes2 = SvmChangeSet::new();
        changes2.insert([1u8; 32], dummy_update(200));

        assert_ne!(store.compute_root(&changes1).unwrap(), store.compute_root(&changes2).unwrap());
    }

    #[test]
    fn compute_root_merges_with_existing() {
        let store = SvmStateStore::new();

        // Apply some base state
        let mut base = SvmChangeSet::new();
        base.insert([1u8; 32], dummy_update(100));
        store.apply_changes(&base).unwrap();

        // Compute root with additional changes
        let mut pending = SvmChangeSet::new();
        pending.insert([2u8; 32], dummy_update(200));

        let root = store.compute_root(&pending).unwrap();
        assert_ne!(root, B256::ZERO);

        // Root with both accounts should differ from root with just one
        let root_base_only = store.compute_root(&SvmChangeSet::new()).unwrap();
        assert_ne!(root, root_base_only);
    }

    #[test]
    fn max_accounts_enforced() {
        let store = SvmStateStore::new().with_max_accounts(2);

        let mut changes = SvmChangeSet::new();
        changes.insert([1u8; 32], dummy_update(100));
        changes.insert([2u8; 32], dummy_update(200));
        changes.insert([3u8; 32], dummy_update(300)); // would exceed limit
        store.apply_changes(&changes).unwrap();

        assert_eq!(store.len(), 2);
        // First two inserted, third silently dropped.
        assert!(store.get_account(&[1u8; 32]).is_some());
        assert!(store.get_account(&[2u8; 32]).is_some());
        assert!(store.get_account(&[3u8; 32]).is_none());
    }

    #[test]
    fn max_accounts_allows_updates() {
        let store = SvmStateStore::new().with_max_accounts(1);

        let mut changes1 = SvmChangeSet::new();
        changes1.insert([1u8; 32], dummy_update(100));
        store.apply_changes(&changes1).unwrap();
        assert_eq!(store.len(), 1);

        // Update existing account should succeed even at capacity.
        let mut changes2 = SvmChangeSet::new();
        changes2.insert([1u8; 32], dummy_update(999));
        store.apply_changes(&changes2).unwrap();
        assert_eq!(store.get_account(&[1u8; 32]).unwrap().lamports, 999);
    }

    #[test]
    fn clone_shares_state() {
        let store = SvmStateStore::new();
        let clone = store.clone();

        let mut changes = SvmChangeSet::new();
        changes.insert([1u8; 32], dummy_update(42));
        store.apply_changes(&changes).unwrap();

        assert_eq!(clone.get_account(&[1u8; 32]).unwrap().lamports, 42);
    }
}
