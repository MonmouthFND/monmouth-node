//! Thread-safe subscription registry with trigger evaluation.

use std::{collections::HashMap, sync::Arc};

use alloy_primitives::{Address, B256, U256};
use monmouth_agent_types::{AgentId, ConditionalSubscription, SubscriptionId, TriggerCondition};
use parking_lot::RwLock;
use tracing::{debug, info};

use crate::SubscriptionError;

/// Default maximum number of subscriptions the registry will hold.
pub const DEFAULT_MAX_SUBSCRIPTIONS: usize = 100_000;

/// Block-level context for trigger evaluation.
///
/// Provides the state snapshot that triggers are evaluated against.
#[derive(Debug, Clone)]
pub struct BlockContext {
    /// Current block number.
    pub block_number: u64,
    /// Current gas price in wei.
    pub gas_price: U256,
    /// Account balances to check against.
    pub balances: HashMap<Address, U256>,
    /// Storage slots that changed in this block: `(address, slot)`.
    pub storage_changes: Vec<(Address, B256)>,
    /// Events emitted in this block: `(address, topic0)`.
    pub events: Vec<(Address, B256)>,
}

/// Internal state protected by the lock.
#[derive(Debug)]
struct Inner {
    /// Primary index: subscription ID to subscription.
    subs: HashMap<SubscriptionId, ConditionalSubscription>,
    /// Secondary index: owner to subscription IDs.
    by_owner: HashMap<AgentId, Vec<SubscriptionId>>,
}

/// Thread-safe registry for conditional subscriptions.
///
/// Agents register subscriptions with trigger conditions. On each block,
/// [`evaluate`] is called with a [`BlockContext`] to find and fire all
/// matching subscriptions.
#[derive(Debug, Clone)]
pub struct SubscriptionRegistry {
    inner: Arc<RwLock<Inner>>,
    max_subscriptions: usize,
}

impl Default for SubscriptionRegistry {
    fn default() -> Self {
        Self {
            inner: Arc::new(RwLock::new(Inner { subs: HashMap::new(), by_owner: HashMap::new() })),
            max_subscriptions: DEFAULT_MAX_SUBSCRIPTIONS,
        }
    }
}

impl SubscriptionRegistry {
    /// Create a new, empty subscription registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the maximum number of subscriptions.
    #[must_use]
    pub const fn with_max_subscriptions(mut self, max: usize) -> Self {
        self.max_subscriptions = max;
        self
    }

    /// Register a new subscription.
    ///
    /// # Errors
    ///
    /// Returns an error if the registry is at capacity or the subscription
    /// ID is a duplicate.
    pub fn subscribe(&self, sub: ConditionalSubscription) -> Result<(), SubscriptionError> {
        let mut inner = self.inner.write();

        if inner.subs.len() >= self.max_subscriptions {
            return Err(SubscriptionError::CapacityExceeded(self.max_subscriptions));
        }

        if inner.subs.contains_key(&sub.id) {
            return Err(SubscriptionError::Duplicate(sub.id));
        }

        info!(
            sub_id = %sub.id,
            owner = %sub.owner,
            "Subscription registered"
        );

        let sub_id = sub.id;
        let owner = sub.owner;
        inner.by_owner.entry(owner).or_default().push(sub_id);
        inner.subs.insert(sub_id, sub);

        Ok(())
    }

    /// Unsubscribe (deactivate) a subscription.
    ///
    /// Only the owner can unsubscribe.
    ///
    /// # Errors
    ///
    /// Returns an error if the subscription is not found or the caller is not
    /// the owner.
    pub fn unsubscribe(
        &self,
        sub_id: SubscriptionId,
        caller: AgentId,
    ) -> Result<(), SubscriptionError> {
        let mut inner = self.inner.write();

        let sub = inner.subs.get_mut(&sub_id).ok_or(SubscriptionError::NotFound(sub_id))?;

        if sub.owner != caller {
            return Err(SubscriptionError::Unauthorized { subscription_id: sub_id, agent: caller });
        }

        debug!(sub_id = %sub_id, "Subscription deactivated");
        sub.active = false;

        Ok(())
    }

