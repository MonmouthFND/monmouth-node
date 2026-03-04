//! Thread-safe capability registry.

use std::{collections::HashMap, sync::Arc};

use parking_lot::RwLock;
use tracing::{debug, info};

use crate::{
    CapabilityError,
    types::{Capability, CapabilitySchema, CapabilitySummary},
};

/// Default maximum number of capabilities.
pub const DEFAULT_MAX_CAPABILITIES: usize = 1_000;

/// Thread-safe registry of capabilities.
///
/// Follows the same concurrency pattern as `FilterRegistry` — uses
/// `parking_lot::RwLock` for synchronous, non-async critical sections.
#[derive(Debug, Clone)]
pub struct CapabilityRegistry {
    inner: Arc<RwLock<HashMap<String, Capability>>>,
    max_capabilities: usize,
}

impl Default for CapabilityRegistry {
    fn default() -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
            max_capabilities: DEFAULT_MAX_CAPABILITIES,
        }
    }
}

impl CapabilityRegistry {
    /// Set the maximum number of capabilities.
    #[must_use]
    pub const fn with_max_capabilities(mut self, max: usize) -> Self {
        self.max_capabilities = max;
        self
    }

    /// Create a registry pre-loaded with capabilities (e.g., from config at startup).
    ///
    /// # Errors
    ///
    /// Returns an error if any capability has an invalid ID, if there are duplicates,
    /// or if the total exceeds the max capacity.
    pub fn from_capabilities(capabilities: Vec<Capability>) -> Result<Self, CapabilityError> {
        let registry = Self::default();
        for cap in capabilities {
            registry.register(cap)?;
        }
        Ok(registry)
    }

    /// Register a new capability.
    ///
    /// # Errors
    ///
    /// Returns an error if the ID is invalid, already exists, or the registry is at capacity.
    pub fn register(&self, capability: Capability) -> Result<(), CapabilityError> {
        validate_capability_id(&capability.id)?;

        let mut inner = self.inner.write();

        if inner.len() >= self.max_capabilities {
            return Err(CapabilityError::CapacityExceeded(self.max_capabilities));
        }

        if inner.contains_key(&capability.id) {
            return Err(CapabilityError::AlreadyExists(capability.id));
        }

        info!(id = %capability.id, name = %capability.name, "Registered capability");
        inner.insert(capability.id.clone(), capability);
        Ok(())
    }

    /// Get a capability by ID.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<Capability> {
        self.inner.read().get(id).cloned()
    }

    /// Get only the schema for a capability by ID.
    #[must_use]
    pub fn get_schema(&self, id: &str) -> Option<CapabilitySchema> {
        self.inner.read().get(id).map(|c| c.schema.clone())
    }

    /// List all capabilities as summaries, sorted by ID for determinism.
    #[must_use]
    pub fn list(&self) -> Vec<CapabilitySummary> {
        let inner = self.inner.read();
        let mut summaries: Vec<CapabilitySummary> =
            inner.values().map(CapabilitySummary::from).collect();
        summaries.sort_by(|a, b| a.id.cmp(&b.id));
        summaries
    }

    /// List only enabled capabilities as summaries, sorted by ID.
    #[must_use]
    pub fn list_enabled(&self) -> Vec<CapabilitySummary> {
        let inner = self.inner.read();
        let mut summaries: Vec<CapabilitySummary> = inner
            .values()
            .filter(|c| c.enabled)
            .map(CapabilitySummary::from)
            .collect();
        summaries.sort_by(|a, b| a.id.cmp(&b.id));
        summaries
    }

    /// Update an existing capability (full replacement).
    ///
    /// # Errors
    ///
    /// Returns an error if the capability does not exist.
    pub fn update(&self, capability: Capability) -> Result<(), CapabilityError> {
        validate_capability_id(&capability.id)?;

        let mut inner = self.inner.write();

        if !inner.contains_key(&capability.id) {
            return Err(CapabilityError::NotFound(capability.id));
        }

        debug!(id = %capability.id, "Updated capability");
        inner.insert(capability.id.clone(), capability);
        Ok(())
    }

    /// Remove a capability by ID, returning it.
    ///
    /// # Errors
    ///
    /// Returns an error if the capability does not exist.
    pub fn remove(&self, id: &str) -> Result<Capability, CapabilityError> {
        let mut inner = self.inner.write();

        inner
            .remove(id)
            .ok_or_else(|| CapabilityError::NotFound(id.to_string()))
            .inspect(|cap| {
                info!(id = %cap.id, "Removed capability");
            })
    }

    /// Returns the number of registered capabilities.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.read().len()
    }

    /// Returns `true` if the registry contains no capabilities.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.read().is_empty()
    }
}

