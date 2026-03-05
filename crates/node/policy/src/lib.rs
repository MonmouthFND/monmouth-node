//! Policy engine for Monmouth agent permissions and rate limiting.
//!
//! Provides a thread-safe policy registry that evaluates agent actions against
//! configurable rules including spending caps, rate limits, and explicit
//! allow/deny/require-confirmation decisions.

#![doc(issue_tracker_base_url = "https://github.com/MonmouthFND/monmouth-node/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

// Used by `monmouth-agent-types` re-exported types (serde derives).
use serde as _;

mod error;
pub use error::PolicyError;

mod registry;
pub use registry::{DEFAULT_MAX_RULES, PolicyRegistry};
