//! Thread-safe attestation registry.

use std::{
    collections::HashMap,
    sync::Arc,
};

use alloy_primitives::{Address, B256};
use monmouth_agent_types::{Attestation, AttestationId};
use parking_lot::RwLock;
use tracing::{debug, info};

use crate::{AttestationError, verify_attestation};

/// Default maximum number of attestations the registry will hold.
pub const DEFAULT_MAX_ATTESTATIONS: usize = 100_000;

/// Internal state protected by the lock.
#[derive(Debug)]
struct Inner {
    /// Primary index: attestation ID to attestation.
    attestations: HashMap<AttestationId, Attestation>,
    /// Secondary index: attester address to attestation IDs.
    by_attester: HashMap<Address, Vec<AttestationId>>,
    /// Secondary index: subject hash to attestation IDs.
    by_subject: HashMap<B256, Vec<AttestationId>>,
}

/// Thread-safe registry for cryptographic attestations.
///
/// Stores attestations with optional verification on submission.
/// Attestations are indexed by ID, attester address, and subject hash.
#[derive(Debug, Clone)]
pub struct AttestationRegistry {
    inner: Arc<RwLock<Inner>>,
    max_attestations: usize,
}

impl Default for AttestationRegistry {
    fn default() -> Self {
        Self {
            inner: Arc::new(RwLock::new(Inner {
                attestations: HashMap::new(),
                by_attester: HashMap::new(),
                by_subject: HashMap::new(),
            })),
            max_attestations: DEFAULT_MAX_ATTESTATIONS,
        }
    }
}

impl AttestationRegistry {
    /// Create a new, empty attestation registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the maximum number of attestations the registry will hold.
    #[must_use]
    pub const fn with_max_attestations(mut self, max: usize) -> Self {
        self.max_attestations = max;
        self
    }

    /// Submit a new attestation, optionally verifying it.
    ///
    /// If `verify` is true, the attestation payload is checked before
    /// storage. For secp256k1, this means the recovered signer must match
    /// the attester. For unsupported types, the attestation is accepted
    /// but marked as unverified.
    ///
    /// # Errors
    ///
    /// Returns an error if the registry is at capacity, the ID is a
    /// duplicate, or verification fails.
    pub fn submit(
        &self,
        mut attestation: Attestation,
        verify: bool,
    ) -> Result<(), AttestationError> {
        if verify {
            let verified = verify_attestation(&attestation)?;
            attestation.verified = verified;
        }

        let mut inner = self.inner.write();

        if inner.attestations.len() >= self.max_attestations {
            return Err(AttestationError::CapacityExceeded(self.max_attestations));
        }

        if inner.attestations.contains_key(&attestation.id) {
            return Err(AttestationError::Duplicate(attestation.id));
        }

        info!(
            id = %attestation.id,
            attester = %attestation.attester,
            verified = attestation.verified,
            attestation_type = ?attestation.attestation_type,
            "Attestation submitted"
        );

        let id = attestation.id;
        let attester = attestation.attester;
        let subject = attestation.subject_hash;

        inner.by_attester.entry(attester).or_default().push(id);
        inner.by_subject.entry(subject).or_default().push(id);
        inner.attestations.insert(id, attestation);

        Ok(())
    }

    /// Look up an attestation by ID.
    #[must_use]
    pub fn get(&self, id: AttestationId) -> Option<Attestation> {
        self.inner.read().attestations.get(&id).cloned()
    }

