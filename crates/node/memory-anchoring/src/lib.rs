//! On-chain memory anchor registry for Monmouth agent context commitments.
//!
//! Provides a thread-safe registry for storing [`MemoryAnchor`]s -- on-chain
//! commitments to off-chain agent context. Each anchor records a content hash,
//! label, and auto-incrementing sequence number per agent, enabling agents to
//! prove the state of their off-chain memory at a given point in time.

#![doc(issue_tracker_base_url = "https://github.com/MonmouthFND/monmouth-node/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

// Used by `monmouth-agent-types` re-exported types (serde derives).
use serde as _;

mod error;
pub use error::MemoryAnchorError;

mod registry;
pub use registry::{DEFAULT_MAX_ANCHORS_PER_AGENT, MemoryAnchorRegistry};
