//! Cryptographic verification for attestation payloads.
//!
//! Secp256k1 signatures are verified natively using `k256`. Other
//! attestation types (Ed25519, TEE quotes, ZK proofs) are accepted
//! but marked as unverified — plug in verifiers as needed.

use alloy_primitives::Address;
use k256::ecdsa::{RecoveryId, Signature, VerifyingKey};
use monmouth_agent_types::{Attestation, AttestationType};
use sha3::{Digest, Keccak256};
use tracing::warn;

use crate::AttestationError;

/// Verify an attestation's cryptographic payload.
///
/// For secp256k1 signatures, this recovers the signer address from the
/// payload and checks it matches the attester. The payload must be a
/// 65-byte recoverable signature `[r (32) | s (32) | v (1)]` over the
/// subject hash.
///
/// For other attestation types, returns `Err(UnsupportedType)` —
/// unverified attestations are rejected until a verifier plugin is registered.
///
/// # Returns
///
/// `Ok(true)` if cryptographically verified.
///
/// # Errors
///
/// Returns an error if the type is unsupported, the payload is malformed,
/// or verification fails.
pub fn verify_attestation(attestation: &Attestation) -> Result<bool, AttestationError> {
    match attestation.attestation_type {
        AttestationType::Secp256k1Signature => {
            verify_secp256k1(attestation)?;
            Ok(true)
        }
        // Reject unverified types — these need external verifier plugins.
        typ @ (AttestationType::Ed25519Signature
        | AttestationType::TeeQuote
        | AttestationType::ZkProof) => {
            warn!(
                id = ?attestation.id,
                attester = ?attestation.attester,
                attestation_type = ?typ,
                "rejecting attestation — no verifier for this type"
            );
            Err(AttestationError::UnsupportedType(typ))
        }
    }
}

