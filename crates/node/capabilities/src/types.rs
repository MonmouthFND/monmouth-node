//! Core types for the capability registry.

use serde::{Deserialize, Serialize};

/// The kind of permission required for a capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionKind {
    /// Permission to execute the capability.
    Execute,
    /// Permission to read data from the capability.
    Read,
    /// Administrative permission for managing the capability.
    Admin,
}

/// A permission requirement for a capability.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Permission {
    /// The kind of permission.
    pub kind: PermissionKind,
    /// The scope of the permission (e.g., a contract address or wildcard).
    pub scope: String,
}

/// Rate limit configuration for a capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RateLimit {
    /// Maximum number of requests allowed within the window.
    pub max_requests: u64,
    /// Duration of the rate limit window in seconds.
    pub window_secs: u64,
}

/// JSON Schema representation for capability input/output.
///
/// Uses `serde_json::Value` to represent arbitrary JSON Schema objects,
/// so this type only implements `PartialEq` (not `Eq`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilitySchema {
    /// JSON Schema for the capability's input parameters.
    pub input: serde_json::Value,
    /// JSON Schema for the capability's output.
    pub output: serde_json::Value,
}

/// Full representation of a registered capability.
///
/// Only implements `PartialEq` because `CapabilitySchema` contains `serde_json::Value`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Capability {
    /// Unique identifier (lowercase, dot-separated, e.g., `"sim.preview"`).
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Description of what this capability does.
    pub description: String,
    /// Semantic version string (e.g., `"1.0.0"`).
    pub version: String,
    /// Typed input/output schema.
    pub schema: CapabilitySchema,
    /// Required permissions.
    pub permissions: Vec<Permission>,
    /// Optional rate limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate_limit: Option<RateLimit>,
    /// Whether this capability is currently enabled.
    pub enabled: bool,
}

/// Lightweight summary of a capability for listing endpoints.
///
/// Implements both `PartialEq` and `Eq` since it contains no floating-point
/// or `serde_json::Value` fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilitySummary {
    /// Unique identifier.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Semantic version string.
    pub version: String,
    /// Whether this capability is currently enabled.
    pub enabled: bool,
}

impl From<&Capability> for CapabilitySummary {
    fn from(cap: &Capability) -> Self {
        Self {
            id: cap.id.clone(),
            name: cap.name.clone(),
            version: cap.version.clone(),
            enabled: cap.enabled,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_summary_from_capability() {
        let cap = Capability {
            id: "test.cap".to_string(),
            name: "Test Capability".to_string(),
            description: "A test capability".to_string(),
            version: "1.0.0".to_string(),
            schema: CapabilitySchema {
                input: serde_json::json!({"type": "object"}),
                output: serde_json::json!({"type": "string"}),
            },
            permissions: vec![Permission {
                kind: PermissionKind::Execute,
                scope: "*".to_string(),
            }],
            rate_limit: Some(RateLimit { max_requests: 100, window_secs: 60 }),
            enabled: true,
        };

        let summary = CapabilitySummary::from(&cap);
        assert_eq!(summary.id, "test.cap");
        assert_eq!(summary.name, "Test Capability");
        assert_eq!(summary.version, "1.0.0");
        assert!(summary.enabled);
    }

    #[test]
    fn capability_json_roundtrip() {
        let cap = Capability {
            id: "sim.preview".to_string(),
            name: "Simulation Preview".to_string(),
            description: "Deterministic preflight simulation".to_string(),
            version: "0.1.0".to_string(),
            schema: CapabilitySchema {
                input: serde_json::json!({"type": "object", "properties": {"tx": {"type": "string"}}}),
                output: serde_json::json!({"type": "object", "properties": {"diff": {"type": "array"}}}),
            },
            permissions: vec![
                Permission { kind: PermissionKind::Execute, scope: "*".to_string() },
                Permission { kind: PermissionKind::Read, scope: "state.*".to_string() },
            ],
            rate_limit: None,
            enabled: true,
        };

        let json = serde_json::to_string(&cap).unwrap();
        let parsed: Capability = serde_json::from_str(&json).unwrap();
        assert_eq!(cap, parsed);
    }

    #[test]
    fn permission_kind_serialization() {
        assert_eq!(serde_json::to_string(&PermissionKind::Execute).unwrap(), "\"execute\"");
        assert_eq!(serde_json::to_string(&PermissionKind::Read).unwrap(), "\"read\"");
        assert_eq!(serde_json::to_string(&PermissionKind::Admin).unwrap(), "\"admin\"");
    }
}
