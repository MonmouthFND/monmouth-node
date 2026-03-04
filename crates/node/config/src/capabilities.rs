//! Configuration types for the capability registry.
//!
//! These types are separate from the runtime `monmouth-capabilities` crate
//! to keep the config crate dependency-free.

use serde::{Deserialize, Serialize};

/// Rate limit definition in configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RateLimitDef {
    /// Maximum number of requests within the window.
    pub max_requests: u64,
    /// Duration of the rate limit window in seconds.
    pub window_secs: u64,
}

/// Permission definition in configuration.
///
/// Uses string-based `kind` for TOML flexibility (e.g., `"execute"`, `"read"`, `"admin"`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionDef {
    /// The kind of permission (e.g., `"execute"`, `"read"`, `"admin"`).
    pub kind: String,
    /// The scope of the permission.
    pub scope: String,
}

/// Schema definition in configuration.
///
/// Only implements `PartialEq` because `serde_json::Value` does not implement `Eq`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SchemaDef {
    /// JSON Schema for the capability's input parameters.
    pub input: serde_json::Value,
    /// JSON Schema for the capability's output.
    pub output: serde_json::Value,
}

/// A single capability definition in configuration.
///
/// Only implements `PartialEq` because it contains `SchemaDef`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityDefinition {
    /// Unique identifier (lowercase, dot-separated).
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Description of the capability.
    #[serde(default)]
    pub description: String,
    /// Semantic version string.
    #[serde(default = "default_version")]
    pub version: String,
    /// Typed input/output schema.
    pub schema: SchemaDef,
    /// Required permissions.
    #[serde(default)]
    pub permissions: Vec<PermissionDef>,
    /// Optional rate limit.
    #[serde(default)]
    pub rate_limit: Option<RateLimitDef>,
    /// Whether this capability is enabled.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

/// Capabilities section of the node configuration.
///
/// Only implements `PartialEq` because it contains `CapabilityDefinition`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CapabilitiesConfig {
    /// List of capability definitions.
    #[serde(default)]
    pub capabilities: Vec<CapabilityDefinition>,
}


fn default_version() -> String {
    "1.0.0".to_string()
}

const fn default_enabled() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_empty() {
        let config = CapabilitiesConfig::default();
        assert!(config.capabilities.is_empty());
    }

    #[test]
    fn json_roundtrip() {
        let config = CapabilitiesConfig {
            capabilities: vec![CapabilityDefinition {
                id: "sim.preview".to_string(),
                name: "Simulation Preview".to_string(),
                description: "Preflight simulation".to_string(),
                version: "1.0.0".to_string(),
                schema: SchemaDef {
                    input: serde_json::json!({"type": "object"}),
                    output: serde_json::json!({"type": "string"}),
                },
                permissions: vec![PermissionDef {
                    kind: "execute".to_string(),
                    scope: "*".to_string(),
                }],
                rate_limit: Some(RateLimitDef { max_requests: 100, window_secs: 60 }),
                enabled: true,
            }],
        };

        let json = serde_json::to_string_pretty(&config).unwrap();
        let parsed: CapabilitiesConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, parsed);
    }

    #[test]
    fn toml_roundtrip() {
        let config = CapabilitiesConfig {
            capabilities: vec![CapabilityDefinition {
                id: "test.cap".to_string(),
                name: "Test".to_string(),
                description: String::new(),
                version: "1.0.0".to_string(),
                schema: SchemaDef {
                    input: serde_json::json!({}),
                    output: serde_json::json!({}),
                },
                permissions: vec![],
                rate_limit: None,
                enabled: true,
            }],
        };

        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: CapabilitiesConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(config, parsed);
    }

    #[test]
    fn partial_defaults() {
        let json = r#"{"capabilities": [{"id": "a", "name": "A", "schema": {"input": {}, "output": {}}}]}"#;
        let config: CapabilitiesConfig = serde_json::from_str(json).unwrap();
        let cap = &config.capabilities[0];
        assert_eq!(cap.version, "1.0.0");
        assert!(cap.enabled);
        assert!(cap.permissions.is_empty());
        assert!(cap.description.is_empty());
    }
}
