//! Trigger-based conditional subscriptions for Monmouth agents.
//!
//! Provides a thread-safe [`SubscriptionRegistry`] that manages
//! [`ConditionalSubscription`]s. Agents subscribe to conditions
//! (balance thresholds, storage changes, events, block numbers, gas
//! prices) and receive notifications when those conditions are met.

#![doc(issue_tracker_base_url = "https://github.com/MonmouthFND/monmouth-node/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

// Used by `monmouth-agent-types` re-exported types (serde derives).
use serde as _;
use serde_json as _;

mod error;
pub use error::SubscriptionError;

mod registry;
pub use registry::{BlockContext, DEFAULT_MAX_SUBSCRIPTIONS, SubscriptionRegistry};
