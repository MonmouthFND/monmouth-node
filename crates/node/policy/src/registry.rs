//! Thread-safe policy registry with spending and rate tracking.

use std::{collections::HashMap, sync::Arc};

use alloy_primitives::U256;
use monmouth_agent_types::{
    AgentId, PolicyAction, PolicyDecision, PolicyRule, PolicyRuleId, VmTarget,
};
use parking_lot::RwLock;
use tracing::{debug, info};

use crate::PolicyError;

/// Default maximum number of policy rules.
pub const DEFAULT_MAX_RULES: usize = 10_000;

/// Tracks cumulative spending within a time window.
#[derive(Debug, Clone)]
pub(crate) struct SpendingWindow {
    /// Total amount spent in the current window.
    pub(crate) total_spent: U256,
    /// Unix timestamp when the current window started.
    pub(crate) window_start: u64,
}

/// Tracks operation count within a time window.
#[derive(Debug, Clone)]
pub(crate) struct RateBucket {
    /// Number of operations in the current window.
    pub(crate) count: u64,
    /// Unix timestamp when the current window started.
    pub(crate) window_start: u64,
}

/// Internal state protected by the lock.
#[derive(Debug)]
struct Inner {
    /// Registered policy rules.
    rules: HashMap<PolicyRuleId, PolicyRule>,
    /// Per-agent spending windows, keyed by (agent, optional capability).
    spending_tracker: HashMap<(AgentId, Option<String>), SpendingWindow>,
    /// Per-agent rate buckets, keyed by (agent, optional capability).
    rate_tracker: HashMap<(AgentId, Option<String>), RateBucket>,
}

/// Thread-safe registry of policy rules with spending and rate tracking.
///
/// Follows the same concurrency pattern as `FilterRegistry` and
/// `CapabilityRegistry` -- uses `parking_lot::RwLock` for synchronous,
/// non-async critical sections.
#[derive(Debug, Clone)]
pub struct PolicyRegistry {
    inner: Arc<RwLock<Inner>>,
    max_rules: usize,
}

impl Default for PolicyRegistry {
    fn default() -> Self {
        Self {
            inner: Arc::new(RwLock::new(Inner {
                rules: HashMap::new(),
                spending_tracker: HashMap::new(),
                rate_tracker: HashMap::new(),
            })),
            max_rules: DEFAULT_MAX_RULES,
        }
    }
}

impl PolicyRegistry {
    /// Create a new, empty policy registry with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the maximum number of rules this registry will accept.
    #[must_use]
    pub const fn with_max_rules(mut self, max: usize) -> Self {
        self.max_rules = max;
        self
    }

    /// Add a policy rule to the registry.
    ///
    /// # Errors
    ///
    /// Returns an error if the registry is at capacity or a rule with the
    /// same ID already exists.
    pub fn add_rule(&self, rule: PolicyRule) -> Result<(), PolicyError> {
        let mut inner = self.inner.write();

        if inner.rules.len() >= self.max_rules {
            return Err(PolicyError::CapacityExceeded(self.max_rules));
        }

        if inner.rules.contains_key(&rule.id) {
            return Err(PolicyError::DuplicateRule(rule.id));
        }

        info!(id = %rule.id, agent = ?rule.agent, action = ?rule.action, "Added policy rule");
        inner.rules.insert(rule.id, rule);
        Ok(())
    }

    /// Remove a policy rule by ID, returning it.
    ///
    /// # Errors
    ///
    /// Returns an error if no rule exists with the given ID.
    pub fn remove_rule(&self, id: PolicyRuleId) -> Result<PolicyRule, PolicyError> {
        let mut inner = self.inner.write();

        inner.rules.remove(&id).ok_or(PolicyError::RuleNotFound(id)).inspect(|rule| {
            info!(id = %rule.id, "Removed policy rule");
        })
    }

    /// Return all rules matching the given agent (agent-specific + global).
    ///
    /// If `agent` is `None`, only global rules are returned.
    #[must_use]
    pub fn rules_for_agent(&self, agent: Option<AgentId>) -> Vec<PolicyRule> {
        let inner = self.inner.read();
        inner
            .rules
            .values()
            .filter(|rule| {
                // Global rules (rule.agent == None) always match.
                // Agent-specific rules match if the agent matches.
                rule.agent.is_none() || rule.agent == agent
            })
            .cloned()
            .collect()
    }

