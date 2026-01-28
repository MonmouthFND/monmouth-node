//! REVM-based example chain driven by threshold-simplex.
//!
//! This example uses `alloy-evm` as the integration layer above `revm` and keeps the execution
//! backend generic over the database trait boundary (`Database` + `DatabaseCommit`).

pub mod application;
pub use application::execution::{
    CHAIN_ID, ExecutionOutcome, SEED_PRECOMPILE_ADDRESS_BYTES, evm_env, execute_txs,
    seed_precompile_address,
};

/// Consensus digest type alias.
pub type ConsensusDigest = commonware_cryptography::sha256::Digest;
/// Public key type alias.
pub type PublicKey = commonware_cryptography::ed25519::PublicKey;
pub(crate) type FinalizationEvent = (u32, ConsensusDigest);

pub mod domain;
pub use domain::{
    AccountChange, Block, BlockCfg, BlockId, BootstrapConfig, StateChanges, StateChangesCfg,
    StateRoot, Tx, TxCfg, TxId, block_id,
};

mod cli;
mod qmdb;
mod config;
mod outcome;
mod simulation;

fn main() {
    use clap::Parser;

    // Parse the cli.
    let cli = cli::Cli::parse();

    // Run the simulation.
    let outcome = cli.run();
    if outcome.is_err() {
        eprintln!("Simulation failed: {:?}", outcome);
        std::process::exit(1);
    };

    // Print the output.
    outcome.expect("success").print();
}
