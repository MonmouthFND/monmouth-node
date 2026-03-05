//! Configuration for the SVM (Solana Virtual Machine) module.

use serde::{Deserialize, Serialize};

/// Configuration for the SVM module.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SvmConfig {
    /// Whether the SVM module is enabled.
    #[serde(default)]
    pub enabled: bool,
    /// Compute budget limit for SVM transactions.
    #[serde(default = "default_compute_budget")]
    pub compute_budget: u64,
}

impl Default for SvmConfig {
    fn default() -> Self {
        Self { enabled: false, compute_budget: default_compute_budget() }
    }
}

const fn default_compute_budget() -> u64 {
    200_000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = SvmConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.compute_budget, 200_000);
    }

    #[test]
    fn json_roundtrip() {
        let config = SvmConfig { enabled: true, compute_budget: 500_000 };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: SvmConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, parsed);
    }
}