    /// Return all registered rules.
    #[must_use]
    pub fn list_rules(&self) -> Vec<PolicyRule> {
        let inner = self.inner.read();
        inner.rules.values().cloned().collect()
    }

    /// Evaluate policy for an agent action and return a decision.
    ///
    /// Logic:
    /// 1. Find rules matching this agent (agent-specific first, then global).
    /// 2. Check spending caps: if tracker shows exceeded, deny.
    /// 3. Check rate limits: if bucket shows exceeded, deny.
    /// 4. Return the most restrictive matching action
    ///    (Deny > RequireConfirmation > Allow).
    /// 5. If no rules match, default to Allow.
    #[must_use]
    pub fn evaluate(
        &self,
        agent: AgentId,
        capability_id: Option<&str>,
        target: Option<VmTarget>,
        value_wei: U256,
        timestamp: u64,
    ) -> PolicyDecision {
        let inner = self.inner.read();
        self.evaluate_inner(&inner, agent, capability_id, target, value_wei, timestamp)
    }

    /// Inner evaluation logic operating on a borrowed `Inner`.
    fn evaluate_inner(
        &self,
        inner: &Inner,
        agent: AgentId,
        capability_id: Option<&str>,
        target: Option<VmTarget>,
        value_wei: U256,
        timestamp: u64,
    ) -> PolicyDecision {
        // Collect matching rules: agent-specific first, then global.
        let matching: Vec<&PolicyRule> = inner
            .rules
            .values()
            .filter(|rule| {
                // Must match agent: global (None) or same agent.
                let agent_match = rule.agent.is_none() || rule.agent == Some(agent);
                if !agent_match {
                    return false;
                }

                // Must match capability if specified on the rule.
                if let Some(ref rule_cap) = rule.capability_id {
                    if let Some(req_cap) = capability_id {
                        if rule_cap != req_cap {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }

                // target is not filtered in rules currently -- reserved for future use.
                let _ = target;

                true
            })
            .collect();

        if matching.is_empty() {
            debug!(agent = %agent, "No policy rules match, defaulting to Allow");
            return PolicyDecision {
                action: PolicyAction::Allow,
                matched_rule: None,
                reason: "no matching rules, default allow".to_string(),
            };
        }

        // Check spending caps and rate limits against trackers.
        let spending_key = (agent, capability_id.map(String::from));

        for rule in &matching {
            // Check spending cap.
            if let Some(ref cap) = rule.spending_cap
                && let Some(window) = inner.spending_tracker.get(&spending_key)
            {
                let window_active = timestamp.saturating_sub(window.window_start) < cap.window_secs;
                if window_active && window.total_spent + value_wei > cap.max_wei {
                    debug!(
                        agent = %agent,
                        rule = %rule.id,
                        spent = %window.total_spent,
                        cap = %cap.max_wei,
                        "Spending cap exceeded"
                    );
                    return PolicyDecision {
                        action: PolicyAction::Deny,
                        matched_rule: Some(rule.id),
                        reason: format!(
                            "spending cap exceeded: {} + {} > {} (rule {})",
                            window.total_spent, value_wei, cap.max_wei, rule.id
                        ),
                    };
                }
            }

            // Check rate limit.
            if let Some(ref limit) = rule.rate_limit
                && let Some(bucket) = inner.rate_tracker.get(&spending_key)
            {
                let window_active =
                    timestamp.saturating_sub(bucket.window_start) < limit.window_secs;
                if window_active && bucket.count >= limit.max_ops {
                    debug!(
                        agent = %agent,
                        rule = %rule.id,
                        count = bucket.count,
                        max = limit.max_ops,
                        "Rate limit exceeded"
                    );
                    return PolicyDecision {
                        action: PolicyAction::Deny,
                        matched_rule: Some(rule.id),
                        reason: format!(
                            "rate limit exceeded: {} >= {} ops (rule {})",
                            bucket.count, limit.max_ops, rule.id
                        ),
                    };
                }
            }
        }

        // Find the most restrictive action among matching rules.
        let mut most_restrictive = PolicyAction::Allow;
        let mut matched_rule = None;

        for rule in &matching {
            let strictness = action_strictness(rule.action);
            if strictness > action_strictness(most_restrictive) {
                most_restrictive = rule.action;
                matched_rule = Some(rule.id);
            }
        }

        debug!(
            agent = %agent,
            action = ?most_restrictive,
            rule = ?matched_rule,
            "Policy evaluation complete"
        );

        PolicyDecision {
            action: most_restrictive,
            matched_rule,
            reason: match most_restrictive {
                PolicyAction::Allow => "allowed by policy".to_string(),
                PolicyAction::Deny => "denied by policy".to_string(),
                PolicyAction::RequireConfirmation => "requires human confirmation".to_string(),
            },
        }
    }

    /// Atomically evaluate policy and record execution if allowed.
    ///
    /// This combines `evaluate()` and `record_execution()` under a single
    /// write lock to prevent race conditions where concurrent operations
    /// could bypass spending or rate limits.
    ///
    /// Returns the policy decision. If the action is `Allow`, spending and
    /// rate trackers are updated atomically within the same lock acquisition.
    #[must_use]
    pub fn evaluate_and_record(
        &self,
        agent: AgentId,
        capability_id: Option<&str>,
        target: Option<VmTarget>,
        value_wei: U256,
        timestamp: u64,
    ) -> PolicyDecision {
        let mut inner = self.inner.write();

        let decision =
            self.evaluate_inner(&inner, agent, capability_id, target, value_wei, timestamp);

        if decision.action == PolicyAction::Allow {
            self.record_inner(
                &mut inner,
                agent,
                capability_id.map(String::from),
                value_wei,
                timestamp,
            );
        }

        decision
    }

    /// Record that an agent executed an action, updating spending and rate
    /// trackers.
    ///
    /// Resets the window if the previous window has expired.
    pub fn record_execution(
        &self,
        agent: AgentId,
        capability_id: Option<String>,
        value_wei: U256,
        timestamp: u64,
    ) {
        let mut inner = self.inner.write();
        self.record_inner(&mut inner, agent, capability_id, value_wei, timestamp);
    }

    /// Inner recording logic operating on a mutably borrowed `Inner`.
    fn record_inner(
        &self,
        inner: &mut Inner,
        agent: AgentId,
        capability_id: Option<String>,
        value_wei: U256,
        timestamp: u64,
    ) {
        let key = (agent, capability_id);

        // Determine the applicable spending window duration from matching rules.
        let spending_window_secs = inner
            .rules
            .values()
            .filter(|r| r.agent.is_none() || r.agent == Some(agent))
            .filter_map(|r| r.spending_cap.as_ref())
            .map(|cap| cap.window_secs)
            .next();

        // Update spending tracker.
        if let Some(window_secs) = spending_window_secs {
            let entry = inner.spending_tracker.entry(key.clone()).or_insert_with(|| {
                SpendingWindow { total_spent: U256::ZERO, window_start: timestamp }
            });

            if timestamp.saturating_sub(entry.window_start) >= window_secs {
                // Window expired, reset.
                entry.total_spent = value_wei;
                entry.window_start = timestamp;
            } else {
                entry.total_spent += value_wei;
            }
        }

        // Determine the applicable rate limit window duration.
        let rate_window_secs = inner
            .rules
            .values()
            .filter(|r| r.agent.is_none() || r.agent == Some(agent))
            .filter_map(|r| r.rate_limit.as_ref())
            .map(|rl| rl.window_secs)
            .next();

        // Update rate tracker.
        if let Some(window_secs) = rate_window_secs {
            let entry = inner
                .rate_tracker
                .entry(key)
                .or_insert_with(|| RateBucket { count: 0, window_start: timestamp });

            if timestamp.saturating_sub(entry.window_start) >= window_secs {
                // Window expired, reset.
                entry.count = 1;
                entry.window_start = timestamp;
            } else {
                entry.count += 1;
            }
        }
    }

    /// Returns the number of registered rules.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.read().rules.len()
    }

    /// Returns `true` if the registry contains no rules.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.read().rules.is_empty()
    }
}

