//! Deserialize raw Solana transaction bytes into `SanitizedTransaction`.
//!
//! Converts the `inner_tx` bytes from a Monmouth agent envelope into
//! the `SanitizedTransaction` type expected by `SvmExecutor::execute()`.

use std::collections::HashSet;

use solana_message::SimpleAddressLoader;
use solana_pubkey::Pubkey;
use solana_transaction::{
    sanitized::SanitizedTransaction, versioned::VersionedTransaction,
};

use crate::SvmError;

/// Deserialize raw Solana wire-format bytes into a `SanitizedTransaction`.
///
/// Expects `raw` to be `bincode`-encoded `VersionedTransaction` bytes
/// (the standard Solana transaction wire format).
pub fn deserialize_svm_tx(raw: &[u8]) -> Result<SanitizedTransaction, SvmError> {
    // Step 1: bincode → VersionedTransaction
    let vtx: VersionedTransaction =
        bincode::deserialize(raw).map_err(|e| SvmError::TxDecode(format!("bincode: {e}")))?;

    // Step 2: VersionedTransaction → SanitizedTransaction
    // - MessageHash::Compute: derive the message hash via blake3
    // - is_simple_vote_tx: false (Monmouth has no vote transactions)
    // - SimpleAddressLoader::Disabled: no address lookup tables
    // - empty reserved keys: Monmouth doesn't reserve Solana system keys
    use solana_transaction::sanitized::MessageHash;

    SanitizedTransaction::try_create(
        vtx,
        MessageHash::Compute,
        Some(false),
        SimpleAddressLoader::Disabled,
        &HashSet::<Pubkey>::new(),
    )
    .map_err(|e| SvmError::TxDecode(format!("sanitize: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_bytes_returns_error() {
        let garbage = &[0xDE, 0xAD, 0xBE, 0xEF];
        let result = deserialize_svm_tx(garbage);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("bincode"), "expected bincode error, got: {err}");
    }

    #[test]
    fn empty_bytes_returns_error() {
        let result = deserialize_svm_tx(&[]);
        assert!(result.is_err());
    }
}
