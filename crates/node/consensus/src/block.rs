//! Kora block type using alloy types.

use alloy_consensus::Header;
use alloy_primitives::{B256, keccak256};
use alloy_rlp::Encodable;

/// Block type for Kora consensus.
///
/// Uses alloy types directly for Ethereum compatibility.
#[derive(Clone, Debug)]
pub struct KoraBlock {
    /// Block header.
    pub header: Header,
    /// Transactions in the block (encoded).
    pub transactions: Vec<Vec<u8>>,
    /// Computed state root.
    pub state_root: B256,
}

impl KoraBlock {
    /// Create a new block.
    pub const fn new(header: Header, transactions: Vec<Vec<u8>>, state_root: B256) -> Self {
        Self { header, transactions, state_root }
    }

    /// Compute the block's hash from the header.
    pub fn hash(&self) -> B256 {
        let mut buf = Vec::new();
        self.header.encode(&mut buf);
        keccak256(&buf)
    }

    /// Get the parent block's hash.
    pub const fn parent_hash(&self) -> B256 {
        self.header.parent_hash
    }

    /// Get the block height.
    pub const fn height(&self) -> u64 {
        self.header.number
    }

    /// Get the block timestamp.
    pub const fn timestamp(&self) -> u64 {
        self.header.timestamp
    }

    /// Get the number of transactions.
    pub const fn tx_count(&self) -> usize {
        self.transactions.len()
    }
}

impl Default for KoraBlock {
    fn default() -> Self {
        Self { header: Header::default(), transactions: Vec::new(), state_root: B256::ZERO }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_default() {
        let block = KoraBlock::default();
        assert_eq!(block.height(), 0);
        assert_eq!(block.tx_count(), 0);
        assert_eq!(block.state_root, B256::ZERO);
    }

    #[test]
    fn block_hash_deterministic() {
        let block = KoraBlock::default();
        let hash1 = block.hash();
        let hash2 = block.hash();
        assert_eq!(hash1, hash2);
    }
}