/// Map a [`PolicyAction`] to a numeric strictness for comparison.
/// Higher is more restrictive.
const fn action_strictness(action: PolicyAction) -> u8 {
    match action {
        PolicyAction::Allow => 0,
        PolicyAction::RequireConfirmation => 1,
        PolicyAction::Deny => 2,
    }
}

#[cfg(test)]
mod tests {
    use alloy_primitives::{Address, B256};
    use monmouth_agent_types::{
        AgentId, PolicyAction, PolicyRule, PolicyRuleId, RateLimitRule, SpendingCap,
    };

    use super::*;

    fn agent_a() -> AgentId {
        AgentId(Address::repeat_byte(0xAA))
    }

    fn agent_b() -> AgentId {
        AgentId(Address::repeat_byte(0xBB))
    }

    fn rule_id(byte: u8) -> PolicyRuleId {
        PolicyRuleId(B256::repeat_byte(byte))
    }

    fn allow_rule(id: PolicyRuleId, agent: Option<AgentId>) -> PolicyRule {
        PolicyRule {
            id,
            agent,
            capability_id: None,
            action: PolicyAction::Allow,
            spending_cap: None,
            rate_limit: None,
        }
    }

    fn deny_rule(id: PolicyRuleId, agent: Option<AgentId>) -> PolicyRule {
        PolicyRule {
            id,
            agent,
            capability_id: None,
            action: PolicyAction::Deny,
            spending_cap: None,
            rate_limit: None,
        }
    }