/// Validate a capability ID: must be lowercase, dot-separated segments,
/// each segment alphanumeric with optional hyphens (no leading/trailing hyphens).
fn validate_capability_id(id: &str) -> Result<(), CapabilityError> {
    if id.is_empty() {
        return Err(CapabilityError::InvalidId("ID cannot be empty".to_string()));
    }

    for segment in id.split('.') {
        if segment.is_empty() {
            return Err(CapabilityError::InvalidId(format!(
                "empty segment in ID: {id}"
            )));
        }
        if segment.starts_with('-') || segment.ends_with('-') {
            return Err(CapabilityError::InvalidId(format!(
                "segment cannot start or end with hyphen: {id}"
            )));
        }
        if !segment.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
            return Err(CapabilityError::InvalidId(format!(
                "ID must be lowercase alphanumeric with hyphens and dots: {id}"
            )));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{CapabilitySchema, Permission, PermissionKind, RateLimit};

    fn test_capability(id: &str) -> Capability {
        Capability {
            id: id.to_string(),
            name: format!("Test {id}"),
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
            rate_limit: None,
            enabled: true,
        }
    }

    #[test]
    fn register_and_get() {
        let registry = CapabilityRegistry::default();
        let cap = test_capability("test.one");
        registry.register(cap.clone()).unwrap();

        let retrieved = registry.get("test.one").unwrap();
        assert_eq!(retrieved, cap);
    }

    #[test]
    fn get_nonexistent_returns_none() {
        let registry = CapabilityRegistry::default();
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn get_schema() {
        let registry = CapabilityRegistry::default();
        let cap = test_capability("test.schema");
        registry.register(cap.clone()).unwrap();

        let schema = registry.get_schema("test.schema").unwrap();
        assert_eq!(schema, cap.schema);
        assert!(registry.get_schema("missing").is_none());
    }

    #[test]
    fn list_sorted_by_id() {
        let registry = CapabilityRegistry::default();
        registry.register(test_capability("z.last")).unwrap();
        registry.register(test_capability("a.first")).unwrap();
        registry.register(test_capability("m.middle")).unwrap();

        let list = registry.list();
        assert_eq!(list.len(), 3);
        assert_eq!(list[0].id, "a.first");
        assert_eq!(list[1].id, "m.middle");
        assert_eq!(list[2].id, "z.last");
    }

    #[test]
    fn list_enabled_only() {
        let registry = CapabilityRegistry::default();
        registry.register(test_capability("enabled.one")).unwrap();

        let mut disabled = test_capability("disabled.one");
        disabled.enabled = false;
        registry.register(disabled).unwrap();

        let enabled = registry.list_enabled();
        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].id, "enabled.one");
    }

    #[test]
    fn update_existing() {
        let registry = CapabilityRegistry::default();
        let cap = test_capability("test.update");
        registry.register(cap).unwrap();

        let mut updated = test_capability("test.update");
        updated.version = "2.0.0".to_string();
        registry.update(updated).unwrap();

        let retrieved = registry.get("test.update").unwrap();
        assert_eq!(retrieved.version, "2.0.0");
    }

    #[test]
    fn update_nonexistent_fails() {
        let registry = CapabilityRegistry::default();
        let cap = test_capability("test.missing");
        let err = registry.update(cap).unwrap_err();
        assert!(matches!(err, CapabilityError::NotFound(_)));
    }

    #[test]
    fn remove_existing() {
        let registry = CapabilityRegistry::default();
        let cap = test_capability("test.remove");
        registry.register(cap.clone()).unwrap();

        let removed = registry.remove("test.remove").unwrap();
        assert_eq!(removed, cap);
        assert!(registry.get("test.remove").is_none());
    }

    #[test]
    fn remove_nonexistent_fails() {
        let registry = CapabilityRegistry::default();
        let err = registry.remove("missing").unwrap_err();
        assert!(matches!(err, CapabilityError::NotFound(_)));
    }

    #[test]
    fn duplicate_registration_fails() {
        let registry = CapabilityRegistry::default();
        registry.register(test_capability("test.dup")).unwrap();
        let err = registry.register(test_capability("test.dup")).unwrap_err();
        assert!(matches!(err, CapabilityError::AlreadyExists(_)));
    }

    #[test]
    fn capacity_exceeded() {
        let registry = CapabilityRegistry::default().with_max_capabilities(2);
        registry.register(test_capability("cap.one")).unwrap();
        registry.register(test_capability("cap.two")).unwrap();

        let err = registry.register(test_capability("cap.three")).unwrap_err();
        assert!(matches!(err, CapabilityError::CapacityExceeded(2)));
    }

    #[test]
    fn len_and_is_empty() {
        let registry = CapabilityRegistry::default();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);

        registry.register(test_capability("test.len")).unwrap();
        assert!(!registry.is_empty());
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn from_capabilities() {
        let caps = vec![test_capability("a.one"), test_capability("b.two")];
        let registry = CapabilityRegistry::from_capabilities(caps).unwrap();
        assert_eq!(registry.len(), 2);
    }

    #[test]
    fn from_capabilities_with_duplicates_fails() {
        let caps = vec![test_capability("test.dup"), test_capability("test.dup")];
        let err = CapabilityRegistry::from_capabilities(caps).unwrap_err();
        assert!(matches!(err, CapabilityError::AlreadyExists(_)));
    }

    #[test]
    fn valid_ids() {
        assert!(validate_capability_id("sim.preview").is_ok());
        assert!(validate_capability_id("agent-loop.delegation").is_ok());
        assert!(validate_capability_id("a").is_ok());
        assert!(validate_capability_id("a.b.c.d").is_ok());
        assert!(validate_capability_id("my-cap123").is_ok());
    }

    #[test]
    fn invalid_ids() {
        assert!(validate_capability_id("").is_err());
        assert!(validate_capability_id("Test.Cap").is_err());
        assert!(validate_capability_id("test..cap").is_err());
        assert!(validate_capability_id("-test.cap").is_err());
        assert!(validate_capability_id("test.cap-").is_err());
        assert!(validate_capability_id("test cap").is_err());
        assert!(validate_capability_id("test_cap").is_err());
    }

    #[test]
    fn thread_safety() {
        use std::sync::Arc;

        let registry = Arc::new(CapabilityRegistry::default());
        let mut handles = vec![];

        for i in 0..10 {
            let reg = Arc::clone(&registry);
            handles.push(std::thread::spawn(move || {
                let cap = test_capability(&format!("thread.cap-{i}"));
                reg.register(cap).unwrap();
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(registry.len(), 10);
    }

    #[test]
    fn rate_limit_on_capability() {
        let mut cap = test_capability("test.rated");
        cap.rate_limit = Some(RateLimit { max_requests: 50, window_secs: 30 });

        let registry = CapabilityRegistry::default();
        registry.register(cap).unwrap();

        let retrieved = registry.get("test.rated").unwrap();
        assert_eq!(retrieved.rate_limit.unwrap().max_requests, 50);
    }
}
