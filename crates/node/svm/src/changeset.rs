//! SVM state change tracking.
//!
//! Maps Solana account mutations from transaction execution into a structured
//! changeset that can be applied to storage.

use std::collections::BTreeMap;

/// A single account update from SVM execution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SvmAccountUpdate {
    /// Account balance in lamports.
    pub lamports: u64,
    /// Account data bytes.
    pub data: Vec<u8>,
    /// Program owner of this account.
    pub owner: [u8; 32],
    /// Whether this account is executable (a program).
    pub executable: bool,
    /// Rent epoch (deprecated in Solana but still part of the account model).
    pub rent_epoch: u64,
}

/// Set of SVM account changes from a block or transaction batch.
///
/// Keys are 32-byte Solana public keys. Separate from EVM's `ChangeSet`
/// which uses 20-byte addresses.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SvmChangeSet {
    /// Account updates keyed by 32-byte public key.
    pub accounts: BTreeMap<[u8; 32], SvmAccountUpdate>,
}

impl SvmChangeSet {
    /// Create an empty changeset.
    #[must_use]
    pub fn new() -> Self {
        Self { accounts: BTreeMap::new() }
    }

    /// Returns true if there are no account changes.
    pub fn is_empty(&self) -> bool {
        self.accounts.is_empty()
    }

    /// Number of account changes.
    pub fn len(&self) -> usize {
        self.accounts.len()
    }

    /// Insert or overwrite an account update.
    pub fn insert(&mut self, pubkey: [u8; 32], update: SvmAccountUpdate) {
        self.accounts.insert(pubkey, update);
    }

    /// Merge another changeset into this one. Later values overwrite earlier ones.
    pub fn merge(&mut self, other: Self) {
        for (pubkey, update) in other.accounts {
            self.accounts.insert(pubkey, update);
        }
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
    fn empty_changeset() {
        let cs = SvmChangeSet::new();
        assert!(cs.is_empty());
        assert_eq!(cs.len(), 0);
    }

    #[test]
    fn insert_and_len() {
        let mut cs = SvmChangeSet::new();
        cs.insert([1u8; 32], dummy_update(100));
        assert_eq!(cs.len(), 1);
        assert!(!cs.is_empty());
    }

    #[test]
    fn merge_overwrites() {
        let mut cs1 = SvmChangeSet::new();
        cs1.insert([1u8; 32], dummy_update(100));

        let mut cs2 = SvmChangeSet::new();
        cs2.insert([1u8; 32], dummy_update(200));
        cs2.insert([2u8; 32], dummy_update(300));

        cs1.merge(cs2);
        assert_eq!(cs1.len(), 2);
        assert_eq!(cs1.accounts[&[1u8; 32]].lamports, 200);
        assert_eq!(cs1.accounts[&[2u8; 32]].lamports, 300);
    }
}
