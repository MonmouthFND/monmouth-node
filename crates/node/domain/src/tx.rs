//! Transactions

use alloy_primitives::{Bytes, keccak256};
use bytes::{Buf, BufMut};
use commonware_codec::{Encode, EncodeSize, Error as CodecError, RangeCfg, Read, Write};

use super::TxId;

#[derive(Clone, Copy, Debug)]
/// Configuration used when decoding transactions from bytes.
pub struct TxCfg {
    /// Maximum encoded transaction size accepted by the codec.
    pub max_tx_bytes: usize,
}

/// Raw transaction bytes for the example.
///
/// This is expected to contain a signed Ethereum transaction envelope.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tx {
    /// Encoded transaction bytes.
    pub bytes: Bytes,
}

impl Tx {
    /// Compute the transaction identifier from its encoded contents.
    pub fn id(&self) -> TxId {
        TxId(keccak256(self.encode()))
    }

    /// Create a new transaction from encoded bytes.
    pub const fn new(bytes: Bytes) -> Self {
        Self { bytes }
    }
}

impl Write for Tx {
    fn write(&self, buf: &mut impl BufMut) {
        self.bytes.as_ref().write(buf);
    }
}

impl EncodeSize for Tx {
    fn encode_size(&self) -> usize {
        self.bytes.as_ref().encode_size()
    }
}

impl Read for Tx {
    type Cfg = TxCfg;

    fn read_cfg(buf: &mut impl Buf, cfg: &Self::Cfg) -> Result<Self, CodecError> {
        let data = Vec::<u8>::read_cfg(buf, &(RangeCfg::new(0..=cfg.max_tx_bytes), ()))?;
        Ok(Self { bytes: Bytes::from(data) })
    }
}
