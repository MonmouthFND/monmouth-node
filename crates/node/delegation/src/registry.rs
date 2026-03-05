//! Thread-safe delegation session registry.

use std::{collections::HashMap, sync::Arc};

use alloy_primitives::{Address, U256};
use monmouth_agent_types::{SessionGrant, SessionId};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use crate::{crypto::verify_session_grant, error::DelegationError};

/// Default maximum number of active sessions.
pub const DEFAULT_MAX_SESSIONS: usize = 10_000;

/// Internal representation of an active delegation session.
#[derive(Debug, Clone)]
pub(crate) struct ActiveSession {
    /// The session grant that created this session.
    pub(crate) grant: SessionGrant,
    /// Unique session identifier.
    pub(crate) id: SessionId,
    /// Total amount of wei spent through this session so far.
    pub(crate) total_spent_wei: U256,
    /// Unix timestamp when this session was created.
    pub(crate) created_at: u64,
}

/// Public view of a delegation session returned from queries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    /// Unique session identifier.
    pub id: SessionId,
    /// The address that owns the session.
    pub owner: Address,
    /// The address that has been delegated permissions.
    pub delegate: Address,
    /// Capability IDs the delegate may invoke.
    pub capabilities: Vec<String>,
    /// Maximum total spend in wei.
    pub spending_limit_wei: U256,
    /// Total amount spent through this session so far.
    pub total_spent_wei: U256,
    /// Unix timestamp when the session expires.
    pub expires_at: u64,
    /// Unix timestamp when the session was created.
    pub created_at: u64,
}

impl From<&ActiveSession> for SessionInfo {
    fn from(session: &ActiveSession) -> Self {
        Self {
            id: session.id,
            owner: session.grant.owner,
            delegate: session.grant.delegate,
            capabilities: session.grant.capabilities.clone(),
            spending_limit_wei: session.grant.spending_limit_wei,
            total_spent_wei: session.total_spent_wei,
            expires_at: session.grant.expires_at,
            created_at: session.created_at,
        }
    }
}

/// Thread-safe registry of delegation sessions.
///
/// Uses `parking_lot::RwLock` for synchronous, non-async critical sections
/// (same pattern as [`FilterRegistry`](../../rpc) and [`CapabilityRegistry`]).
#[derive(Debug, Clone)]
pub struct DelegationRegistry {
    inner: Arc<RwLock<HashMap<SessionId, ActiveSession>>>,
    max_sessions: usize,
}

impl Default for DelegationRegistry {
    fn default() -> Self {
        Self { inner: Arc::new(RwLock::new(HashMap::new())), max_sessions: DEFAULT_MAX_SESSIONS }
    }
}

impl DelegationRegistry {
    /// Set the maximum number of concurrent sessions.
    #[must_use]
    pub const fn with_max_sessions(mut self, max: usize) -> Self {
        self.max_sessions = max;
        self
    }

    /// Create a new delegation session from a signed grant.
    ///
    /// The signature is verified against the grant, and the recovered address
    /// must match the grant's `owner` field. The session ID is derived from
    /// the grant's nonce and owner to ensure determinism.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The signature is invalid or does not match the grant's owner
    /// - The registry is at capacity
    /// - A session with the same ID already exists
    pub fn create_session(
        &self,
        grant: SessionGrant,
        signature: &[u8],
        current_time: u64,
    ) -> Result<SessionId, DelegationError> {
        // Verify signature and recover signer.
        let signer = verify_session_grant(&grant, signature)?;

        if signer != grant.owner {
            return Err(DelegationError::InvalidSignature(format!(
                "signer {signer} does not match grant owner {}",
                grant.owner
            )));
        }

        // Derive a deterministic session ID from owner + nonce.
        let session_id = derive_session_id(&grant);

        let mut inner = self.inner.write();

        if inner.len() >= self.max_sessions {
            return Err(DelegationError::CapacityExceeded(self.max_sessions));
        }

        if inner.contains_key(&session_id) {
            return Err(DelegationError::DuplicateSession(session_id));
        }

        info!(
            session_id = %session_id,
            owner = %grant.owner,
            delegate = %grant.delegate,
            capabilities = ?grant.capabilities,
            expires_at = grant.expires_at,
            "Created delegation session"
        );

        inner.insert(
            session_id,
            ActiveSession {
                grant,
                id: session_id,
                total_spent_wei: U256::ZERO,
                created_at: current_time,
            },
        );

        Ok(session_id)
    }