    fn confirm_rule(id: PolicyRuleId, agent: Option<AgentId>) -> PolicyRule {
        PolicyRule {
            id,
            agent,
            capability_id: None,
            action: PolicyAction::RequireConfirmation,
            spending_cap: None,
            rate_limit: None,
        }
    }

    #[test]
    fn default_allow_when_no_rules() {
        let registry = PolicyRegistry::new();
        let decision = registry.evaluate(agent_a(), None, None, U256::ZERO, 1000);
        assert_eq!(decision.action, PolicyAction::Allow);
        assert!(decision.matched_rule.is_none());
    }

    #[test]
    fn add_and_remove_rules() {
        let registry = PolicyRegistry::new();
        let rule = allow_rule(rule_id(1), None);

        registry.add_rule(rule.clone()).unwrap();
        assert_eq!(registry.len(), 1);
        assert!(!registry.is_empty());

        let removed = registry.remove_rule(rule_id(1)).unwrap();
        assert_eq!(removed.id, rule.id);
        assert!(registry.is_empty());
    }

    #[test]
    fn remove_nonexistent_fails() {
        let registry = PolicyRegistry::new();
        let err = registry.remove_rule(rule_id(99)).unwrap_err();
        assert!(matches!(err, PolicyError::RuleNotFound(_)));
    }

    #[test]
    fn duplicate_rule_fails() {
        let registry = PolicyRegistry::new();
        registry.add_rule(allow_rule(rule_id(1), None)).unwrap();
        let err = registry.add_rule(allow_rule(rule_id(1), None)).unwrap_err();
        assert!(matches!(err, PolicyError::DuplicateRule(_)));
    }

    #[test]
    fn agent_specific_vs_global_rules() {
        let registry = PolicyRegistry::new();

        // Global allow rule.
        registry.add_rule(allow_rule(rule_id(1), None)).unwrap();

        // Agent-specific deny for agent_a.
        registry.add_rule(deny_rule(rule_id(2), Some(agent_a()))).unwrap();

        // agent_a should be denied (most restrictive wins).
        let decision = registry.evaluate(agent_a(), None, None, U256::ZERO, 1000);
        assert_eq!(decision.action, PolicyAction::Deny);

        // agent_b should be allowed (only global rule matches).
        let decision = registry.evaluate(agent_b(), None, None, U256::ZERO, 1000);
        assert_eq!(decision.action, PolicyAction::Allow);
    }

    #[test]
    fn rules_for_agent_returns_matching() {
        let registry = PolicyRegistry::new();
        registry.add_rule(allow_rule(rule_id(1), None)).unwrap();
        registry.add_rule(deny_rule(rule_id(2), Some(agent_a()))).unwrap();
        registry.add_rule(allow_rule(rule_id(3), Some(agent_b()))).unwrap();

        let rules = registry.rules_for_agent(Some(agent_a()));
        // Should include global (id=1) and agent_a specific (id=2).
        assert_eq!(rules.len(), 2);

        let rules = registry.rules_for_agent(None);
        // Only global rules.
        assert_eq!(rules.len(), 1);
    }