    /// List attestations from a specific attester.
    #[must_use]
    pub fn list_by_attester(&self, attester: Address) -> Vec<Attestation> {
        let inner = self.inner.read();
        inner
            .by_attester
            .get(&attester)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| inner.attestations.get(id).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// List attestations for a specific subject hash.
    #[must_use]
    pub fn list_by_subject(&self, subject_hash: B256) -> Vec<Attestation> {
        let inner = self.inner.read();
        inner
            .by_subject
            .get(&subject_hash)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| inner.attestations.get(id).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Verify an existing attestation in the registry.
    ///
    /// Re-runs verification and updates the `verified` flag.
    ///
    /// # Errors
    ///
    /// Returns an error if the attestation is not found or verification fails.
    pub fn verify(&self, id: AttestationId) -> Result<bool, AttestationError> {
        let mut inner = self.inner.write();

        let attestation = inner
            .attestations
            .get_mut(&id)
            .ok_or(AttestationError::NotFound(id))?;

        let verified = verify_attestation(attestation)?;
        attestation.verified = verified;

        debug!(id = %id, verified, "Attestation verification result");

        Ok(verified)
    }

    /// Returns the total number of attestations in the registry.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.read().attestations.len()
    }

    /// Returns `true` if the registry contains no attestations.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.read().attestations.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use alloy_primitives::Address;
    use k256::ecdsa::{RecoveryId, Signature, SigningKey, signature::hazmat::PrehashSigner};
    use monmouth_agent_types::AttestationType;
    use sha3::{Digest, Keccak256};

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

    fn att_id(byte: u8) -> AttestationId {
        AttestationId(B256::repeat_byte(byte))
    }

    fn secp256k1_attestation(id: AttestationId) -> Attestation {
        let key = test_signing_key();
        let attester = address_from_key(&key);
        let subject_hash = B256::repeat_byte(0xAB);
        let payload = sign_subject(&key, subject_hash);

        Attestation {
            id,
            attestation_type: AttestationType::Secp256k1Signature,
            attester,
            subject_hash,
            payload,
            timestamp: 1_700_000_000,
            verified: false,
        }
    }

    fn unverifiable_attestation(id: AttestationId) -> Attestation {
        Attestation {
            id,
            attestation_type: AttestationType::TeeQuote,
            attester: Address::repeat_byte(0xCC),
            subject_hash: B256::repeat_byte(0xDD),
            payload: vec![0u8; 128],
            timestamp: 1_700_000_000,
            verified: false,
        }
    }

    #[test]
    fn submit_and_get_with_verification() {
        let reg = AttestationRegistry::new();
        reg.submit(secp256k1_attestation(att_id(1)), true).unwrap();

        let att = reg.get(att_id(1)).unwrap();
        assert!(att.verified);
    }

    #[test]
    fn submit_without_verification() {
        let reg = AttestationRegistry::new();
        reg.submit(secp256k1_attestation(att_id(1)), false).unwrap();

        let att = reg.get(att_id(1)).unwrap();
        assert!(!att.verified);
    }

    #[test]
    fn submit_unverifiable_type_rejected() {
        let reg = AttestationRegistry::new();
        let err = reg.submit(unverifiable_attestation(att_id(1)), true).unwrap_err();

        // Unsupported types are now rejected outright.
        assert!(matches!(err, AttestationError::UnsupportedType(_)));
        assert!(reg.get(att_id(1)).is_none());
    }

    #[test]
    fn submit_bad_secp256k1_fails_verification() {
        let reg = AttestationRegistry::new();
        let mut att = secp256k1_attestation(att_id(1));
        att.attester = Address::repeat_byte(0xFF); // wrong attester

        let err = reg.submit(att, true).unwrap_err();
        assert!(matches!(err, AttestationError::VerificationFailed { .. }));
    }

    #[test]
    fn duplicate_rejected() {
        let reg = AttestationRegistry::new();
        reg.submit(secp256k1_attestation(att_id(1)), false).unwrap();

        let err = reg.submit(secp256k1_attestation(att_id(1)), false).unwrap_err();
        assert!(matches!(err, AttestationError::Duplicate(_)));
    }

    #[test]
    fn verify_existing_attestation() {
        let reg = AttestationRegistry::new();
        reg.submit(secp256k1_attestation(att_id(1)), false).unwrap();

        assert!(!reg.get(att_id(1)).unwrap().verified);

        let verified = reg.verify(att_id(1)).unwrap();
        assert!(verified);
        assert!(reg.get(att_id(1)).unwrap().verified);
    }

    #[test]
    fn list_by_attester() {
        let reg = AttestationRegistry::new();
        let att1 = secp256k1_attestation(att_id(1));
        let attester = att1.attester;
        reg.submit(att1, false).unwrap();

        let mut att2 = secp256k1_attestation(att_id(2));
        att2.subject_hash = B256::repeat_byte(0xCD);
        att2.payload = sign_subject(&test_signing_key(), att2.subject_hash);
        reg.submit(att2, false).unwrap();

        reg.submit(unverifiable_attestation(att_id(3)), false).unwrap();

        assert_eq!(reg.list_by_attester(attester).len(), 2);
    }

    #[test]
    fn list_by_subject() {
        let reg = AttestationRegistry::new();
        let subject = B256::repeat_byte(0xAB);
        reg.submit(secp256k1_attestation(att_id(1)), false).unwrap();

        assert_eq!(reg.list_by_subject(subject).len(), 1);
        assert!(reg.list_by_subject(B256::ZERO).is_empty());
    }

    #[test]
    fn capacity_exceeded() {
        let reg = AttestationRegistry::new().with_max_attestations(1);
        reg.submit(secp256k1_attestation(att_id(1)), false).unwrap();

        let err = reg.submit(unverifiable_attestation(att_id(2)), false).unwrap_err();
        assert!(matches!(err, AttestationError::CapacityExceeded(1)));
    }

    #[test]
    fn len_and_is_empty() {
        let reg = AttestationRegistry::new();
        assert!(reg.is_empty());
        assert_eq!(reg.len(), 0);

        reg.submit(secp256k1_attestation(att_id(1)), false).unwrap();
        assert!(!reg.is_empty());
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn thread_safety() {
        let reg = Arc::new(AttestationRegistry::new());
        let mut handles = vec![];

        for i in 0..10u8 {
            let r = Arc::clone(&reg);
            handles.push(std::thread::spawn(move || {
                r.submit(unverifiable_attestation(att_id(i)), false).unwrap();
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(reg.len(), 10);
    }

    #[test]
    fn error_codes() {
        assert_eq!(AttestationError::NotFound(att_id(1)).code(), -32880);
        assert_eq!(AttestationError::Duplicate(att_id(1)).code(), -32881);
        assert_eq!(
            AttestationError::VerificationFailed {
                id: att_id(1),
                reason: "x".into(),
            }
            .code(),
            -32882
        );
        assert_eq!(
            AttestationError::UnsupportedType(AttestationType::ZkProof).code(),
            -32883
        );
        assert_eq!(AttestationError::CapacityExceeded(10).code(), -32885);
    }
}
