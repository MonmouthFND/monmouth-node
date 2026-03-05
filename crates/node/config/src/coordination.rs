//! Configuration for the coordination protocol module.

use serde::{Deserialize, Serialize};

/// Configuration for the coordination protocol module.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoordinationConfig {
    /// Whether coordination is enabled.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Maximum number of active jobs.
    #[serde(default = "default_max_jobs")]
    pub max_jobs: usize,
}

impl Default for CoordinationConfig {
    fn default() -> Self {
        Self { enabled: default_enabled(), max_jobs: default_max_jobs() }
    }
}

const fn default_enabled() -> bool {
    true
}

const fn default_max_jobs() -> usize {
    10_000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = CoordinationConfig::default();
        assert!(config.enabled);
        assert_eq!(config.max_jobs, 10_000);
    }

    #[test]
    fn json_roundtrip() {
        let config = CoordinationConfig { enabled: false, max_jobs: 500 };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: CoordinationConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, parsed);
    }
}
