//! Agent-friendly structured state queries for Monmouth.
//!
//! Provides a [`StateProvider`] trait for pluggable state backends and a
//! [`StateObserver`] that executes [`StateQuery`] requests, returning typed
//! [`StateQueryResult`] values. Supports single and batch queries.

#![doc(issue_tracker_base_url = "https://github.com/MonmouthFND/monmouth-node/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

// Used by `monmouth-agent-types` re-exported types (serde derives).
use serde as _;
use serde_json as _;

mod error;
pub use error::StateObservationError;

mod provider;
pub use provider::{NoopStateProvider, StateProvider};

mod observer;
pub use observer::{DEFAULT_MAX_BATCH_SIZE, StateObserver};