    /// Validate that a session permits a given operation.
    ///
    /// Checks that the session exists, has not expired, the delegate matches,
    /// the requested capability is granted, and the spending limit is not
    /// exceeded.
    ///
    /// # Errors
    ///
    /// Returns an error if any validation check fails.
    pub fn validate_session(
        &self,
        session_id: SessionId,
        delegate: Address,
        capability_id: &str,
        value_wei: U256,
        current_time: u64,
    ) -> Result<(), DelegationError> {
        let inner = self.inner.read();

        let session = inner.get(&session_id).ok_or(DelegationError::SessionNotFound(session_id))?;

        // Check expiry.
        if current_time >= session.grant.expires_at {
            return Err(DelegationError::SessionExpired(session_id));
        }

        // Check delegate.
        if delegate != session.grant.delegate {
            return Err(DelegationError::Unauthorized {
                session_id,
                expected_owner: session.grant.delegate,
                actual: delegate,
            });
        }

        // Check capability.
        if !session.grant.capabilities.iter().any(|c| c == capability_id) {
            return Err(DelegationError::CapabilityNotGranted {
                session_id,
                capability_id: capability_id.to_string(),
            });
        }

        // Check spending limit.
        let projected_spend = session.total_spent_wei.checked_add(value_wei).unwrap_or(U256::MAX);
        if projected_spend > session.grant.spending_limit_wei {
            return Err(DelegationError::SpendingLimitExceeded {
                session_id,
                limit: session.grant.spending_limit_wei,
                attempted: projected_spend,
            });
        }

        debug!(
            session_id = %session_id,
            delegate = %delegate,
            capability_id = %capability_id,
            value_wei = %value_wei,
            "Session validation passed"
        );

        Ok(())
    }

    /// Record spending against a session, updating the cumulative total.
    ///
    /// # Errors
    ///
    /// Returns an error if the session does not exist or the spending limit
    /// would be exceeded.
    pub fn record_spend(&self, session_id: SessionId, amount: U256) -> Result<(), DelegationError> {
        let mut inner = self.inner.write();

        let session =
            inner.get_mut(&session_id).ok_or(DelegationError::SessionNotFound(session_id))?;

        let new_total = session.total_spent_wei.checked_add(amount).unwrap_or(U256::MAX);
        if new_total > session.grant.spending_limit_wei {
            return Err(DelegationError::SpendingLimitExceeded {
                session_id,
                limit: session.grant.spending_limit_wei,
                attempted: new_total,
            });
        }

        session.total_spent_wei = new_total;

        debug!(
            session_id = %session_id,
            amount = %amount,
            total_spent = %session.total_spent_wei,
            "Recorded session spend"
        );

        Ok(())
    }

    /// Revoke a delegation session. Only the session owner may revoke.
    ///
    /// # Errors
    ///
    /// Returns an error if the session does not exist or the caller is not
    /// the session owner.
    pub fn revoke_session(
        &self,
        session_id: SessionId,
        owner: Address,
    ) -> Result<(), DelegationError> {
        let mut inner = self.inner.write();

        let session = inner.get(&session_id).ok_or(DelegationError::SessionNotFound(session_id))?;

        if session.grant.owner != owner {
            return Err(DelegationError::Unauthorized {
                session_id,
                expected_owner: session.grant.owner,
                actual: owner,
            });
        }

        inner.remove(&session_id);

        info!(session_id = %session_id, owner = %owner, "Revoked delegation session");

        Ok(())
    }

