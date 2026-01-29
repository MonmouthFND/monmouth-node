//! Node wiring for the tokio runtime simulation.
//!
//! Each node runs:
//! - a marshal instance (block dissemination, backfill, and finalized block delivery), and
//! - a threshold-simplex engine instance that orders opaque digests.

pub(crate) mod config;
pub(crate) mod marshal;

pub(crate) use config::{ThresholdScheme, threshold_schemes};
