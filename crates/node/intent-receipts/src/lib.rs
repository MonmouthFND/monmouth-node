//! Intent receipt store for Monmouth transaction audit trails.
//!
//! Provides a thread-safe store for recording [`IntentReceipt`]s that pair
//! declared intent with actual execution outcome. Receipts are indexed by
//! transaction hash, agent, and block number so agents and auditors can
//! query the full history of what was intended versus what happened.

#![doc(issue_tracker_base_url = "https://github.com/MonmouthFND/monmouth-node/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

// Used by `monmouth-agent-types` re-exported types (serde derives).
use serde as _;
use serde_json as _;

mod error;
pub use error::IntentReceiptError;

mod registry;
pub use registry::{DEFAULT_MAX_RECEIPTS, IntentReceiptStore};
