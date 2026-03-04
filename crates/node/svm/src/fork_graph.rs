//! Minimal ForkGraph implementation for Monmouth.
//!
//! Monmouth has linear block progression (no forks), so this is a trivial
//! implementation that satisfies the `TransactionBatchProcessor<FG>` bound.

use std::sync::RwLock;

use solana_program_runtime::loaded_programs::{BlockRelation, ForkGraph};

/// Trivial fork graph for Monmouth's linear chain.
///
/// Since Monmouth uses BFT Simplex consensus with no forks,
/// all slots on the same chain are treated as ancestor/descendant
/// based on height comparison.
#[derive(Debug, Default)]
pub struct MonmouthForkGraph {
    /// Current slot (block height).
    current_slot: RwLock<u64>,
}

impl MonmouthForkGraph {
    /// Create a new fork graph at the given slot.
    pub const fn new(slot: u64) -> Self {
        Self { current_slot: RwLock::new(slot) }
    }

    /// Update the current slot.
    pub fn set_slot(&self, slot: u64) {
        *self.current_slot.write().unwrap() = slot;
    }
}

impl ForkGraph for MonmouthForkGraph {
    fn relationship(&self, a: u64, b: u64) -> BlockRelation {
        match a.cmp(&b) {
            std::cmp::Ordering::Less => BlockRelation::Ancestor,
            std::cmp::Ordering::Equal => BlockRelation::Equal,
            std::cmp::Ordering::Greater => BlockRelation::Descendant,
        }
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_relationship() {
        let fg = MonmouthForkGraph::new(10);
        assert_eq!(fg.relationship(5, 10), BlockRelation::Ancestor);
        assert_eq!(fg.relationship(10, 10), BlockRelation::Equal);
        assert_eq!(fg.relationship(15, 10), BlockRelation::Descendant);
    }

    #[test]
    fn set_slot_works() {
        let fg = MonmouthForkGraph::new(0);
        fg.set_slot(42);
        assert_eq!(*fg.current_slot.read().unwrap(), 42);
    }
}
