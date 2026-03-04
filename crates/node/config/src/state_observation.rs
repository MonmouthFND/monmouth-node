//! Configuration for the state observation module.

use serde::{Deserialize, Serialize};

/// Configuration for the state observation module.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservationConfig {
    /// Whether state observation is enabled.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Maximum number of queries in a batch.
    #[serde(default = "default_max_batch_size")]
    pub max_batch_size: usize,
}

impl Default for ObservationConfig {
    fn default() -> Self {
        Self { enabled: default_enabled(), max_batch_size: default_max_batch_size() }
    }
}

const fn default_enabled() -> bool {
    true
}

const fn default_max_batch_size() -> usize {
    100
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = ObservationConfig::default();
        assert!(config.enabled);
        assert_eq!(config.max_batch_size, 100);
    }

    #[test]
    fn json_roundtrip() {
        let config = ObservationConfig { enabled: false, max_batch_size: 10 };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: ObservationConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, parsed);
    }
}
