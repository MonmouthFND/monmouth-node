//! Thread-safe memory anchor registry.

use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use alloy_primitives::B256;
use monmouth_agent_types::{AgentId, MemoryAnchor, MemoryAnchorKey};
use parking_lot::RwLock;
use tracing::{debug, info};

use crate::MemoryAnchorError;

/// Default maximum number of anchors per agent.
pub const DEFAULT_MAX_ANCHORS_PER_AGENT: usize = 10_000;

/// Internal state protected by the lock.
#[derive(Debug)]
struct Inner {
    /// Anchors ordered by (agent, sequence) for efficient range queries.
    anchors: BTreeMap<MemoryAnchorKey, MemoryAnchor>,
    /// Tracks the latest sequence number per agent.
    latest_sequence: HashMap<AgentId, u64>,
}

/// Thread-safe registry for on-chain memory anchors.
///
/// Follows the same concurrency pattern as `FilterRegistry` and
/// `CapabilityRegistry` -- uses `parking_lot::RwLock` for synchronous,
/// non-async critical sections.
#[derive(Debug, Clone)]
pub struct MemoryAnchorRegistry {
    inner: Arc<RwLock<Inner>>,
    max_anchors_per_agent: usize,
}

impl Default for MemoryAnchorRegistry {
    fn default() -> Self {
        Self {
            inner: Arc::new(RwLock::new(Inner {
                anchors: BTreeMap::new(),
                latest_sequence: HashMap::new(),
            })),
            max_anchors_per_agent: DEFAULT_MAX_ANCHORS_PER_AGENT,
        }
    }
}

impl MemoryAnchorRegistry {
    /// Create a new, empty memory anchor registry with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the maximum number of anchors per agent.
    #[must_use]
    pub const fn with_max_anchors_per_agent(mut self, max: usize) -> Self {
        self.max_anchors_per_agent = max;
        self
    }

    /// Create a new memory anchor for the given agent.
    ///
    /// The sequence number is auto-incremented per agent. Returns the
    /// newly created anchor.
    ///
    /// # Errors
    ///
    /// Returns an error if the agent has reached the maximum number of
    /// anchors.
    pub fn anchor(
        &self,
        agent: AgentId,
        content_hash: B256,
        label: String,
        timestamp: u64,
    ) -> Result<MemoryAnchor, MemoryAnchorError> {
        let mut inner = self.inner.write();

        // Check per-agent capacity.
        let current_count = inner
            .anchors
            .range(
                MemoryAnchorKey { agent, sequence: 0 }..=MemoryAnchorKey {
                    agent,
                    sequence: u64::MAX,
                },
            )
            .count();

        if current_count >= self.max_anchors_per_agent {
            return Err(MemoryAnchorError::CapacityExceeded {
                agent,
                max: self.max_anchors_per_agent,
            });
        }

        // Auto-increment sequence number.
        let sequence = inner.latest_sequence.entry(agent).or_insert(0);
        *sequence += 1;
        let seq = *sequence;

        let anchor =
            MemoryAnchor { agent, sequence: seq, content_hash, label: label.clone(), timestamp };

        let key = MemoryAnchorKey { agent, sequence: seq };

        info!(
            agent = %agent,
            sequence = seq,
            content_hash = %content_hash,
            label = %label,
            "Anchored memory"
        );

        inner.anchors.insert(key, anchor.clone());

        Ok(anchor)
    }

    /// Look up an anchor by agent and sequence number.
    #[must_use]
    pub fn get(&self, agent: AgentId, sequence: u64) -> Option<MemoryAnchor> {
        let key = MemoryAnchorKey { agent, sequence };
        self.inner.read().anchors.get(&key).cloned()
    }

    /// Get the most recent anchor for an agent.
    #[must_use]
    pub fn latest(&self, agent: AgentId) -> Option<MemoryAnchor> {
        let inner = self.inner.read();
        let seq = inner.latest_sequence.get(&agent)?;
        let key = MemoryAnchorKey { agent, sequence: *seq };
        inner.anchors.get(&key).cloned()
    }

    /// List anchors for a given agent, most recent first.
    ///
    /// Returns up to `limit` anchors, ordered by descending sequence number.
    #[must_use]
    pub fn list_for_agent(&self, agent: AgentId, limit: usize) -> Vec<MemoryAnchor> {
        let inner = self.inner.read();

        debug!(agent = %agent, limit, "Listing anchors for agent");

        let anchors: Vec<MemoryAnchor> = inner
            .anchors
            .range(
                MemoryAnchorKey { agent, sequence: 0 }..=MemoryAnchorKey {
                    agent,
                    sequence: u64::MAX,
                },
            )
            .rev()
            .take(limit)
            .map(|(_, anchor)| anchor.clone())
            .collect();

        anchors
    }