/// Verify a secp256k1 attestation by recovering the signer from the
/// payload and checking it matches the attester address.
fn verify_secp256k1(attestation: &Attestation) -> Result<(), AttestationError> {
    let payload = &attestation.payload;

    if payload.len() != 65 {
        return Err(AttestationError::MalformedPayload(format!(
            "secp256k1 signature must be 65 bytes, got {}",
            payload.len()
        )));
    }

    let sig = Signature::from_slice(&payload[..64]).map_err(|e| {
        AttestationError::MalformedPayload(format!("invalid signature bytes: {e}"))
    })?;

    let recovery_id = RecoveryId::from_byte(payload[64]).ok_or_else(|| {
        AttestationError::MalformedPayload("invalid recovery id byte".to_string())
    })?;

    // The subject_hash is the message that was signed.
    let prehash: [u8; 32] = attestation.subject_hash.0;

    let verifying_key =
        VerifyingKey::recover_from_prehash(&prehash, &sig, recovery_id).map_err(|e| {
            AttestationError::VerificationFailed {
                id: attestation.id,
                reason: format!("key recovery failed: {e}"),
            }
        })?;

    // Derive Ethereum address from the recovered public key.
    let public_key_bytes = verifying_key.to_encoded_point(false);
    let public_key_hash = Keccak256::digest(&public_key_bytes.as_bytes()[1..]);
    let recovered_address = Address::from_slice(&public_key_hash[12..]);

    if recovered_address != attestation.attester {
        return Err(AttestationError::VerificationFailed {
            id: attestation.id,
            reason: format!(
                "signer mismatch: recovered {recovered_address}, expected {}",
                attestation.attester
            ),
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use alloy_primitives::{Address, B256};
    use k256::ecdsa::{SigningKey, signature::hazmat::PrehashSigner};
    use monmouth_agent_types::AttestationId;

    use super::*;

    fn test_signing_key() -> SigningKey {
        let secret = [0x42u8; 32];
        SigningKey::from_bytes((&secret).into()).unwrap()
    }

    fn address_from_key(key: &SigningKey) -> Address {
        let verifying_key = key.verifying_key();
        let public_key_bytes = verifying_key.to_encoded_point(false);
        let public_key_hash = Keccak256::digest(&public_key_bytes.as_bytes()[1..]);
        Address::from_slice(&public_key_hash[12..])
    }

    fn sign_subject(key: &SigningKey, subject_hash: B256) -> Vec<u8> {
        let prehash: [u8; 32] = subject_hash.0;
        let (sig, rid): (Signature, RecoveryId) =
            key.sign_prehash(&prehash).expect("signing is infallible");
        let mut result = Vec::with_capacity(65);
        result.extend_from_slice(&sig.to_bytes());
        result.push(rid.to_byte());
        result
    }

    #[test]
    fn secp256k1_verification_success() {
        let key = test_signing_key();
        let attester = address_from_key(&key);
        let subject_hash = B256::repeat_byte(0xAB);
        let payload = sign_subject(&key, subject_hash);

        let attestation = Attestation {
            id: AttestationId(B256::repeat_byte(1)),
            attestation_type: AttestationType::Secp256k1Signature,
            attester,
            subject_hash,
            payload,
            timestamp: 1_700_000_000,
            verified: false,
        };

        assert!(verify_attestation(&attestation).unwrap());
    }

    #[test]
    fn secp256k1_wrong_attester() {
        let key = test_signing_key();
        let wrong_attester = Address::repeat_byte(0xFF);
        let subject_hash = B256::repeat_byte(0xAB);
        let payload = sign_subject(&key, subject_hash);

        let attestation = Attestation {
            id: AttestationId(B256::repeat_byte(1)),
            attestation_type: AttestationType::Secp256k1Signature,
            attester: wrong_attester,
            subject_hash,
            payload,
            timestamp: 1_700_000_000,
            verified: false,
        };

        let err = verify_attestation(&attestation).unwrap_err();
        assert!(matches!(err, AttestationError::VerificationFailed { .. }));
    }

    #[test]
    fn secp256k1_wrong_payload_length() {
        let attestation = Attestation {
            id: AttestationId(B256::repeat_byte(1)),
            attestation_type: AttestationType::Secp256k1Signature,
            attester: Address::ZERO,
            subject_hash: B256::ZERO,
            payload: vec![0u8; 32], // too short
            timestamp: 1_700_000_000,
            verified: false,
        };

        let err = verify_attestation(&attestation).unwrap_err();
        assert!(matches!(err, AttestationError::MalformedPayload(_)));
    }

    #[test]
    fn secp256k1_garbage_payload() {
        let attestation = Attestation {
            id: AttestationId(B256::repeat_byte(1)),
            attestation_type: AttestationType::Secp256k1Signature,
            attester: Address::ZERO,
            subject_hash: B256::ZERO,
            payload: vec![0xFFu8; 65],
            timestamp: 1_700_000_000,
            verified: false,
        };

        let err = verify_attestation(&attestation).unwrap_err();
        // Could be MalformedPayload or VerificationFailed depending on bytes.
        assert!(
            matches!(err, AttestationError::MalformedPayload(_))
                || matches!(err, AttestationError::VerificationFailed { .. })
        );
    }

    #[test]
    fn ed25519_rejected_unsupported() {
        let attestation = Attestation {
            id: AttestationId(B256::repeat_byte(1)),
            attestation_type: AttestationType::Ed25519Signature,
            attester: Address::ZERO,
            subject_hash: B256::ZERO,
            payload: vec![0u8; 64],
            timestamp: 1_700_000_000,
            verified: false,
        };

        let err = verify_attestation(&attestation).unwrap_err();
        assert!(matches!(err, AttestationError::UnsupportedType(AttestationType::Ed25519Signature)));
    }

    #[test]
    fn tee_quote_rejected_unsupported() {
        let attestation = Attestation {
            id: AttestationId(B256::repeat_byte(1)),
            attestation_type: AttestationType::TeeQuote,
            attester: Address::ZERO,
            subject_hash: B256::ZERO,
            payload: vec![0u8; 128],
            timestamp: 1_700_000_000,
            verified: false,
        };

        let err = verify_attestation(&attestation).unwrap_err();
        assert!(matches!(err, AttestationError::UnsupportedType(AttestationType::TeeQuote)));
    }

    #[test]
    fn zk_proof_rejected_unsupported() {
        let attestation = Attestation {
            id: AttestationId(B256::repeat_byte(1)),
            attestation_type: AttestationType::ZkProof,
            attester: Address::ZERO,
            subject_hash: B256::ZERO,
            payload: vec![0u8; 256],
            timestamp: 1_700_000_000,
            verified: false,
        };

        let err = verify_attestation(&attestation).unwrap_err();
        assert!(matches!(err, AttestationError::UnsupportedType(AttestationType::ZkProof)));
    }
}
