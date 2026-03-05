//! Configuration for the delegation module.

use serde::{Deserialize, Serialize};

/// Configuration for the delegation module.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelegationConfig {
    /// Whether delegation is enabled.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Maximum number of active sessions.
    #[serde(default = "default_max_sessions")]
    pub max_sessions: usize,
    /// Maximum session duration in seconds.
    #[serde(default = "default_max_session_duration_secs")]
    pub max_session_duration_secs: u64,
}

impl Default for DelegationConfig {
    fn default() -> Self {
        Self {
            enabled: default_enabled(),
            max_sessions: default_max_sessions(),
            max_session_duration_secs: default_max_session_duration_secs(),
        }
    }
}

const fn default_enabled() -> bool {
    true
}

const fn default_max_sessions() -> usize {
    10_000
}

const fn default_max_session_duration_secs() -> u64 {
    86400
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = DelegationConfig::default();
        assert!(config.enabled);
        assert_eq!(config.max_sessions, 10_000);
        assert_eq!(config.max_session_duration_secs, 86400);
    }

    #[test]
    fn json_roundtrip() {
        let config =
            DelegationConfig { enabled: false, max_sessions: 100, max_session_duration_secs: 3600 };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: DelegationConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, parsed);
    }
}
