//! Cryptographic signing and verification for session grants.
//!
//! Uses secp256k1 (via `k256`) with keccak256 hashing to produce
//! Ethereum-compatible recoverable signatures over session grants.

use alloy_primitives::Address;
use k256::ecdsa::{
    RecoveryId, Signature, SigningKey, VerifyingKey, signature::hazmat::PrehashSigner,
};
use monmouth_agent_types::SessionGrant;
use sha3::{Digest, Keccak256};

use crate::error::DelegationError;

/// Sign a session grant, returning a 65-byte recoverable signature.
///
/// The signature format is `[r (32 bytes) | s (32 bytes) | v (1 byte)]`,
/// compatible with Ethereum's `ecrecover`.
///
/// # Errors
///
/// Returns [`DelegationError::InvalidSignature`] if the signing key fails
/// to produce a recoverable signature (should not happen with valid keys).
pub fn sign_session_grant(
    grant: &SessionGrant,
    key: &SigningKey,
) -> Result<Vec<u8>, DelegationError> {
    let hash = hash_grant(grant);
    let (signature, recovery_id): (Signature, RecoveryId) = key
        .sign_prehash(&hash)
        .map_err(|e| DelegationError::InvalidSignature(format!("signing failed: {e}")))?;

    let mut result = Vec::with_capacity(65);
    result.extend_from_slice(&signature.to_bytes());
    result.push(recovery_id.to_byte());
    Ok(result)
}

/// Verify a session grant signature and recover the signer address.
///
/// Expects a 65-byte signature in `[r | s | v]` format.
///
/// # Errors
///
/// Returns [`DelegationError::InvalidSignature`] if the signature is
/// malformed or recovery fails.
pub fn verify_session_grant(
    grant: &SessionGrant,
    signature: &[u8],
) -> Result<Address, DelegationError> {
    if signature.len() != 65 {
        return Err(DelegationError::InvalidSignature(format!(
            "expected 65 bytes, got {}",
            signature.len()
        )));
    }

    let hash = hash_grant(grant);

    let sig = Signature::from_slice(&signature[..64])
        .map_err(|e| DelegationError::InvalidSignature(format!("invalid signature bytes: {e}")))?;

    let recovery_id = RecoveryId::from_byte(signature[64])
        .ok_or_else(|| DelegationError::InvalidSignature("invalid recovery id".to_string()))?;

    let verifying_key = VerifyingKey::recover_from_prehash(&hash, &sig, recovery_id)
        .map_err(|e| DelegationError::InvalidSignature(format!("recovery failed: {e}")))?;

    // Derive Ethereum address from the uncompressed public key.
    let public_key_bytes = verifying_key.to_encoded_point(false);
    let public_key_hash = Keccak256::digest(&public_key_bytes.as_bytes()[1..]);
    let address = Address::from_slice(&public_key_hash[12..]);

    Ok(address)
}

/// Compute a deterministic keccak256 hash of a session grant.
///
/// The hash is computed over the ABI-style packed encoding of all grant
/// fields in a fixed order to ensure determinism.
fn hash_grant(grant: &SessionGrant) -> [u8; 32] {
    let mut hasher = Keccak256::new();

    // Owner (20 bytes).
    hasher.update(grant.owner.as_slice());

    // Delegate (20 bytes).
    hasher.update(grant.delegate.as_slice());

    // Capabilities: hash each capability string, then the count.
    let cap_count = grant.capabilities.len() as u64;
    hasher.update(cap_count.to_be_bytes());
    for cap in &grant.capabilities {
        hasher.update(cap.as_bytes());
    }

    // Spending limit (32 bytes, big-endian).
    hasher.update(grant.spending_limit_wei.to_be_bytes::<32>());

    // Expires at (8 bytes, big-endian).
    hasher.update(grant.expires_at.to_be_bytes());

    // Nonce (8 bytes, big-endian).
    hasher.update(grant.nonce.to_be_bytes());

    hasher.finalize().into()
}

#[cfg(test)]
mod tests {
    use alloy_primitives::{Address, U256};
    use k256::ecdsa::SigningKey;

    use super::*;

    fn test_grant() -> SessionGrant {
        SessionGrant {
            owner: Address::repeat_byte(0x01),
            delegate: Address::repeat_byte(0x02),
            capabilities: vec!["sim.preview".to_string(), "state.read".to_string()],
            spending_limit_wei: U256::from(1_000_000u64),
            expires_at: 1_700_000_000,
            nonce: 1,
        }
    }

    fn test_signing_key() -> SigningKey {
        // Deterministic key for tests.
        let secret = [0x42u8; 32];
        SigningKey::from_bytes((&secret).into()).unwrap()
    }

    #[test]
    fn sign_and_verify_roundtrip() {
        let grant = test_grant();
        let key = test_signing_key();

        let signature = sign_session_grant(&grant, &key).unwrap();
        assert_eq!(signature.len(), 65);

        let recovered = verify_session_grant(&grant, &signature).unwrap();

        // Derive expected address from the test key.
        let verifying_key = key.verifying_key();
        let public_key_bytes = verifying_key.to_encoded_point(false);
        let public_key_hash = Keccak256::digest(&public_key_bytes.as_bytes()[1..]);
        let expected_address = Address::from_slice(&public_key_hash[12..]);

        assert_eq!(recovered, expected_address);
    }

    #[test]
    fn verify_wrong_grant_fails() {
        let grant = test_grant();
        let key = test_signing_key();
        let signature = sign_session_grant(&grant, &key).unwrap();

        // Modify the grant.
        let mut wrong_grant = grant;
        wrong_grant.nonce = 999;

        let recovered = verify_session_grant(&wrong_grant, &signature).unwrap();

        // Recovered address will differ from the expected signer.
        let verifying_key = key.verifying_key();
        let public_key_bytes = verifying_key.to_encoded_point(false);
        let public_key_hash = Keccak256::digest(&public_key_bytes.as_bytes()[1..]);
        let expected_address = Address::from_slice(&public_key_hash[12..]);

        // The signature is still technically valid (it recovers *some* address),
        // but it won't match the expected owner.
        assert_ne!(recovered, expected_address);
    }

    #[test]
    fn verify_invalid_signature_length() {
        let grant = test_grant();
        let err = verify_session_grant(&grant, &[0u8; 64]).unwrap_err();
        assert!(matches!(err, DelegationError::InvalidSignature(_)));
    }

    #[test]
    fn verify_garbage_signature() {
        let grant = test_grant();
        let err = verify_session_grant(&grant, &[0xffu8; 65]).unwrap_err();
        assert!(matches!(err, DelegationError::InvalidSignature(_)));
    }

    #[test]
    fn hash_is_deterministic() {
        let grant = test_grant();
        let h1 = hash_grant(&grant);
        let h2 = hash_grant(&grant);
        assert_eq!(h1, h2);
    }

    #[test]
    fn hash_differs_for_different_grants() {
        let grant1 = test_grant();
        let mut grant2 = test_grant();
        grant2.nonce = 42;
        assert_ne!(hash_grant(&grant1), hash_grant(&grant2));
    }
}
