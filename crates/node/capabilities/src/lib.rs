//! Capability registry for Monmouth agent-native modules.
//!
//! Provides a thread-safe registry of capabilities (tools/actions) that agents
//! can discover via RPC. Each capability has a typed schema, permissions, and
//! optional rate limits.

#![doc(issue_tracker_base_url = "https://github.com/MonmouthFND/monmouth-node/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod error;
pub use error::CapabilityError;

mod registry;
pub use registry::{CapabilityRegistry, DEFAULT_MAX_CAPABILITIES};

mod types;
pub use types::{
    Capability, CapabilitySchema, CapabilitySummary, Permission, PermissionKind, RateLimit,
};
