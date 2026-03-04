//! Transaction simulation and preflight preview for Monmouth.
//!
//! Provides an object-safe [`SimulationProvider`] trait for pluggable
//! simulation backends, plus a [`NoopSimulationProvider`] for use when
//! no backend is configured.

#![doc(issue_tracker_base_url = "https://github.com/MonmouthFND/monmouth-node/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

// Used by `monmouth-agent-types` re-exported types (serde derives, alloy types).
use alloy_primitives as _;
use serde as _;
use serde_json as _;
use tracing as _;

mod error;
pub use error::SimulationError;

mod service;
pub use service::{NoopSimulationProvider, SimulationProvider};