    /// Evaluate all active subscriptions against a block context.
    ///
    /// Returns the list of subscriptions that fired. Each fired subscription
    /// has its `trigger_count` incremented. Subscriptions that reach
    /// `max_triggers` are automatically deactivated.
    pub fn evaluate(&self, ctx: &BlockContext) -> Vec<ConditionalSubscription> {
        let mut inner = self.inner.write();
        let mut fired = Vec::new();

        for sub in inner.subs.values_mut() {
            if !sub.active {
                continue;
            }

            if Self::matches_condition(&sub.condition, ctx) {
                sub.trigger_count += 1;
                debug!(
                    sub_id = %sub.id,
                    trigger_count = sub.trigger_count,
                    max_triggers = sub.max_triggers,
                    "Subscription triggered"
                );

                fired.push(sub.clone());

                // Auto-deactivate if triggers exhausted.
                if sub.trigger_count >= sub.max_triggers {
                    info!(sub_id = %sub.id, "Subscription exhausted, deactivating");
                    sub.active = false;
                }
            }
        }

        fired
    }

    /// Check if a condition matches the given block context.
    fn matches_condition(condition: &TriggerCondition, ctx: &BlockContext) -> bool {
        match condition {
            TriggerCondition::BalanceBelow { address, threshold } => {
                ctx.balances.get(address).is_some_and(|balance| balance < threshold)
            }
            TriggerCondition::BalanceAbove { address, threshold } => {
                ctx.balances.get(address).is_some_and(|balance| balance > threshold)
            }
            TriggerCondition::StorageChanged { address, slot } => {
                ctx.storage_changes.iter().any(|(a, s)| a == address && s == slot)
            }
            TriggerCondition::EventEmitted { address, topic0 } => {
                ctx.events.iter().any(|(a, t)| a == address && t == topic0)
            }
            TriggerCondition::BlockNumber { block } => ctx.block_number >= *block,
            TriggerCondition::GasPriceBelow { threshold } => ctx.gas_price < *threshold,
        }
    }

    /// Look up a subscription by ID.
    #[must_use]
    pub fn get(&self, sub_id: SubscriptionId) -> Option<ConditionalSubscription> {
        self.inner.read().subs.get(&sub_id).cloned()
    }

    /// List subscriptions owned by an agent.
    #[must_use]
    pub fn list_for_owner(&self, owner: AgentId) -> Vec<ConditionalSubscription> {
        let inner = self.inner.read();
        inner
            .by_owner
            .get(&owner)
            .map(|ids| ids.iter().filter_map(|id| inner.subs.get(id).cloned()).collect())
            .unwrap_or_default()
    }

    /// List all active subscriptions.
    #[must_use]
    pub fn list_active(&self) -> Vec<ConditionalSubscription> {
        self.inner.read().subs.values().filter(|s| s.active).cloned().collect()
    }

