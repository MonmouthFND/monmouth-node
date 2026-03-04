//! Configuration for the policy engine module.

use serde::{Deserialize, Serialize};

/// Configuration for the policy engine module.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyConfig {
    /// Whether the policy engine is enabled.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Pre-configured policy rules loaded at startup.
    #[serde(default)]
    pub rules: Vec<PolicyRuleDef>,
}

impl Default for PolicyConfig {
    fn default() -> Self {
        Self { enabled: default_enabled(), rules: Vec::new() }
    }
}

/// A policy rule definition in configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyRuleDef {
    /// Optional agent address this rule applies to (hex string).
    #[serde(default)]
    pub agent: Option<String>,
    /// Optional capability ID filter.
    #[serde(default)]
    pub capability_id: Option<String>,
    /// Action: "allow", "deny", or "require_confirmation".
    pub action: String,
    /// Optional spending cap in wei (decimal string).
    #[serde(default)]
    pub spending_cap_wei: Option<String>,
    /// Optional spending cap window in seconds.
    #[serde(default)]
    pub spending_cap_window_secs: Option<u64>,
    /// Optional rate limit max operations.
    #[serde(default)]
    pub rate_limit_max_ops: Option<u64>,
    /// Optional rate limit window in seconds.
    #[serde(default)]
    pub rate_limit_window_secs: Option<u64>,
}

const fn default_enabled() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = PolicyConfig::default();
        assert!(config.enabled);
        assert!(config.rules.is_empty());
    }

    #[test]
    fn json_roundtrip() {
        let config = PolicyConfig {
            enabled: true,
            rules: vec![PolicyRuleDef {
                agent: None,
                capability_id: Some("sim.preview".to_string()),
                action: "allow".to_string(),
                spending_cap_wei: Some("1000000".to_string()),
                spending_cap_window_secs: Some(3600),
                rate_limit_max_ops: Some(100),
                rate_limit_window_secs: Some(60),
            }],
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: PolicyConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, parsed);
    }
}
