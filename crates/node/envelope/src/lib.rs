//! Agent transaction envelope codec for Monmouth.
//!
//! Provides encoding/decoding for extended Monmouth transaction envelopes
//! that include VM routing, session delegation, and intent declarations
//! alongside standard EIP-2718 transactions.

#![doc(issue_tracker_base_url = "https://github.com/MonmouthFND/monmouth-node/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod error;
pub use error::EnvelopeError;

mod types;
pub use types::AgentTxEnvelope;

mod codec;
pub use codec::{
    MONMOUTH_MAGIC, decode_agent_envelope, encode_agent_envelope, is_monmouth_envelope,
};
