//! Contains the simulation outcome.

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::Duration,
};

use alloy_evm::revm::primitives::{B256, U256};
use anyhow::Context as _;
use commonware_cryptography::ed25519;
use commonware_p2p::{Manager as _, simulated};
use commonware_runtime::{Metrics as _, Runner as _, tokio};
use commonware_utils::{TryCollect as _, ordered::Set};
use futures::{StreamExt as _, channel::mpsc};

use crate::{
    BootstrapConfig, ConsensusDigest, FinalizationEvent,
    application::{
        NodeEnvironment, ThresholdScheme, TransportControl, start_node, threshold_schemes,
    },
};

#[derive(Clone, Copy, Debug)]
/// Summary of a completed simulation run.
pub struct SimOutcome {
    /// Finalized head digest (the value ordered by threshold-simplex).
    pub head: ConsensusDigest,
    /// State commitment at the head digest.
    pub state_root: crate::StateRoot,
    /// Latest tracked threshold-simplex seed hash (used as `prevrandao`).
    pub seed: B256,
    /// Final balance of the sender account after the demo transfer.
    pub from_balance: U256,
    /// Final balance of the receiver account after the demo transfer.
    pub to_balance: U256,
}

impl SimOutcome {
    /// Prints out the simulation outcome.
    pub fn print(self) {
        println!("finalized head: {:?}", self.head);
        println!("state root: {:?}", self.state_root);
        println!("seed: {:?}", self.seed);
        println!("from balance: {:?}", self.from_balance);
        println!("to balance: {:?}", self.to_balance);
    }
}
