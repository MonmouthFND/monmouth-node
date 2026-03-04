//! Verification utilities for Monmouth.
//!
//! Provides deterministic math primitives (Q8.24 fixed-point arithmetic,
//! vector dot products) for use in simulation, coordination, and attestation
//! modules. These are pure functions with no blockchain dependencies.

#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

pub mod fixed_point;
pub use fixed_point::{VerificationError, dot_product_q8_24};
