//! Configuration types for Monmouth nodes.
#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/MonmouthFND/monmouth-node/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod consensus;
pub use consensus::{ConsensusConfig, DEFAULT_THRESHOLD};

mod error;
pub use error::ConfigError;

mod execution;
pub use execution::{DEFAULT_BLOCK_TIME, DEFAULT_GAS_LIMIT, ExecutionConfig};

mod network;
pub use network::{DEFAULT_LISTEN_ADDR, NetworkConfig};

mod node;
pub use node::{DEFAULT_CHAIN_ID, DEFAULT_DATA_DIR, NodeConfig};

mod capabilities;
pub use capabilities::{
    CapabilitiesConfig, CapabilityDefinition, PermissionDef, RateLimitDef, SchemaDef,
};

mod rpc;
pub use rpc::{DEFAULT_HTTP_ADDR, DEFAULT_WS_ADDR, RpcConfig};

mod envelope;
pub use envelope::EnvelopeConfig;

mod simulation;
pub use simulation::SimulationConfig;

mod policy;
pub use policy::{PolicyConfig, PolicyRuleDef};

mod delegation;
pub use delegation::DelegationConfig;

mod intent_receipts;
pub use intent_receipts::IntentReceiptsConfig;

mod memory_anchoring;
pub use memory_anchoring::MemoryAnchoringConfig;

mod state_observation;
pub use state_observation::ObservationConfig;

mod coordination;
pub use coordination::CoordinationConfig;

mod conditional_subs;
pub use conditional_subs::ConditionalSubsConfig;

mod attestation;
pub use attestation::AttestationConfig;

mod svm;
pub use svm::SvmConfig;