    /// Verify that a stored anchor matches the provided content hash.
    ///
    /// Returns `Ok(true)` if the hashes match, `Ok(false)` if they do not.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryAnchorError::NotFound`] if no anchor exists for the
    /// given agent and sequence number.
    pub fn verify(
        &self,
        agent: AgentId,
        sequence: u64,
        content_hash: B256,
    ) -> Result<bool, MemoryAnchorError> {
        let key = MemoryAnchorKey { agent, sequence };
        let inner = self.inner.read();

        let anchor =
            inner.anchors.get(&key).ok_or(MemoryAnchorError::NotFound { agent, sequence })?;

        debug!(
            agent = %agent,
            sequence,
            stored = %anchor.content_hash,
            provided = %content_hash,
            "Verifying memory anchor"
        );

        Ok(anchor.content_hash == content_hash)
    }

    /// Returns the total number of anchors across all agents.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.read().anchors.len()
    }

    /// Returns `true` if the registry contains no anchors.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.read().anchors.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use alloy_primitives::Address;

    use super::*;

    fn agent_a() -> AgentId {
        AgentId(Address::repeat_byte(0xAA))
    }

    fn agent_b() -> AgentId {
        AgentId(Address::repeat_byte(0xBB))
    }

    fn hash(byte: u8) -> B256 {
        B256::repeat_byte(byte)
    }

    #[test]
    fn anchor_creation_with_auto_sequence() {
        let registry = MemoryAnchorRegistry::new();

        let a1 = registry.anchor(agent_a(), hash(1), "first".to_string(), 1000).unwrap();
        assert_eq!(a1.sequence, 1);
        assert_eq!(a1.agent, agent_a());
        assert_eq!(a1.content_hash, hash(1));
        assert_eq!(a1.label, "first");
        assert_eq!(a1.timestamp, 1000);

        let a2 = registry.anchor(agent_a(), hash(2), "second".to_string(), 2000).unwrap();
        assert_eq!(a2.sequence, 2);

        let a3 = registry.anchor(agent_a(), hash(3), "third".to_string(), 3000).unwrap();
        assert_eq!(a3.sequence, 3);
    }

    #[test]
    fn get_by_key() {
        let registry = MemoryAnchorRegistry::new();
        registry.anchor(agent_a(), hash(1), "test".to_string(), 1000).unwrap();

        let retrieved = registry.get(agent_a(), 1).unwrap();
        assert_eq!(retrieved.content_hash, hash(1));
        assert_eq!(retrieved.label, "test");
    }

    #[test]
    fn get_nonexistent_returns_none() {
        let registry = MemoryAnchorRegistry::new();
        assert!(registry.get(agent_a(), 1).is_none());
    }

    #[test]
    fn latest_for_agent() {
        let registry = MemoryAnchorRegistry::new();
        registry.anchor(agent_a(), hash(1), "first".to_string(), 1000).unwrap();
        registry.anchor(agent_a(), hash(2), "second".to_string(), 2000).unwrap();
        registry.anchor(agent_a(), hash(3), "third".to_string(), 3000).unwrap();

        let latest = registry.latest(agent_a()).unwrap();
        assert_eq!(latest.sequence, 3);
        assert_eq!(latest.content_hash, hash(3));
    }

    #[test]
    fn latest_for_unknown_agent() {
        let registry = MemoryAnchorRegistry::new();
        assert!(registry.latest(agent_a()).is_none());
    }

    #[test]
    fn list_ordering_most_recent_first() {
        let registry = MemoryAnchorRegistry::new();
        registry.anchor(agent_a(), hash(1), "first".to_string(), 1000).unwrap();
        registry.anchor(agent_a(), hash(2), "second".to_string(), 2000).unwrap();
        registry.anchor(agent_a(), hash(3), "third".to_string(), 3000).unwrap();

        let list = registry.list_for_agent(agent_a(), 10);
        assert_eq!(list.len(), 3);
        assert_eq!(list[0].sequence, 3);
        assert_eq!(list[1].sequence, 2);
        assert_eq!(list[2].sequence, 1);
    }

    #[test]
    fn list_respects_limit() {
        let registry = MemoryAnchorRegistry::new();
        registry.anchor(agent_a(), hash(1), "first".to_string(), 1000).unwrap();
        registry.anchor(agent_a(), hash(2), "second".to_string(), 2000).unwrap();
        registry.anchor(agent_a(), hash(3), "third".to_string(), 3000).unwrap();

        let list = registry.list_for_agent(agent_a(), 2);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].sequence, 3);
        assert_eq!(list[1].sequence, 2);
    }

    #[test]
    fn list_empty_for_unknown_agent() {
        let registry = MemoryAnchorRegistry::new();
        assert!(registry.list_for_agent(agent_a(), 10).is_empty());
    }

    #[test]
    fn verify_matching_hash() {
        let registry = MemoryAnchorRegistry::new();
        registry.anchor(agent_a(), hash(1), "test".to_string(), 1000).unwrap();

        let result = registry.verify(agent_a(), 1, hash(1)).unwrap();
        assert!(result);
    }

    #[test]
    fn verify_mismatched_hash() {
        let registry = MemoryAnchorRegistry::new();
        registry.anchor(agent_a(), hash(1), "test".to_string(), 1000).unwrap();

        let result = registry.verify(agent_a(), 1, hash(99)).unwrap();
        assert!(!result);
    }

    #[test]
    fn verify_not_found() {
        let registry = MemoryAnchorRegistry::new();

        let err = registry.verify(agent_a(), 1, hash(1)).unwrap_err();
        assert!(matches!(err, MemoryAnchorError::NotFound { .. }));
    }

    #[test]
    fn capacity_per_agent() {
        let registry = MemoryAnchorRegistry::new().with_max_anchors_per_agent(2);

        registry.anchor(agent_a(), hash(1), "first".to_string(), 1000).unwrap();
        registry.anchor(agent_a(), hash(2), "second".to_string(), 2000).unwrap();

        let err = registry.anchor(agent_a(), hash(3), "third".to_string(), 3000).unwrap_err();
        assert!(matches!(err, MemoryAnchorError::CapacityExceeded { max: 2, .. }));

        // Other agent should still be able to add anchors.
        registry.anchor(agent_b(), hash(4), "b-first".to_string(), 4000).unwrap();
        assert_eq!(registry.len(), 3);
    }

    #[test]
    fn multiple_agents_independent_sequences() {
        let registry = MemoryAnchorRegistry::new();

        let a1 = registry.anchor(agent_a(), hash(1), "a-1".to_string(), 1000).unwrap();
        let b1 = registry.anchor(agent_b(), hash(2), "b-1".to_string(), 1000).unwrap();
        let a2 = registry.anchor(agent_a(), hash(3), "a-2".to_string(), 2000).unwrap();

        // Each agent has independent sequences.
        assert_eq!(a1.sequence, 1);
        assert_eq!(b1.sequence, 1);
        assert_eq!(a2.sequence, 2);

        // Listing for each agent returns only their anchors.
        let list_a = registry.list_for_agent(agent_a(), 10);
        assert_eq!(list_a.len(), 2);

        let list_b = registry.list_for_agent(agent_b(), 10);
        assert_eq!(list_b.len(), 1);
    }

    #[test]
    fn len_and_is_empty() {
        let registry = MemoryAnchorRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);

        registry.anchor(agent_a(), hash(1), "test".to_string(), 1000).unwrap();
        assert!(!registry.is_empty());
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn thread_safety() {
        let registry = Arc::new(MemoryAnchorRegistry::new());
        let mut handles = vec![];

        for i in 0..10u8 {
            let reg = Arc::clone(&registry);
            handles.push(std::thread::spawn(move || {
                reg.anchor(
                    AgentId(Address::repeat_byte(i)),
                    hash(i),
                    format!("thread-{i}"),
                    u64::from(i) * 1000,
                )
                .unwrap();
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(registry.len(), 10);
    }

    #[test]
    fn error_codes() {
        assert_eq!(MemoryAnchorError::NotFound { agent: agent_a(), sequence: 1 }.code(), -32900);
        assert_eq!(
            MemoryAnchorError::CapacityExceeded { agent: agent_a(), max: 10 }.code(),
            -32901
        );
        assert_eq!(
            MemoryAnchorError::VerificationFailed {
                agent: agent_a(),
                sequence: 1,
                expected: hash(1),
                actual: hash(2),
            }
            .code(),
            -32902
        );
    }
}