    #[test]
    fn list_rules_returns_all() {
        let registry = PolicyRegistry::new();
        registry.add_rule(allow_rule(rule_id(1), None)).unwrap();
        registry.add_rule(deny_rule(rule_id(2), Some(agent_a()))).unwrap();

        let all = registry.list_rules();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn spending_cap_enforcement_within_window() {
        let registry = PolicyRegistry::new();

        let rule = PolicyRule {
            id: rule_id(1),
            agent: Some(agent_a()),
            capability_id: None,
            action: PolicyAction::Allow,
            spending_cap: Some(SpendingCap { max_wei: U256::from(1000u64), window_secs: 3600 }),
            rate_limit: None,
        };
        registry.add_rule(rule).unwrap();

        // Record some spending.
        registry.record_execution(agent_a(), None, U256::from(800u64), 100);

        // Should still allow up to cap.
        let decision = registry.evaluate(agent_a(), None, None, U256::from(100u64), 100);
        assert_eq!(decision.action, PolicyAction::Allow);

        // Should deny exceeding cap.
        let decision = registry.evaluate(agent_a(), None, None, U256::from(300u64), 100);
        assert_eq!(decision.action, PolicyAction::Deny);
        assert!(decision.reason.contains("spending cap exceeded"));
    }

    #[test]
    fn spending_cap_window_reset() {
        let registry = PolicyRegistry::new();

        let rule = PolicyRule {
            id: rule_id(1),
            agent: Some(agent_a()),
            capability_id: None,
            action: PolicyAction::Allow,
            spending_cap: Some(SpendingCap { max_wei: U256::from(1000u64), window_secs: 3600 }),
            rate_limit: None,
        };
        registry.add_rule(rule).unwrap();

        // Record spending at t=100.
        registry.record_execution(agent_a(), None, U256::from(900u64), 100);

        // At t=100 should deny exceeding.
        let decision = registry.evaluate(agent_a(), None, None, U256::from(200u64), 100);
        assert_eq!(decision.action, PolicyAction::Deny);

        // Record at t=4000 (window expired: 4000 - 100 = 3900 >= 3600).
        // This resets the window.
        registry.record_execution(agent_a(), None, U256::from(100u64), 4000);

        // Now at t=4000, only 100 spent in new window, should allow.
        let decision = registry.evaluate(agent_a(), None, None, U256::from(800u64), 4000);
        assert_eq!(decision.action, PolicyAction::Allow);
    }

    #[test]
    fn rate_limit_enforcement_within_window() {
        let registry = PolicyRegistry::new();

        let rule = PolicyRule {
            id: rule_id(1),
            agent: Some(agent_a()),
            capability_id: None,
            action: PolicyAction::Allow,
            spending_cap: None,
            rate_limit: Some(RateLimitRule { max_ops: 3, window_secs: 60 }),
        };
        registry.add_rule(rule).unwrap();

        // Record 3 operations.
        registry.record_execution(agent_a(), None, U256::ZERO, 100);
        registry.record_execution(agent_a(), None, U256::ZERO, 101);
        registry.record_execution(agent_a(), None, U256::ZERO, 102);

        // 4th should be denied.
        let decision = registry.evaluate(agent_a(), None, None, U256::ZERO, 103);
        assert_eq!(decision.action, PolicyAction::Deny);
        assert!(decision.reason.contains("rate limit exceeded"));
    }

    #[test]
    fn rate_limit_window_reset() {
        let registry = PolicyRegistry::new();

        let rule = PolicyRule {
            id: rule_id(1),
            agent: Some(agent_a()),
            capability_id: None,
            action: PolicyAction::Allow,
            spending_cap: None,
            rate_limit: Some(RateLimitRule { max_ops: 2, window_secs: 60 }),
        };
        registry.add_rule(rule).unwrap();

        // Use up the limit.
        registry.record_execution(agent_a(), None, U256::ZERO, 100);
        registry.record_execution(agent_a(), None, U256::ZERO, 101);

        // Denied at t=102.
        let decision = registry.evaluate(agent_a(), None, None, U256::ZERO, 102);
        assert_eq!(decision.action, PolicyAction::Deny);

        // Record at t=200 (window expired: 200 - 100 = 100 >= 60).
        registry.record_execution(agent_a(), None, U256::ZERO, 200);

        // Now at t=200, only 1 op in new window, should allow.
        let decision = registry.evaluate(agent_a(), None, None, U256::ZERO, 200);
        assert_eq!(decision.action, PolicyAction::Allow);
    }

    #[test]
    fn most_restrictive_action_wins() {
        let registry = PolicyRegistry::new();

        // Global allow.
        registry.add_rule(allow_rule(rule_id(1), None)).unwrap();

        // Global require-confirmation.
        registry.add_rule(confirm_rule(rule_id(2), None)).unwrap();

        let decision = registry.evaluate(agent_a(), None, None, U256::ZERO, 1000);
        assert_eq!(decision.action, PolicyAction::RequireConfirmation);
    }

    #[test]
    fn deny_overrides_allow() {
        let registry = PolicyRegistry::new();

        // Agent-specific allow.
        registry.add_rule(allow_rule(rule_id(1), Some(agent_a()))).unwrap();

        // Global deny.
        registry.add_rule(deny_rule(rule_id(2), None)).unwrap();

        let decision = registry.evaluate(agent_a(), None, None, U256::ZERO, 1000);
        assert_eq!(decision.action, PolicyAction::Deny);
    }

    #[test]
    fn require_confirmation_behavior() {
        let registry = PolicyRegistry::new();

        registry.add_rule(confirm_rule(rule_id(1), Some(agent_a()))).unwrap();

        let decision = registry.evaluate(agent_a(), None, None, U256::ZERO, 1000);
        assert_eq!(decision.action, PolicyAction::RequireConfirmation);
        assert!(decision.reason.contains("confirmation"));
    }

    #[test]
    fn capacity_exceeded() {
        let registry = PolicyRegistry::new().with_max_rules(2);
        registry.add_rule(allow_rule(rule_id(1), None)).unwrap();
        registry.add_rule(allow_rule(rule_id(2), None)).unwrap();

        let err = registry.add_rule(allow_rule(rule_id(3), None)).unwrap_err();
        assert!(matches!(err, PolicyError::CapacityExceeded(2)));
    }

    #[test]
    fn thread_safety() {
        let registry = Arc::new(PolicyRegistry::new());
        let mut handles = vec![];

        for i in 0..10u8 {
            let reg = Arc::clone(&registry);
            handles.push(std::thread::spawn(move || {
                let rule = allow_rule(rule_id(i), None);
                reg.add_rule(rule).unwrap();
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(registry.len(), 10);
    }

    #[test]
    fn evaluate_and_record_atomic() {
        let registry = PolicyRegistry::new();

        let rule = PolicyRule {
            id: rule_id(1),
            agent: Some(agent_a()),
            capability_id: None,
            action: PolicyAction::Allow,
            spending_cap: None,
            rate_limit: Some(RateLimitRule { max_ops: 2, window_secs: 60 }),
        };
        registry.add_rule(rule).unwrap();

        // First two should be allowed and recorded atomically.
        let d1 = registry.evaluate_and_record(agent_a(), None, None, U256::ZERO, 100);
        assert_eq!(d1.action, PolicyAction::Allow);
        let d2 = registry.evaluate_and_record(agent_a(), None, None, U256::ZERO, 101);
        assert_eq!(d2.action, PolicyAction::Allow);

        // Third should be denied (2 ops already recorded).
        let d3 = registry.evaluate_and_record(agent_a(), None, None, U256::ZERO, 102);
        assert_eq!(d3.action, PolicyAction::Deny);
    }

    #[test]
    fn error_codes() {
        assert_eq!(PolicyError::RuleNotFound(rule_id(1)).code(), -32700);
        assert_eq!(PolicyError::DuplicateRule(rule_id(1)).code(), -32701);
        assert_eq!(PolicyError::CapacityExceeded(10).code(), -32702);
        assert_eq!(
            PolicyError::Denied { agent: agent_a(), reason: "test".to_string() }.code(),
            -32703
        );
    }
}
