//! Configuration for the memory anchoring module.

use serde::{Deserialize, Serialize};

/// Configuration for the memory anchoring module.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryAnchoringConfig {
    /// Whether memory anchoring is enabled.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Maximum number of anchors per agent.
    #[serde(default = "default_max_anchors_per_agent")]
    pub max_anchors_per_agent: usize,
}

impl Default for MemoryAnchoringConfig {
    fn default() -> Self {
        Self {
            enabled: default_enabled(),
            max_anchors_per_agent: default_max_anchors_per_agent(),
        }
    }
}

const fn default_enabled() -> bool {
    true
}

const fn default_max_anchors_per_agent() -> usize {
    1_000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = MemoryAnchoringConfig::default();
        assert!(config.enabled);
        assert_eq!(config.max_anchors_per_agent, 1_000);
    }

    #[test]
    fn json_roundtrip() {
        let config = MemoryAnchoringConfig { enabled: false, max_anchors_per_agent: 50 };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: MemoryAnchoringConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, parsed);
    }
}