    /// Get public information about a session.
    #[must_use]
    pub fn get_session(&self, session_id: SessionId) -> Option<SessionInfo> {
        self.inner.read().get(&session_id).map(SessionInfo::from)
    }

    /// List all sessions owned by a given address.
    #[must_use]
    pub fn sessions_for_owner(&self, owner: Address) -> Vec<SessionInfo> {
        self.inner
            .read()
            .values()
            .filter(|s| s.grant.owner == owner)
            .map(SessionInfo::from)
            .collect()
    }

    /// List all sessions where the given address is the delegate.
    #[must_use]
    pub fn sessions_for_delegate(&self, delegate: Address) -> Vec<SessionInfo> {
        self.inner
            .read()
            .values()
            .filter(|s| s.grant.delegate == delegate)
            .map(SessionInfo::from)
            .collect()
    }

    /// Returns the number of active sessions.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.read().len()
    }

    /// Returns `true` if the registry contains no sessions.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.read().is_empty()
    }
}

/// Derive a deterministic session ID from a grant's owner address and nonce.
fn derive_session_id(grant: &SessionGrant) -> SessionId {
    use sha3::{Digest, Keccak256};

    let mut hasher = Keccak256::new();
    hasher.update(b"monmouth.session.v1");
    hasher.update(grant.owner.as_slice());
    hasher.update(grant.nonce.to_be_bytes());
    let hash: [u8; 32] = hasher.finalize().into();
    SessionId(alloy_primitives::B256::from(hash))
}

#[cfg(test)]
mod tests {
    use alloy_primitives::{Address, U256};
    use k256::ecdsa::SigningKey;
    use sha3::{Digest, Keccak256};

    use super::*;
    use crate::crypto::sign_session_grant;

    /// Derive the Ethereum address from a signing key.
    fn address_from_key(key: &SigningKey) -> Address {
        let vk = key.verifying_key();
        let pk_bytes = vk.to_encoded_point(false);
        let hash = Keccak256::digest(&pk_bytes.as_bytes()[1..]);
        Address::from_slice(&hash[12..])
    }

    fn owner_key() -> SigningKey {
        let secret = [0x11u8; 32];
        SigningKey::from_bytes((&secret).into()).unwrap()
    }

    fn delegate_key() -> SigningKey {
        let secret = [0x22u8; 32];
        SigningKey::from_bytes((&secret).into()).unwrap()
    }

    fn test_grant(owner_key: &SigningKey, delegate_addr: Address) -> SessionGrant {
        SessionGrant {
            owner: address_from_key(owner_key),
            delegate: delegate_addr,
            capabilities: vec!["sim.preview".to_string(), "state.read".to_string()],
            spending_limit_wei: U256::from(1_000_000u64),
            expires_at: 2_000_000_000, // Far future.
            nonce: 1,
        }
    }

    fn setup() -> (DelegationRegistry, SigningKey, Address, SessionGrant) {
        let registry = DelegationRegistry::default();
        let ok = owner_key();
        let dk = delegate_key();
        let delegate_addr = address_from_key(&dk);
        let grant = test_grant(&ok, delegate_addr);
        (registry, ok, delegate_addr, grant)
    }

    #[test]
    fn create_and_get_session() {
        let (registry, ok, delegate_addr, grant) = setup();
        let sig = sign_session_grant(&grant, &ok).unwrap();

        let session_id = registry.create_session(grant.clone(), &sig, 1_000_000).unwrap();
        let info = registry.get_session(session_id).unwrap();

        assert_eq!(info.owner, grant.owner);
        assert_eq!(info.delegate, delegate_addr);
        assert_eq!(info.capabilities, grant.capabilities);
        assert_eq!(info.spending_limit_wei, grant.spending_limit_wei);
        assert_eq!(info.total_spent_wei, U256::ZERO);
        assert_eq!(info.created_at, 1_000_000);
    }

