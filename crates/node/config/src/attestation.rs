//! Configuration for the attestation module.

use serde::{Deserialize, Serialize};

/// Configuration for the attestation module.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttestationConfig {
    /// Whether the attestation module is enabled.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Maximum number of stored attestations.
    #[serde(default = "default_max_attestations")]
    pub max_attestations: usize,
}

impl Default for AttestationConfig {
    fn default() -> Self {
        Self { enabled: default_enabled(), max_attestations: default_max_attestations() }
    }
}

const fn default_enabled() -> bool {
    true
}

const fn default_max_attestations() -> usize {
    100_000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = AttestationConfig::default();
        assert!(config.enabled);
        assert_eq!(config.max_attestations, 100_000);
    }

    #[test]
    fn json_roundtrip() {
        let config = AttestationConfig { enabled: false, max_attestations: 1000 };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: AttestationConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, parsed);
    }
}
