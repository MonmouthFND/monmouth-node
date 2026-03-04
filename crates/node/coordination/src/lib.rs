//! Multi-agent coordination, job lifecycle, and escrow for Monmouth.
//!
//! Provides a thread-safe [`CoordinationRegistry`] that manages [`Job`]
//! proposals, acceptance, execution, completion, dispute, settlement,
//! and cancellation. Escrow entries track funds locked for active jobs.

#![doc(issue_tracker_base_url = "https://github.com/MonmouthFND/monmouth-node/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

// Used by `monmouth-agent-types` re-exported types (serde derives).
use serde as _;
use serde_json as _;

mod error;
pub use error::CoordinationError;

mod registry;
pub use registry::{CoordinationRegistry, DEFAULT_MAX_JOBS};
