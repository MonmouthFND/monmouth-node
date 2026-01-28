//! Contains the simulation config.

#[derive(Clone, Copy, Debug)]
/// Configuration for a simulation run.
pub struct SimConfig {
    /// Number of nodes participating in the simulation.
    pub nodes: usize,
    /// Number of blocks to finalize before stopping.
    pub blocks: u64,
    /// Seed used for deterministic randomness.
    pub seed: u64,
}