    #[test]
    fn validate_session_success() {
        let (registry, ok, delegate_addr, grant) = setup();
        let sig = sign_session_grant(&grant, &ok).unwrap();
        let session_id = registry.create_session(grant, &sig, 1_000_000).unwrap();

        registry
            .validate_session(
                session_id,
                delegate_addr,
                "sim.preview",
                U256::from(100u64),
                1_500_000_000,
            )
            .unwrap();
    }

    #[test]
    fn validate_expired_session() {
        let (registry, ok, delegate_addr, grant) = setup();
        let sig = sign_session_grant(&grant, &ok).unwrap();
        let session_id = registry.create_session(grant.clone(), &sig, 1_000_000).unwrap();

        let err = registry
            .validate_session(
                session_id,
                delegate_addr,
                "sim.preview",
                U256::ZERO,
                grant.expires_at + 1, // After expiry.
            )
            .unwrap_err();

        assert!(matches!(err, DelegationError::SessionExpired(_)));
    }

    #[test]
    fn validate_wrong_delegate() {
        let (registry, ok, _delegate_addr, grant) = setup();
        let sig = sign_session_grant(&grant, &ok).unwrap();
        let session_id = registry.create_session(grant, &sig, 1_000_000).unwrap();

        let wrong_addr = Address::repeat_byte(0xff);
        let err = registry
            .validate_session(session_id, wrong_addr, "sim.preview", U256::ZERO, 1_500_000_000)
            .unwrap_err();

        assert!(matches!(err, DelegationError::Unauthorized { .. }));
    }

    #[test]
    fn validate_capability_not_granted() {
        let (registry, ok, delegate_addr, grant) = setup();
        let sig = sign_session_grant(&grant, &ok).unwrap();
        let session_id = registry.create_session(grant, &sig, 1_000_000).unwrap();

        let err = registry
            .validate_session(session_id, delegate_addr, "admin.destroy", U256::ZERO, 1_500_000_000)
            .unwrap_err();

        assert!(matches!(err, DelegationError::CapabilityNotGranted { .. }));
    }

    #[test]
    fn validate_spending_limit_exceeded() {
        let (registry, ok, delegate_addr, grant) = setup();
        let sig = sign_session_grant(&grant, &ok).unwrap();
        let session_id = registry.create_session(grant.clone(), &sig, 1_000_000).unwrap();

        let err = registry
            .validate_session(
                session_id,
                delegate_addr,
                "sim.preview",
                grant.spending_limit_wei + U256::from(1u64),
                1_500_000_000,
            )
            .unwrap_err();

        assert!(matches!(err, DelegationError::SpendingLimitExceeded { .. }));
    }

    #[test]
    fn record_spend_and_check_cumulative() {
        let (registry, ok, _delegate_addr, grant) = setup();
        let sig = sign_session_grant(&grant, &ok).unwrap();
        let session_id = registry.create_session(grant.clone(), &sig, 1_000_000).unwrap();

        // Spend half the limit.
        let half = grant.spending_limit_wei / U256::from(2u64);
        registry.record_spend(session_id, half).unwrap();

        let info = registry.get_session(session_id).unwrap();
        assert_eq!(info.total_spent_wei, half);

        // Spending more than remaining should fail.
        let err = registry.record_spend(session_id, grant.spending_limit_wei).unwrap_err();
        assert!(matches!(err, DelegationError::SpendingLimitExceeded { .. }));

        // Spending the other half should succeed.
        registry.record_spend(session_id, half).unwrap();
        let info = registry.get_session(session_id).unwrap();
        assert_eq!(info.total_spent_wei, grant.spending_limit_wei);
    }

    #[test]
    fn revoke_session_by_owner() {
        let (registry, ok, _delegate_addr, grant) = setup();
        let sig = sign_session_grant(&grant, &ok).unwrap();
        let owner_addr = grant.owner;
        let session_id = registry.create_session(grant, &sig, 1_000_000).unwrap();

        registry.revoke_session(session_id, owner_addr).unwrap();
        assert!(registry.get_session(session_id).is_none());
    }

