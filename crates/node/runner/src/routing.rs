//! Transaction routing by VM target.
//!
//! Inspects raw transaction bytes to determine whether they should be
//! executed by the EVM or SVM. Transactions wrapped in a Monmouth
//! envelope (`0x4d` magic byte) with `VmTarget::Svm` are routed to
//! the SVM; everything else goes to the EVM.

use alloy_primitives::Bytes;
use monmouth_domain::Tx;
use monmouth_envelope::{decode_agent_envelope, is_monmouth_envelope};

/// Partitioned transaction batches for dual-VM execution.
#[derive(Debug, Default)]
pub struct PartitionedTxs {
    /// Transactions destined for the EVM (including their index in the original batch).
    pub evm: Vec<(usize, Tx)>,
    /// Transactions destined for the SVM, with their inner (unwrapped) bytes.
    pub svm: Vec<(usize, Bytes)>,
}

/// Partition a batch of transactions by VM target.
///
/// - Transactions with the Monmouth magic byte (`0x4d`) are decoded as
///   agent envelopes. If `vm_target == Svm`, the inner transaction bytes
///   are placed in the SVM batch.
/// - All other transactions (plain EIP-2718 or EVM envelopes) go to EVM.
///
/// Returns the partitioned batches preserving original ordering indices.
pub fn partition_by_vm_target(txs: &[Tx]) -> PartitionedTxs {
    let mut result = PartitionedTxs::default();

    for (i, tx) in txs.iter().enumerate() {
        if is_monmouth_envelope(&tx.bytes) {
            match decode_agent_envelope(&tx.bytes) {
                Ok(envelope) => match envelope.vm_target {
                    monmouth_agent_types::VmTarget::Svm => {
                            result.svm.push((i, Bytes::from(envelope.inner_tx)));
                        }
                        monmouth_agent_types::VmTarget::Evm => {
                        // EVM-targeted envelope — pass original bytes to EVM.
                        result.evm.push((i, tx.clone()));
                    }
                }
                Err(_) => {
                    // Failed to decode envelope — treat as EVM tx.
                    result.evm.push((i, tx.clone()));
                }
            }
        } else {
            // No magic byte — standard EVM transaction.
            result.evm.push((i, tx.clone()));
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use monmouth_agent_types::VmTarget;
    use monmouth_envelope::encode_agent_envelope;

    use super::*;

    fn plain_evm_tx() -> Tx {
        Tx::new(Bytes::from_static(&[0x02, 0xf8, 0x70]))
    }

    fn svm_envelope_tx() -> Tx {
        let envelope = monmouth_envelope::AgentTxEnvelope {
            vm_target: VmTarget::Svm,
            module_hint: None,
            session_id: None,
            intent: None,
            inner_tx: vec![0xaa, 0xbb, 0xcc],
            raw: Vec::new(),
        };
        Tx::new(Bytes::from(encode_agent_envelope(&envelope).unwrap()))
    }

    fn evm_envelope_tx() -> Tx {
        let envelope = monmouth_envelope::AgentTxEnvelope {
            vm_target: VmTarget::Evm,
            module_hint: None,
            session_id: None,
            intent: None,
            inner_tx: vec![0x02, 0xf8],
            raw: Vec::new(),
        };
        Tx::new(Bytes::from(encode_agent_envelope(&envelope).unwrap()))
    }

    #[test]
    fn empty_batch() {
        let result = partition_by_vm_target(&[]);
        assert!(result.evm.is_empty());
        assert!(result.svm.is_empty());
    }

    #[test]
    fn all_evm_plain() {
        let txs = vec![plain_evm_tx(), plain_evm_tx()];
        let result = partition_by_vm_target(&txs);
        assert_eq!(result.evm.len(), 2);
        assert!(result.svm.is_empty());
    }

    #[test]
    fn svm_envelope_routes_to_svm() {
        let txs = vec![svm_envelope_tx()];
        let result = partition_by_vm_target(&txs);
        assert!(result.evm.is_empty());
        assert_eq!(result.svm.len(), 1);
        assert_eq!(result.svm[0].1.as_ref(), &[0xaa, 0xbb, 0xcc]);
    }

    #[test]
    fn evm_envelope_routes_to_evm() {
        let txs = vec![evm_envelope_tx()];
        let result = partition_by_vm_target(&txs);
        assert_eq!(result.evm.len(), 1);
        assert!(result.svm.is_empty());
    }

    #[test]
    fn mixed_batch_preserves_indices() {
        let txs = vec![plain_evm_tx(), svm_envelope_tx(), plain_evm_tx(), svm_envelope_tx()];
        let result = partition_by_vm_target(&txs);

        assert_eq!(result.evm.len(), 2);
        assert_eq!(result.evm[0].0, 0);
        assert_eq!(result.evm[1].0, 2);

        assert_eq!(result.svm.len(), 2);
        assert_eq!(result.svm[0].0, 1);
        assert_eq!(result.svm[1].0, 3);
    }

    #[test]
    fn malformed_envelope_falls_back_to_evm() {
        // Starts with magic byte but isn't valid envelope.
        let tx = Tx::new(Bytes::from_static(&[0x4d, 0x00]));
        let result = partition_by_vm_target(&[tx]);
        assert_eq!(result.evm.len(), 1);
        assert!(result.svm.is_empty());
    }

    #[test]
    fn all_svm_batch() {
        let txs = vec![svm_envelope_tx(), svm_envelope_tx(), svm_envelope_tx()];
        let result = partition_by_vm_target(&txs);
        assert!(result.evm.is_empty());
        assert_eq!(result.svm.len(), 3);
        assert_eq!(result.svm[0].0, 0);
        assert_eq!(result.svm[1].0, 1);
        assert_eq!(result.svm[2].0, 2);
    }

    #[test]
    fn large_mixed_batch() {
        let txs: Vec<Tx> = (0..100)
            .map(|i| {
                if i % 2 == 0 {
                    plain_evm_tx()
                } else {
                    svm_envelope_tx()
                }
            })
            .collect();
        let result = partition_by_vm_target(&txs);
        assert_eq!(result.evm.len(), 50);
        assert_eq!(result.svm.len(), 50);
        for (idx, (orig_idx, _)) in result.evm.iter().enumerate() {
            assert_eq!(*orig_idx, idx * 2);
        }
        for (idx, (orig_idx, _)) in result.svm.iter().enumerate() {
            assert_eq!(*orig_idx, idx * 2 + 1);
        }
    }

    #[test]
    fn double_wrapped_envelope_treated_as_evm() {
        // Inner envelope targeting SVM
        let inner_bytes = encode_agent_envelope(&monmouth_envelope::AgentTxEnvelope {
            vm_target: VmTarget::Svm,
            module_hint: None,
            session_id: None,
            intent: None,
            inner_tx: vec![0xaa, 0xbb],
            raw: Vec::new(),
        }).unwrap();
        // Outer envelope targeting EVM wrapping the inner envelope
        let outer = monmouth_envelope::AgentTxEnvelope {
            vm_target: VmTarget::Evm,
            module_hint: None,
            session_id: None,
            intent: None,
            inner_tx: inner_bytes,
            raw: Vec::new(),
        };
        let tx = Tx::new(Bytes::from(encode_agent_envelope(&outer).unwrap()));
        let result = partition_by_vm_target(&[tx]);
        // Outer says EVM, so it goes to EVM regardless of inner
        assert_eq!(result.evm.len(), 1);
        assert!(result.svm.is_empty());
    }
}
