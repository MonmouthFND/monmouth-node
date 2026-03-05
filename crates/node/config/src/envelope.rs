//! Configuration for the transaction envelope module.

use serde::{Deserialize, Serialize};

/// Configuration for the transaction envelope module.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvelopeConfig {
    /// Whether to accept extended Monmouth envelope format (magic byte `0x4d`).
    #[serde(default = "default_accept_extended")]
    pub accept_extended: bool,
}

impl Default for EnvelopeConfig {
    fn default() -> Self {
        Self { accept_extended: default_accept_extended() }
    }
}

const fn default_accept_extended() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = EnvelopeConfig::default();
        assert!(config.accept_extended);
    }

    #[test]
    fn json_roundtrip() {
        let config = EnvelopeConfig { accept_extended: false };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: EnvelopeConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, parsed);
    }
}