    #[test]
    fn revoke_session_unauthorized() {
        let (registry, ok, _delegate_addr, grant) = setup();
        let sig = sign_session_grant(&grant, &ok).unwrap();
        let session_id = registry.create_session(grant, &sig, 1_000_000).unwrap();

        let wrong_owner = Address::repeat_byte(0xee);
        let err = registry.revoke_session(session_id, wrong_owner).unwrap_err();
        assert!(matches!(err, DelegationError::Unauthorized { .. }));

        // Session should still exist.
        assert!(registry.get_session(session_id).is_some());
    }

    #[test]
    fn revoke_nonexistent_session() {
        let registry = DelegationRegistry::default();
        let fake_id = SessionId(alloy_primitives::B256::ZERO);
        let err = registry.revoke_session(fake_id, Address::ZERO).unwrap_err();
        assert!(matches!(err, DelegationError::SessionNotFound(_)));
    }

    #[test]
    fn session_not_found() {
        let registry = DelegationRegistry::default();
        let fake_id = SessionId(alloy_primitives::B256::ZERO);
        assert!(registry.get_session(fake_id).is_none());

        let err =
            registry.validate_session(fake_id, Address::ZERO, "test", U256::ZERO, 0).unwrap_err();
        assert!(matches!(err, DelegationError::SessionNotFound(_)));
    }

    #[test]
    fn duplicate_session() {
        let (registry, ok, _delegate_addr, grant) = setup();
        let sig = sign_session_grant(&grant, &ok).unwrap();

        registry.create_session(grant.clone(), &sig, 1_000_000).unwrap();

        // Same grant (same owner + nonce) => same session ID => duplicate.
        let err = registry.create_session(grant, &sig, 1_000_000).unwrap_err();
        assert!(matches!(err, DelegationError::DuplicateSession(_)));
    }

    #[test]
    fn capacity_exceeded() {
        let registry = DelegationRegistry::default().with_max_sessions(1);
        let ok = owner_key();
        let delegate_addr = address_from_key(&delegate_key());

        let grant1 = SessionGrant {
            owner: address_from_key(&ok),
            delegate: delegate_addr,
            capabilities: vec!["test".to_string()],
            spending_limit_wei: U256::from(1u64),
            expires_at: 2_000_000_000,
            nonce: 1,
        };
        let sig1 = sign_session_grant(&grant1, &ok).unwrap();
        registry.create_session(grant1, &sig1, 0).unwrap();

        let grant2 = SessionGrant {
            owner: address_from_key(&ok),
            delegate: delegate_addr,
            capabilities: vec!["test".to_string()],
            spending_limit_wei: U256::from(1u64),
            expires_at: 2_000_000_000,
            nonce: 2, // Different nonce => different session ID.
        };
        let sig2 = sign_session_grant(&grant2, &ok).unwrap();
        let err = registry.create_session(grant2, &sig2, 0).unwrap_err();
        assert!(matches!(err, DelegationError::CapacityExceeded(1)));
    }

    #[test]
    fn sessions_for_owner_and_delegate() {
        let registry = DelegationRegistry::default();
        let ok = owner_key();
        let dk = delegate_key();
        let owner_addr = address_from_key(&ok);
        let delegate_addr = address_from_key(&dk);

        // Create two sessions with different nonces.
        for nonce in 1..=2 {
            let grant = SessionGrant {
                owner: owner_addr,
                delegate: delegate_addr,
                capabilities: vec!["test".to_string()],
                spending_limit_wei: U256::from(100u64),
                expires_at: 2_000_000_000,
                nonce,
            };
            let sig = sign_session_grant(&grant, &ok).unwrap();
            registry.create_session(grant, &sig, 0).unwrap();
        }

        let by_owner = registry.sessions_for_owner(owner_addr);
        assert_eq!(by_owner.len(), 2);

        let by_delegate = registry.sessions_for_delegate(delegate_addr);
        assert_eq!(by_delegate.len(), 2);

        // Unknown address returns empty.
        let unknown = registry.sessions_for_owner(Address::repeat_byte(0xff));
        assert!(unknown.is_empty());
    }

