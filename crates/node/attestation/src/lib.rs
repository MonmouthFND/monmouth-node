//! Cryptographic attestation registry for Monmouth.
//!
//! Stores and verifies attestations — cryptographic proofs that off-chain
//! computation occurred as claimed. Supports secp256k1 signature verification
//! natively, with extensible slots for Ed25519, TEE quotes, and ZK proofs.

#![doc(issue_tracker_base_url = "https://github.com/MonmouthFND/monmouth-node/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

// Used by `monmouth-agent-types` re-exported types (serde derives).
use serde as _;
use serde_json as _;

mod error;
pub use error::AttestationError;

mod verify;
pub use verify::verify_attestation;

mod registry;
pub use registry::{AttestationRegistry, DEFAULT_MAX_ATTESTATIONS};
