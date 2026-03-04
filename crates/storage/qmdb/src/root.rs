//! State root computation.

use alloy_primitives::{B256, keccak256};

const MONMOUTH_ROOT_NAMESPACE: &[u8] = b"_MONMOUTH_QMDB_ROOT";
const MONMOUTH_SVM_ROOT_NAMESPACE: &[u8] = b"_MONMOUTH_QMDB_SVM_ROOT";

/// State root computation utility.
#[derive(Debug, Clone, Copy)]
pub struct StateRoot;

impl StateRoot {
    /// Compute EVM state root from three partition roots.
    pub fn compute(accounts_root: B256, storage_root: B256, code_root: B256) -> B256 {
        let mut buf = Vec::with_capacity(MONMOUTH_ROOT_NAMESPACE.len() + 96);
        buf.extend_from_slice(MONMOUTH_ROOT_NAMESPACE);
        buf.extend_from_slice(accounts_root.as_slice());
        buf.extend_from_slice(storage_root.as_slice());
        buf.extend_from_slice(code_root.as_slice());
        keccak256(buf)
    }

    /// Compute SVM state root from the SVM accounts root.
    ///
    /// Uses a separate namespace (`_MONMOUTH_QMDB_SVM_ROOT`) so that
    /// SVM and EVM roots never collide even if given the same input bytes.
    pub fn compute_svm(accounts_root: B256) -> B256 {
        let mut buf = Vec::with_capacity(MONMOUTH_SVM_ROOT_NAMESPACE.len() + 32);
        buf.extend_from_slice(MONMOUTH_SVM_ROOT_NAMESPACE);
        buf.extend_from_slice(accounts_root.as_slice());
        keccak256(buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_root() {
        let a = B256::repeat_byte(0x11);
        let s = B256::repeat_byte(0x22);
        let c = B256::repeat_byte(0x33);

        let root1 = StateRoot::compute(a, s, c);
        let root2 = StateRoot::compute(a, s, c);
        assert_eq!(root1, root2);
    }

    #[test]
    fn different_inputs_different_root() {
        let root1 = StateRoot::compute(B256::ZERO, B256::ZERO, B256::ZERO);
        let root2 = StateRoot::compute(B256::repeat_byte(1), B256::ZERO, B256::ZERO);
        assert_ne!(root1, root2);
    }

    #[test]
    fn deterministic_svm_root() {
        let a = B256::repeat_byte(0x11);
        let root1 = StateRoot::compute_svm(a);
        let root2 = StateRoot::compute_svm(a);
        assert_eq!(root1, root2);
    }

    #[test]
    fn different_svm_inputs() {
        let root1 = StateRoot::compute_svm(B256::ZERO);
        let root2 = StateRoot::compute_svm(B256::repeat_byte(1));
        assert_ne!(root1, root2);
    }

    #[test]
    fn evm_and_svm_roots_differ() {
        // Even with the same input bytes, EVM and SVM roots must differ
        // because they use different namespace prefixes.
        let input = B256::repeat_byte(0x42);
        let evm_root = StateRoot::compute(input, B256::ZERO, B256::ZERO);
        let svm_root = StateRoot::compute_svm(input);
        assert_ne!(evm_root, svm_root);
    }
}
