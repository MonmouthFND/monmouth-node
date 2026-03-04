//! Configuration for the simulation / preview module.

use serde::{Deserialize, Serialize};

/// Configuration for the simulation module.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimulationConfig {
    /// Whether the simulation module is enabled.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Maximum number of transactions in a bundle simulation.
    #[serde(default = "default_max_bundle_size")]
    pub max_bundle_size: usize,
    /// Maximum gas allowed for a single simulation.
    #[serde(default = "default_max_gas")]
    pub max_gas: u64,
    /// Timeout for simulation execution in milliseconds.
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
}

impl Default for SimulationConfig {
    fn default() -> Self {
        Self {
            enabled: default_enabled(),
            max_bundle_size: default_max_bundle_size(),
            max_gas: default_max_gas(),
            timeout_ms: default_timeout_ms(),
        }
    }
}

const fn default_enabled() -> bool {
    true
}

const fn default_max_bundle_size() -> usize {
    10
}

const fn default_max_gas() -> u64 {
    30_000_000
}

const fn default_timeout_ms() -> u64 {
    5000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = SimulationConfig::default();
        assert!(config.enabled);
        assert_eq!(config.max_bundle_size, 10);
        assert_eq!(config.max_gas, 30_000_000);
        assert_eq!(config.timeout_ms, 5000);
    }

    #[test]
    fn json_roundtrip() {
        let config = SimulationConfig { enabled: false, max_bundle_size: 5, max_gas: 15_000_000, timeout_ms: 2000 };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: SimulationConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, parsed);
    }
}
