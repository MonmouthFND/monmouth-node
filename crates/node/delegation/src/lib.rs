//! Scoped delegation and session key management for Monmouth.
//!
//! Provides a thread-safe registry of delegation sessions that allow agents
//! to grant time-limited, capability-scoped permissions to delegate addresses.
//! Session grants are signed with secp256k1 and verified on creation.

#![doc(issue_tracker_base_url = "https://github.com/MonmouthFND/monmouth-node/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod error;
pub use error::DelegationError;

mod crypto;
pub use crypto::{sign_session_grant, verify_session_grant};

mod registry;
pub use registry::{DEFAULT_MAX_SESSIONS, DelegationRegistry, SessionInfo};
