//! Thread-safe intent receipt store.

use std::{
    cmp::Reverse,
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use alloy_primitives::B256;
use monmouth_agent_types::{AgentId, IntentReceipt};
use parking_lot::RwLock;
use tracing::{debug, info};

use crate::IntentReceiptError;

/// Default maximum number of receipts the store will hold.
pub const DEFAULT_MAX_RECEIPTS: usize = 100_000;

/// Internal state protected by the lock.
#[derive(Debug)]
struct Inner {
    /// Primary index: transaction hash to receipt.
    by_tx_hash: HashMap<B256, IntentReceipt>,
    /// Secondary index: agent to list of transaction hashes.
    by_agent: HashMap<AgentId, Vec<B256>>,
    /// Block number index for range queries.
    by_block: BTreeMap<u64, Vec<B256>>,
}

/// Thread-safe store for intent receipts.
///
/// Follows the same concurrency pattern as `FilterRegistry` and
/// `CapabilityRegistry` -- uses `parking_lot::RwLock` for synchronous,
/// non-async critical sections.
#[derive(Debug, Clone)]
pub struct IntentReceiptStore {
    inner: Arc<RwLock<Inner>>,
    max_receipts: usize,
}

impl Default for IntentReceiptStore {
    fn default() -> Self {
        Self {
            inner: Arc::new(RwLock::new(Inner {
                by_tx_hash: HashMap::new(),
                by_agent: HashMap::new(),
                by_block: BTreeMap::new(),
            })),
            max_receipts: DEFAULT_MAX_RECEIPTS,
        }
    }
}

impl IntentReceiptStore {
    /// Create a new, empty intent receipt store with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the maximum number of receipts the store will hold.
    #[must_use]
    pub const fn with_max_receipts(mut self, max: usize) -> Self {
        self.max_receipts = max;
        self
    }

    /// Record a new intent receipt.
    ///
    /// Inserts the receipt into all three indexes (by tx hash, by agent,
    /// and by block number).
    ///
    /// # Errors
    ///
    /// Returns an error if the store is at capacity or if a receipt with the
    /// same transaction hash already exists.
    pub fn record(&self, receipt: IntentReceipt) -> Result<(), IntentReceiptError> {
        let mut inner = self.inner.write();

        if inner.by_tx_hash.len() >= self.max_receipts {
            return Err(IntentReceiptError::CapacityExceeded(self.max_receipts));
        }

        if inner.by_tx_hash.contains_key(&receipt.tx_hash) {
            return Err(IntentReceiptError::DuplicateReceipt(receipt.tx_hash));
        }

        let tx_hash = receipt.tx_hash;
        let agent = receipt.agent;
        let block_number = receipt.block_number;

        info!(
            tx_hash = %tx_hash,
            agent = %agent,
            block = block_number,
            match_score = receipt.match_score,
            "Recorded intent receipt"
        );

        inner.by_tx_hash.insert(tx_hash, receipt);
        inner.by_agent.entry(agent).or_default().push(tx_hash);
        inner.by_block.entry(block_number).or_default().push(tx_hash);

        Ok(())
    }

    /// Look up a receipt by transaction hash.
    #[must_use]
    pub fn get(&self, tx_hash: B256) -> Option<IntentReceipt> {
        self.inner.read().by_tx_hash.get(&tx_hash).cloned()
    }

    /// List receipts for a given agent, most recent first.
    ///
    /// Returns up to `limit` receipts, ordered by descending block number.
    #[must_use]
    pub fn list_for_agent(&self, agent: AgentId, limit: usize) -> Vec<IntentReceipt> {
        let inner = self.inner.read();

        let Some(tx_hashes) = inner.by_agent.get(&agent) else {
            return Vec::new();
        };

        debug!(agent = %agent, total = tx_hashes.len(), limit, "Listing receipts for agent");

        let mut receipts: Vec<IntentReceipt> =
            tx_hashes.iter().filter_map(|hash| inner.by_tx_hash.get(hash).cloned()).collect();

        // Sort by block number descending (most recent first).
        receipts.sort_by_key(|r| Reverse(r.block_number));
        receipts.truncate(limit);
        receipts
    }

    /// List receipts within a block range (inclusive on both ends).
    ///
    /// Returns all receipts in blocks from `from_block` to `to_block`,
    /// ordered by ascending block number.
    #[must_use]
    pub fn list_in_range(&self, from_block: u64, to_block: u64) -> Vec<IntentReceipt> {
        let inner = self.inner.read();

        debug!(from = from_block, to = to_block, "Listing receipts in block range");

        inner
            .by_block
            .range(from_block..=to_block)
            .flat_map(|(_, hashes)| {
                hashes.iter().filter_map(|hash| inner.by_tx_hash.get(hash).cloned())
            })
            .collect()
    }

    /// Returns the total number of receipts in the store.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.read().by_tx_hash.len()
    }

    /// Returns `true` if the store contains no receipts.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.read().by_tx_hash.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use alloy_primitives::Address;
    use monmouth_agent_types::{ActualOutcome, IntentDeclaration};

    use super::*;

    fn agent_a() -> AgentId {
        AgentId(Address::repeat_byte(0xAA))
    }

    fn agent_b() -> AgentId {
        AgentId(Address::repeat_byte(0xBB))
    }

    fn tx_hash(byte: u8) -> B256 {
        B256::repeat_byte(byte)
    }

    fn test_receipt(hash: B256, agent: AgentId, block: u64) -> IntentReceipt {
        IntentReceipt {
            tx_hash: hash,
            agent,
            declared_intent: IntentDeclaration {
                description: "Transfer tokens".to_string(),
                intent_type: "transfer".to_string(),
                expected_outcome: "100 tokens moved".to_string(),
            },
            actual_outcome: ActualOutcome {
                success: true,
                gas_used: 21_000,
                summary: "100 tokens moved".to_string(),
            },
            match_score: 1.0,
            timestamp: 1_700_000_000,
            block_number: block,
        }
    }

    #[test]
    fn record_and_get_roundtrip() {
        let store = IntentReceiptStore::new();
        let receipt = test_receipt(tx_hash(1), agent_a(), 100);
        store.record(receipt.clone()).unwrap();

        let retrieved = store.get(tx_hash(1)).unwrap();
        assert_eq!(retrieved.tx_hash, receipt.tx_hash);
        assert_eq!(retrieved.agent, receipt.agent);
        assert_eq!(retrieved.block_number, 100);
    }

    #[test]
    fn get_nonexistent_returns_none() {
        let store = IntentReceiptStore::new();
        assert!(store.get(tx_hash(99)).is_none());
    }

    #[test]
    fn duplicate_rejection() {
        let store = IntentReceiptStore::new();
        store.record(test_receipt(tx_hash(1), agent_a(), 100)).unwrap();

        let err = store.record(test_receipt(tx_hash(1), agent_a(), 101)).unwrap_err();
        assert!(matches!(err, IntentReceiptError::DuplicateReceipt(_)));
    }

    #[test]
    fn agent_listing_most_recent_first() {
        let store = IntentReceiptStore::new();
        store.record(test_receipt(tx_hash(1), agent_a(), 100)).unwrap();
        store.record(test_receipt(tx_hash(2), agent_a(), 200)).unwrap();
        store.record(test_receipt(tx_hash(3), agent_a(), 150)).unwrap();

        let list = store.list_for_agent(agent_a(), 10);
        assert_eq!(list.len(), 3);
        // Most recent first: 200, 150, 100.
        assert_eq!(list[0].block_number, 200);
        assert_eq!(list[1].block_number, 150);
        assert_eq!(list[2].block_number, 100);
    }

    #[test]
    fn agent_listing_respects_limit() {
        let store = IntentReceiptStore::new();
        store.record(test_receipt(tx_hash(1), agent_a(), 100)).unwrap();
        store.record(test_receipt(tx_hash(2), agent_a(), 200)).unwrap();
        store.record(test_receipt(tx_hash(3), agent_a(), 300)).unwrap();

        let list = store.list_for_agent(agent_a(), 2);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].block_number, 300);
        assert_eq!(list[1].block_number, 200);
    }

    #[test]
    fn agent_listing_empty_for_unknown_agent() {
        let store = IntentReceiptStore::new();
        store.record(test_receipt(tx_hash(1), agent_a(), 100)).unwrap();

        let list = store.list_for_agent(agent_b(), 10);
        assert!(list.is_empty());
    }

    #[test]
    fn block_range_queries() {
        let store = IntentReceiptStore::new();
        store.record(test_receipt(tx_hash(1), agent_a(), 100)).unwrap();
        store.record(test_receipt(tx_hash(2), agent_a(), 200)).unwrap();
        store.record(test_receipt(tx_hash(3), agent_b(), 300)).unwrap();
        store.record(test_receipt(tx_hash(4), agent_a(), 400)).unwrap();

        // Range 150..=300 should include blocks 200 and 300.
        let list = store.list_in_range(150, 300);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].block_number, 200);
        assert_eq!(list[1].block_number, 300);
    }

    #[test]
    fn block_range_inclusive_boundaries() {
        let store = IntentReceiptStore::new();
        store.record(test_receipt(tx_hash(1), agent_a(), 100)).unwrap();
        store.record(test_receipt(tx_hash(2), agent_a(), 200)).unwrap();

        // Exact boundaries should be included.
        let list = store.list_in_range(100, 200);
        assert_eq!(list.len(), 2);

        // Single block range.
        let list = store.list_in_range(100, 100);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].block_number, 100);
    }

    #[test]
    fn block_range_empty_result() {
        let store = IntentReceiptStore::new();
        store.record(test_receipt(tx_hash(1), agent_a(), 100)).unwrap();

        let list = store.list_in_range(200, 300);
        assert!(list.is_empty());
    }

    #[test]
    fn capacity_exceeded() {
        let store = IntentReceiptStore::new().with_max_receipts(2);
        store.record(test_receipt(tx_hash(1), agent_a(), 100)).unwrap();
        store.record(test_receipt(tx_hash(2), agent_a(), 200)).unwrap();

        let err = store.record(test_receipt(tx_hash(3), agent_a(), 300)).unwrap_err();
        assert!(matches!(err, IntentReceiptError::CapacityExceeded(2)));
    }

    #[test]
    fn len_and_is_empty() {
        let store = IntentReceiptStore::new();
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);

        store.record(test_receipt(tx_hash(1), agent_a(), 100)).unwrap();
        assert!(!store.is_empty());
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn multiple_agents_independent() {
        let store = IntentReceiptStore::new();
        store.record(test_receipt(tx_hash(1), agent_a(), 100)).unwrap();
        store.record(test_receipt(tx_hash(2), agent_b(), 200)).unwrap();
        store.record(test_receipt(tx_hash(3), agent_a(), 300)).unwrap();

        let list_a = store.list_for_agent(agent_a(), 10);
        assert_eq!(list_a.len(), 2);

        let list_b = store.list_for_agent(agent_b(), 10);
        assert_eq!(list_b.len(), 1);
    }

    #[test]
    fn multiple_receipts_same_block() {
        let store = IntentReceiptStore::new();
        store.record(test_receipt(tx_hash(1), agent_a(), 100)).unwrap();
        store.record(test_receipt(tx_hash(2), agent_b(), 100)).unwrap();

        let list = store.list_in_range(100, 100);
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn thread_safety() {
        let store = Arc::new(IntentReceiptStore::new());
        let mut handles = vec![];

        for i in 0..10u8 {
            let s = Arc::clone(&store);
            handles.push(std::thread::spawn(move || {
                let receipt = test_receipt(tx_hash(i), agent_a(), u64::from(i) * 100);
                s.record(receipt).unwrap();
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(store.len(), 10);
    }

    #[test]
    fn error_codes() {
        assert_eq!(IntentReceiptError::NotFound(tx_hash(1)).code(), -32800);
        assert_eq!(IntentReceiptError::CapacityExceeded(10).code(), -32801);
        assert_eq!(IntentReceiptError::DuplicateReceipt(tx_hash(1)).code(), -32802);
    }
}
