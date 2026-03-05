//! Account bridge for SVM transaction processing.
//!
//! Implements `TransactionProcessingCallback` over an in-memory `BTreeMap`
//! so the SVM processor can read account state during execution.
//! Phase 2 swaps this for a QMDB-backed implementation.

use std::collections::BTreeMap;

use solana_account::AccountSharedData;
use solana_pubkey::Pubkey;
use solana_svm_callback::{AccountState, InvokeContextCallback, TransactionProcessingCallback};

/// In-memory account store implementing the SVM callback interface.
///
/// Wraps a `BTreeMap<Pubkey, AccountSharedData>` for Phase 1.
/// Phase 2 replaces the inner map with QMDB-backed reads.
#[derive(Clone, Debug, Default)]
pub struct SvmAccountBridge {
    accounts: BTreeMap<Pubkey, AccountSharedData>,
}

impl SvmAccountBridge {
    /// Create a new bridge with the given account map.
    #[must_use]
    pub const fn new(accounts: BTreeMap<Pubkey, AccountSharedData>) -> Self {
        Self { accounts }
    }

    /// Create an empty bridge (no pre-existing accounts).
    #[must_use]
    pub const fn empty() -> Self {
        Self { accounts: BTreeMap::new() }
    }

    /// Insert an account into the bridge.
    pub fn set_account(&mut self, pubkey: Pubkey, account: AccountSharedData) {
        self.accounts.insert(pubkey, account);
    }

    /// Get a reference to an account.
    #[must_use]
    pub fn get_account(&self, pubkey: &Pubkey) -> Option<&AccountSharedData> {
        self.accounts.get(pubkey)
    }

    /// Number of accounts in the bridge.
    pub fn len(&self) -> usize {
        self.accounts.len()
    }

    /// Whether the bridge is empty.
    pub fn is_empty(&self) -> bool {
        self.accounts.is_empty()
    }
}

impl InvokeContextCallback for SvmAccountBridge {
    fn get_epoch_stake(&self) -> u64 {
        // Monmouth doesn't have stake-weighted consensus in the Solana sense.
        // Return a fixed value so SVM programs that query stake don't fail.
        1_000_000_000
    }

    fn get_epoch_stake_for_vote_account(&self, _vote_address: &Pubkey) -> u64 {
        0
    }

    fn is_precompile(&self, _program_id: &Pubkey) -> bool {
        // No Solana precompiles in Monmouth's SVM — keep it minimal.
        false
    }
}

impl TransactionProcessingCallback for SvmAccountBridge {
    fn get_account_shared_data(
        &self,
        pubkey: &Pubkey,
    ) -> Option<(AccountSharedData, /* slot */ u64)> {
        self.accounts.get(pubkey).map(|acct| (acct.clone(), 0))
    }

    fn inspect_account(
        &self,
        _address: &Pubkey,
        _account_state: AccountState<'_>,
        _is_writable: bool,
    ) {
        // No-op for now. Could add metrics/tracing in Phase 4.
    }
}

#[cfg(test)]
mod tests {
    use solana_account::{Account, ReadableAccount};

    use super::*;

    fn system_program() -> Pubkey {
        Pubkey::default()
    }

    fn funded_account(lamports: u64) -> AccountSharedData {
        AccountSharedData::from(Account {
            lamports,
            data: vec![],
            owner: system_program(),
            executable: false,
            rent_epoch: 0,
        })
    }

    #[test]
    fn empty_bridge_returns_none() {
        let bridge = SvmAccountBridge::empty();
        assert!(bridge.get_account_shared_data(&Pubkey::new_unique()).is_none());
    }

    #[test]
    fn set_and_get_account() {
        let mut bridge = SvmAccountBridge::empty();
        let pk = Pubkey::new_unique();
        bridge.set_account(pk, funded_account(1_000_000));

        let (acct, slot) = bridge.get_account_shared_data(&pk).unwrap();
        assert_eq!(acct.lamports(), 1_000_000);
        assert_eq!(slot, 0);
    }

    #[test]
    fn bridge_from_map() {
        let pk = Pubkey::new_unique();
        let mut map = BTreeMap::new();
        map.insert(pk, funded_account(500));

        let bridge = SvmAccountBridge::new(map);
        assert_eq!(bridge.len(), 1);
        assert!(bridge.get_account(&pk).is_some());
    }

    #[test]
    fn epoch_stake_nonzero() {
        let bridge = SvmAccountBridge::empty();
        assert!(bridge.get_epoch_stake() > 0);
    }
}
