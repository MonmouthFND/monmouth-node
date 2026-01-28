//! Deterministic demo scenario for the simulation.
//!
//! The example chain "prefunds" two addresses and injects a single transfer at height 1.

use alloy_primitives::{Address, U256};
use kora_domain::Tx;

use crate::tx::{address_from_key, receiver_key, sender_key, sign_eip1559_transfer};
#[derive(Clone, Debug)]
pub(super) struct DemoTransfer {
    pub(super) from: Address,
    pub(super) to: Address,
    pub(super) alloc: Vec<(Address, U256)>,
    pub(super) tx: Tx,
    pub(super) expected_from: U256,
    pub(super) expected_to: U256,
}

impl DemoTransfer {
    pub(super) fn new() -> Self {
        let sender = sender_key();
        let receiver = receiver_key();
        let from = address_from_key(&sender);
        let to = address_from_key(&receiver);
        let tx = sign_eip1559_transfer(&sender, to, U256::from(100u64), 0, 21_000);

        Self {
            from,
            to,
            alloc: vec![(from, U256::from(1_000_000u64)), (to, U256::ZERO)],
            tx,
            expected_from: U256::from(1_000_000u64 - 100),
            expected_to: U256::from(100u64),
        }
    }
}
