//! Configuration for the conditional subscriptions module.

use serde::{Deserialize, Serialize};

/// Configuration for the conditional subscriptions module.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConditionalSubsConfig {
    /// Whether conditional subscriptions are enabled.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Maximum number of active subscriptions.
    #[serde(default = "default_max_subscriptions")]
    pub max_subscriptions: usize,
}

impl Default for ConditionalSubsConfig {
    fn default() -> Self {
        Self { enabled: default_enabled(), max_subscriptions: default_max_subscriptions() }
    }
}

const fn default_enabled() -> bool {
    true
}

const fn default_max_subscriptions() -> usize {
    10_000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = ConditionalSubsConfig::default();
        assert!(config.enabled);
        assert_eq!(config.max_subscriptions, 10_000);
    }

    #[test]
    fn json_roundtrip() {
        let config = ConditionalSubsConfig { enabled: false, max_subscriptions: 100 };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: ConditionalSubsConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, parsed);
    }
}