    #[test]
    fn len_and_is_empty() {
        let (registry, ok, _delegate_addr, grant) = setup();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);

        let sig = sign_session_grant(&grant, &ok).unwrap();
        registry.create_session(grant, &sig, 0).unwrap();
        assert!(!registry.is_empty());
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn invalid_signature_on_create() {
        let (registry, _ok, _delegate_addr, grant) = setup();
        let bad_sig = vec![0u8; 65];

        let err = registry.create_session(grant, &bad_sig, 0).unwrap_err();
        assert!(matches!(err, DelegationError::InvalidSignature(_)));
    }

    #[test]
    fn wrong_signer_on_create() {
        let (registry, _ok, _delegate_addr, grant) = setup();

        // Sign with delegate key instead of owner key.
        let dk = delegate_key();
        let sig = sign_session_grant(&grant, &dk).unwrap();

        let err = registry.create_session(grant, &sig, 0).unwrap_err();
        assert!(matches!(err, DelegationError::InvalidSignature(_)));
    }

    #[test]
    fn session_info_serialization() {
        let info = SessionInfo {
            id: SessionId(alloy_primitives::B256::ZERO),
            owner: Address::ZERO,
            delegate: Address::repeat_byte(1),
            capabilities: vec!["test".to_string()],
            spending_limit_wei: U256::from(100u64),
            total_spent_wei: U256::from(50u64),
            expires_at: 2_000_000_000,
            created_at: 1_000_000,
        };

        let json = serde_json::to_string(&info).unwrap();
        let parsed: SessionInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(info, parsed);
    }

    #[test]
    fn thread_safety() {
        let registry = Arc::new(DelegationRegistry::default());
        let ok = owner_key();
        let delegate_addr = address_from_key(&delegate_key());
        let owner_addr = address_from_key(&ok);

        let mut handles = vec![];

        for i in 0..10u64 {
            let reg = Arc::clone(&registry);
            let ok_clone = ok.clone();
            handles.push(std::thread::spawn(move || {
                let grant = SessionGrant {
                    owner: owner_addr,
                    delegate: delegate_addr,
                    capabilities: vec!["test".to_string()],
                    spending_limit_wei: U256::from(100u64),
                    expires_at: 2_000_000_000,
                    nonce: i + 100,
                };
                let sig = sign_session_grant(&grant, &ok_clone).unwrap();
                reg.create_session(grant, &sig, 0).unwrap();
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(registry.len(), 10);
    }

    #[test]
    fn error_codes() {
        let sid = SessionId(alloy_primitives::B256::ZERO);
        assert_eq!(DelegationError::SessionNotFound(sid).code(), -32600);
        assert_eq!(DelegationError::SessionExpired(sid).code(), -32601);
        assert_eq!(
            DelegationError::Unauthorized {
                session_id: sid,
                expected_owner: Address::ZERO,
                actual: Address::ZERO,
            }
            .code(),
            -32602
        );
        assert_eq!(
            DelegationError::SpendingLimitExceeded {
                session_id: sid,
                limit: U256::ZERO,
                attempted: U256::ZERO,
            }
            .code(),
            -32603
        );
        assert_eq!(
            DelegationError::CapabilityNotGranted { session_id: sid, capability_id: String::new() }
                .code(),
            -32604
        );
        assert_eq!(DelegationError::CapacityExceeded(0).code(), -32605);
        assert_eq!(DelegationError::InvalidSignature(String::new()).code(), -32606);
        assert_eq!(DelegationError::DuplicateSession(sid).code(), -32607);
    }
}