    /// Returns the total number of subscriptions.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.read().subs.len()
    }

    /// Returns `true` if the registry contains no subscriptions.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.read().subs.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use alloy_primitives::Address;

    use super::*;

    fn owner_a() -> AgentId {
        AgentId(Address::repeat_byte(0xAA))
    }

    fn owner_b() -> AgentId {
        AgentId(Address::repeat_byte(0xBB))
    }

    fn sub_id(byte: u8) -> SubscriptionId {
        SubscriptionId(B256::repeat_byte(byte))
    }

    fn addr(byte: u8) -> Address {
        Address::repeat_byte(byte)
    }

    fn test_sub(
        id: SubscriptionId,
        owner: AgentId,
        condition: TriggerCondition,
        max_triggers: u64,
    ) -> ConditionalSubscription {
        ConditionalSubscription {
            id,
            owner,
            condition,
            webhook_url: None,
            max_triggers,
            trigger_count: 0,
            active: true,
            created_at: 1_700_000_000,
        }
    }

    fn empty_context(block: u64) -> BlockContext {
        BlockContext {
            block_number: block,
            gas_price: U256::from(20_000_000_000u64), // 20 gwei
            balances: HashMap::new(),
            storage_changes: Vec::new(),
            events: Vec::new(),
        }
    }

    #[test]
    fn subscribe_and_get() {
        let reg = SubscriptionRegistry::new();
        let sub = test_sub(sub_id(1), owner_a(), TriggerCondition::BlockNumber { block: 100 }, 5);
        reg.subscribe(sub.clone()).unwrap();

        let retrieved = reg.get(sub_id(1)).unwrap();
        assert_eq!(retrieved.id, sub.id);
        assert!(retrieved.active);
    }

    #[test]
    fn duplicate_rejected() {
        let reg = SubscriptionRegistry::new();
        let sub = test_sub(sub_id(1), owner_a(), TriggerCondition::BlockNumber { block: 100 }, 5);
        reg.subscribe(sub.clone()).unwrap();

        let err = reg.subscribe(sub).unwrap_err();
        assert!(matches!(err, SubscriptionError::Duplicate(_)));
    }

    #[test]
    fn unsubscribe() {
        let reg = SubscriptionRegistry::new();
        reg.subscribe(test_sub(
            sub_id(1),
            owner_a(),
            TriggerCondition::BlockNumber { block: 100 },
            5,
        ))
        .unwrap();

        reg.unsubscribe(sub_id(1), owner_a()).unwrap();
        assert!(!reg.get(sub_id(1)).unwrap().active);
    }

    #[test]
    fn unsubscribe_wrong_owner() {
        let reg = SubscriptionRegistry::new();
        reg.subscribe(test_sub(
            sub_id(1),
            owner_a(),
            TriggerCondition::BlockNumber { block: 100 },
            5,
        ))
        .unwrap();

        let err = reg.unsubscribe(sub_id(1), owner_b()).unwrap_err();
        assert!(matches!(err, SubscriptionError::Unauthorized { .. }));
    }

    #[test]
    fn evaluate_balance_below() {
        let reg = SubscriptionRegistry::new();
        reg.subscribe(test_sub(
            sub_id(1),
            owner_a(),
            TriggerCondition::BalanceBelow { address: addr(0x01), threshold: U256::from(1000u64) },
            3,
        ))
        .unwrap();

        // Balance above threshold — no fire.
        let mut ctx = empty_context(100);
        ctx.balances.insert(addr(0x01), U256::from(2000u64));
        assert!(reg.evaluate(&ctx).is_empty());

        // Balance below threshold — fires.
        ctx.balances.insert(addr(0x01), U256::from(500u64));
        let fired = reg.evaluate(&ctx);
        assert_eq!(fired.len(), 1);
        assert_eq!(fired[0].id, sub_id(1));

        // Trigger count should increment.
        assert_eq!(reg.get(sub_id(1)).unwrap().trigger_count, 1);
    }

    #[test]
    fn evaluate_balance_above() {
        let reg = SubscriptionRegistry::new();
        reg.subscribe(test_sub(
            sub_id(1),
            owner_a(),
            TriggerCondition::BalanceAbove { address: addr(0x01), threshold: U256::from(1000u64) },
            3,
        ))
        .unwrap();

        let mut ctx = empty_context(100);
        ctx.balances.insert(addr(0x01), U256::from(2000u64));
        let fired = reg.evaluate(&ctx);
        assert_eq!(fired.len(), 1);
    }

    #[test]
    fn evaluate_storage_changed() {
        let reg = SubscriptionRegistry::new();
        let slot = B256::repeat_byte(0x42);
        reg.subscribe(test_sub(
            sub_id(1),
            owner_a(),
            TriggerCondition::StorageChanged { address: addr(0x01), slot },
            3,
        ))
        .unwrap();

        let mut ctx = empty_context(100);
        // Wrong slot — no fire.
        ctx.storage_changes.push((addr(0x01), B256::ZERO));
        assert!(reg.evaluate(&ctx).is_empty());

        // Correct slot — fires.
        ctx.storage_changes.push((addr(0x01), slot));
        assert_eq!(reg.evaluate(&ctx).len(), 1);
    }

    #[test]
    fn evaluate_event_emitted() {
        let reg = SubscriptionRegistry::new();
        let topic = B256::repeat_byte(0xEE);
        reg.subscribe(test_sub(
            sub_id(1),
            owner_a(),
            TriggerCondition::EventEmitted { address: addr(0x01), topic0: topic },
            3,
        ))
        .unwrap();

        let mut ctx = empty_context(100);
        ctx.events.push((addr(0x01), topic));
        assert_eq!(reg.evaluate(&ctx).len(), 1);
    }

    #[test]
    fn evaluate_block_number() {
        let reg = SubscriptionRegistry::new();
        reg.subscribe(test_sub(
            sub_id(1),
            owner_a(),
            TriggerCondition::BlockNumber { block: 100 },
            1,
        ))
        .unwrap();

        // Block 99 — no fire.
        assert!(reg.evaluate(&empty_context(99)).is_empty());

        // Block 100 — fires and auto-deactivates (max_triggers = 1).
        let fired = reg.evaluate(&empty_context(100));
        assert_eq!(fired.len(), 1);
        assert!(!reg.get(sub_id(1)).unwrap().active);
    }

    #[test]
    fn evaluate_gas_price_below() {
        let reg = SubscriptionRegistry::new();
        reg.subscribe(test_sub(
            sub_id(1),
            owner_a(),
            TriggerCondition::GasPriceBelow {
                threshold: U256::from(10_000_000_000u64), // 10 gwei
            },
            3,
        ))
        .unwrap();

        // 20 gwei — no fire.
        assert!(reg.evaluate(&empty_context(100)).is_empty());

        // 5 gwei — fires.
        let mut ctx = empty_context(100);
        ctx.gas_price = U256::from(5_000_000_000u64);
        assert_eq!(reg.evaluate(&ctx).len(), 1);
    }

    #[test]
    fn auto_deactivate_on_trigger_exhaustion() {
        let reg = SubscriptionRegistry::new();
        reg.subscribe(test_sub(
            sub_id(1),
            owner_a(),
            TriggerCondition::BlockNumber { block: 0 },
            2,
        ))
        .unwrap();

        reg.evaluate(&empty_context(1));
        assert!(reg.get(sub_id(1)).unwrap().active);
        assert_eq!(reg.get(sub_id(1)).unwrap().trigger_count, 1);

        reg.evaluate(&empty_context(2));
        assert!(!reg.get(sub_id(1)).unwrap().active);
        assert_eq!(reg.get(sub_id(1)).unwrap().trigger_count, 2);

        // Should not fire again.
        assert!(reg.evaluate(&empty_context(3)).is_empty());
    }

    #[test]
    fn inactive_subs_not_evaluated() {
        let reg = SubscriptionRegistry::new();
        reg.subscribe(test_sub(
            sub_id(1),
            owner_a(),
            TriggerCondition::BlockNumber { block: 0 },
            10,
        ))
        .unwrap();

        reg.unsubscribe(sub_id(1), owner_a()).unwrap();
        assert!(reg.evaluate(&empty_context(100)).is_empty());
    }

    #[test]
    fn list_for_owner() {
        let reg = SubscriptionRegistry::new();
        reg.subscribe(test_sub(
            sub_id(1),
            owner_a(),
            TriggerCondition::BlockNumber { block: 100 },
            5,
        ))
        .unwrap();
        reg.subscribe(test_sub(
            sub_id(2),
            owner_a(),
            TriggerCondition::BlockNumber { block: 200 },
            5,
        ))
        .unwrap();
        reg.subscribe(test_sub(
            sub_id(3),
            owner_b(),
            TriggerCondition::BlockNumber { block: 300 },
            5,
        ))
        .unwrap();

        assert_eq!(reg.list_for_owner(owner_a()).len(), 2);
        assert_eq!(reg.list_for_owner(owner_b()).len(), 1);
    }

    #[test]
    fn list_active() {
        let reg = SubscriptionRegistry::new();
        reg.subscribe(test_sub(
            sub_id(1),
            owner_a(),
            TriggerCondition::BlockNumber { block: 100 },
            5,
        ))
        .unwrap();
        reg.subscribe(test_sub(
            sub_id(2),
            owner_a(),
            TriggerCondition::BlockNumber { block: 200 },
            5,
        ))
        .unwrap();

        reg.unsubscribe(sub_id(1), owner_a()).unwrap();
        assert_eq!(reg.list_active().len(), 1);
    }

    #[test]
    fn capacity_exceeded() {
        let reg = SubscriptionRegistry::new().with_max_subscriptions(1);
        reg.subscribe(test_sub(
            sub_id(1),
            owner_a(),
            TriggerCondition::BlockNumber { block: 100 },
            5,
        ))
        .unwrap();

        let err = reg
            .subscribe(test_sub(
                sub_id(2),
                owner_a(),
                TriggerCondition::BlockNumber { block: 200 },
                5,
            ))
            .unwrap_err();
        assert!(matches!(err, SubscriptionError::CapacityExceeded(1)));
    }

    #[test]
    fn len_and_is_empty() {
        let reg = SubscriptionRegistry::new();
        assert!(reg.is_empty());
        assert_eq!(reg.len(), 0);

        reg.subscribe(test_sub(
            sub_id(1),
            owner_a(),
            TriggerCondition::BlockNumber { block: 100 },
            5,
        ))
        .unwrap();
        assert!(!reg.is_empty());
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn thread_safety() {
        let reg = Arc::new(SubscriptionRegistry::new());
        let mut handles = vec![];

        for i in 0..10u8 {
            let r = Arc::clone(&reg);
            handles.push(std::thread::spawn(move || {
                r.subscribe(test_sub(
                    sub_id(i),
                    owner_a(),
                    TriggerCondition::BlockNumber { block: u64::from(i) * 100 },
                    5,
                ))
                .unwrap();
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(reg.len(), 10);
    }

    #[test]
    fn error_codes() {
        assert_eq!(SubscriptionError::NotFound(sub_id(1)).code(), -32870);
        assert_eq!(SubscriptionError::Duplicate(sub_id(1)).code(), -32871);
        assert_eq!(SubscriptionError::TriggersExhausted(sub_id(1)).code(), -32872);
        assert_eq!(SubscriptionError::CapacityExceeded(10).code(), -32875);
    }
}
