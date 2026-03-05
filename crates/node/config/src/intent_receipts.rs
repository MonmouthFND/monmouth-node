//! Configuration for the intent receipts module.

use serde::{Deserialize, Serialize};

/// Configuration for the intent receipts module.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntentReceiptsConfig {
    /// Whether intent receipts are enabled.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Maximum number of stored receipts.
    #[serde(default = "default_max_receipts")]
    pub max_receipts: usize,
}

impl Default for IntentReceiptsConfig {
    fn default() -> Self {
        Self { enabled: default_enabled(), max_receipts: default_max_receipts() }
    }
}

const fn default_enabled() -> bool {
    true
}

const fn default_max_receipts() -> usize {
    100_000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = IntentReceiptsConfig::default();
        assert!(config.enabled);
        assert_eq!(config.max_receipts, 100_000);
    }

    #[test]
    fn json_roundtrip() {
        let config = IntentReceiptsConfig { enabled: false, max_receipts: 500 };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: IntentReceiptsConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, parsed);
    }
}
