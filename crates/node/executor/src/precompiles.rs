//! Custom precompiles for Monmouth agent-native operations.
//!
//! Extends the standard Ethereum precompiles with agent-specific operations
//! at well-known addresses.

use alloy_primitives::{Address, Bytes};
use revm::{
    context::{Cfg, LocalContextTr},
    context_interface::ContextTr,
    handler::{EthPrecompiles, PrecompileProvider},
    interpreter::{CallInput, CallInputs, Gas, InstructionResult, InterpreterResult},
    primitives::hardfork::SpecId,
};

use crate::classifier::precompiles as addrs;

/// Gas costs for custom precompile operations.
mod gas {
    /// Base gas for AI inference stub.
    pub(super) const AI_INFERENCE_BASE: u64 = 10_000;
    /// Base gas for vector similarity stub.
    pub(super) const VECTOR_SIMILARITY_BASE: u64 = 5_000;
    /// Base gas for intent parser stub.
    pub(super) const INTENT_PARSER_BASE: u64 = 5_000;
    /// Base gas for SVM router stub.
    pub(super) const SVM_ROUTER_BASE: u64 = 10_000;
    /// Base gas for cross-chain message passer.
    pub(super) const CROSS_CHAIN_MESSAGE_PASSER_BASE: u64 = 20_000;
}

/// Custom precompile provider for Monmouth that extends standard Ethereum precompiles.
#[derive(Debug, Clone)]
pub struct MonmouthPrecompiles {
    /// Standard Ethereum precompiles.
    inner: EthPrecompiles,
}

impl MonmouthPrecompiles {
    /// All custom precompile addresses.
    const CUSTOM_ADDRESSES: [Address; 5] = [
        addrs::AI_INFERENCE,
        addrs::VECTOR_SIMILARITY,
        addrs::INTENT_PARSER,
        addrs::SVM_ROUTER,
        addrs::CROSS_CHAIN_MESSAGE_PASSER,
    ];

    /// Create a new Monmouth precompile provider with the given spec.
    pub fn new(spec: SpecId) -> Self {
        Self { inner: EthPrecompiles::new(spec) }
    }

    /// Check if an address is a custom Monmouth precompile.
    fn is_custom(address: &Address) -> bool {
        Self::CUSTOM_ADDRESSES.contains(address)
    }

    /// Check if an address is any recognized precompile (custom or standard).
    pub fn contains_address(&self, address: &Address) -> bool {
        Self::is_custom(address) || self.inner.contains(address)
    }

    /// Get all warm addresses (custom + standard).
    pub fn all_warm_addresses(&self) -> impl Iterator<Item = Address> {
        let eth_addrs: Vec<Address> = self.inner.warm_addresses().collect();
        let custom_addrs = Self::CUSTOM_ADDRESSES.to_vec();
        eth_addrs.into_iter().chain(custom_addrs)
    }

    /// Execute a custom precompile.
    fn execute_custom(address: &Address, input: &[u8], gas_limit: u64) -> InterpreterResult {
        let (base_gas, output) = if *address == addrs::AI_INFERENCE {
            (gas::AI_INFERENCE_BASE, execute_ai_inference(input))
        } else if *address == addrs::VECTOR_SIMILARITY {
            (gas::VECTOR_SIMILARITY_BASE, execute_vector_similarity(input))
        } else if *address == addrs::INTENT_PARSER {
            (gas::INTENT_PARSER_BASE, execute_intent_parser(input))
        } else if *address == addrs::SVM_ROUTER {
            (gas::SVM_ROUTER_BASE, execute_svm_router(input))
        } else if *address == addrs::CROSS_CHAIN_MESSAGE_PASSER {
            (gas::CROSS_CHAIN_MESSAGE_PASSER_BASE, execute_cross_chain_message_passer(input))
        } else {
            // Should not reach here due to is_custom check
            return InterpreterResult {
                result: InstructionResult::PrecompileError,
                gas: Gas::new(gas_limit),
                output: Bytes::new(),
            };
        };

        let mut gas = Gas::new(gas_limit);
        if !gas.record_cost(base_gas) {
            tracing::debug!(address = %address, required = base_gas, limit = gas_limit, "precompile out of gas");
            return InterpreterResult {
                result: InstructionResult::PrecompileOOG,
                gas,
                output: Bytes::new(),
            };
        }

        tracing::debug!(
            address = %address,
            input_len = input.len(),
            gas_used = base_gas,
            output_len = output.len(),
            "custom precompile executed"
        );

        InterpreterResult { result: InstructionResult::Return, gas, output }
    }
}

impl<CTX: ContextTr> PrecompileProvider<CTX> for MonmouthPrecompiles {
    type Output = InterpreterResult;

    fn set_spec(&mut self, spec: <CTX::Cfg as Cfg>::Spec) -> bool {
        <EthPrecompiles as PrecompileProvider<CTX>>::set_spec(&mut self.inner, spec)
    }

    fn run(
        &mut self,
        context: &mut CTX,
        inputs: &CallInputs,
    ) -> Result<Option<InterpreterResult>, String> {
        // Check custom precompiles first
        if Self::is_custom(&inputs.bytecode_address) {
            let input_bytes: Vec<u8> = match &inputs.input {
                CallInput::SharedBuffer(range) => {
                    LocalContextTr::shared_memory_buffer_slice(context.local(), range.clone())
                        .map_or_else(Vec::new, |slice| slice.to_vec())
                }
                CallInput::Bytes(bytes) => bytes.0.to_vec(),
            };
            let result =
                Self::execute_custom(&inputs.bytecode_address, &input_bytes, inputs.gas_limit);
            return Ok(Some(result));
        }

        // Delegate to standard Ethereum precompiles
        <EthPrecompiles as PrecompileProvider<CTX>>::run(&mut self.inner, context, inputs)
    }

    fn warm_addresses(&self) -> Box<impl Iterator<Item = Address>> {
        let eth_addrs: Vec<Address> = self.inner.warm_addresses().collect();
        let custom_addrs = Self::CUSTOM_ADDRESSES.to_vec();
        Box::new(eth_addrs.into_iter().chain(custom_addrs))
    }

    fn contains(&self, address: &Address) -> bool {
        Self::is_custom(address) || self.inner.contains(address)
    }
}

// --- Stub implementations ---

/// AI Inference precompile (0x1000).
/// Accepts input data and returns a mock inference result.
fn execute_ai_inference(input: &[u8]) -> Bytes {
    tracing::info!(input_len = input.len(), "AI Inference precompile called");
    // Return ABI-encoded mock response: (bool success, bytes result)
    // For now, return a simple success indicator with input hash
    let mut output = Vec::with_capacity(64);
    // success = true (padded to 32 bytes)
    output.extend_from_slice(&[0u8; 31]);
    output.push(1);
    // result offset
    output.extend_from_slice(&[0u8; 31]);
    output.push(0x40);
    Bytes::from(output)
}

/// Vector Similarity precompile (0x1001).
/// Semantic search stub.
fn execute_vector_similarity(input: &[u8]) -> Bytes {
    tracing::info!(input_len = input.len(), "Vector Similarity precompile called");
    // Return mock similarity score: uint256 score (0.85 scaled to 1e18)
    let mut output = [0u8; 32];
    // 0.85 * 1e18 = 850000000000000000 = 0x0BC8D3F7B3340000
    output[24..32].copy_from_slice(&850_000_000_000_000_000u64.to_be_bytes());
    Bytes::from(output.to_vec())
}

/// Intent Parser precompile (0x1002).
/// Natural language → structured intent stub.
fn execute_intent_parser(input: &[u8]) -> Bytes {
    tracing::info!(input_len = input.len(), "Intent Parser precompile called");
    // Return mock parsed intent: (uint8 intentType, address target, uint256 value)
    let mut output = Vec::with_capacity(96);
    // intentType = 1 (transfer)
    output.extend_from_slice(&[0u8; 31]);
    output.push(1);
    // target = zero address
    output.extend_from_slice(&[0u8; 32]);
    // value = 0
    output.extend_from_slice(&[0u8; 32]);
    Bytes::from(output)
}

/// SVM Router precompile (0x1003).
/// Solana program execution routing stub.
fn execute_svm_router(input: &[u8]) -> Bytes {
    tracing::info!(input_len = input.len(), "SVM Router precompile called");
    // Return success acknowledgment: (bool success, bytes32 txHash)
    let mut output = Vec::with_capacity(64);
    // success = true
    output.extend_from_slice(&[0u8; 31]);
    output.push(1);
    // mock tx hash (all zeros)
    output.extend_from_slice(&[0u8; 32]);
    Bytes::from(output)
}

/// Cross-Chain Message Passer precompile (0x4200).
/// Cross-chain deposit/withdrawal message passing.
fn execute_cross_chain_message_passer(input: &[u8]) -> Bytes {
    tracing::info!(input_len = input.len(), "Cross-Chain Message Passer precompile called");
    // Return message nonce: uint256 nonce
    let mut output = [0u8; 32];
    // nonce = 1 (first message)
    output[31] = 1;
    Bytes::from(output.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_addresses_recognized() {
        assert!(MonmouthPrecompiles::is_custom(&addrs::AI_INFERENCE));
        assert!(MonmouthPrecompiles::is_custom(&addrs::VECTOR_SIMILARITY));
        assert!(MonmouthPrecompiles::is_custom(&addrs::INTENT_PARSER));
        assert!(MonmouthPrecompiles::is_custom(&addrs::SVM_ROUTER));
        assert!(MonmouthPrecompiles::is_custom(&addrs::CROSS_CHAIN_MESSAGE_PASSER));
    }

    #[test]
    fn standard_addresses_not_custom() {
        assert!(!MonmouthPrecompiles::is_custom(&Address::ZERO));
        assert!(!MonmouthPrecompiles::is_custom(&Address::with_last_byte(1))); // ecrecover
    }

    #[test]
    fn contains_both_custom_and_standard() {
        let precompiles = MonmouthPrecompiles::new(SpecId::PRAGUE);
        // Custom
        assert!(precompiles.contains_address(&addrs::AI_INFERENCE));
        assert!(precompiles.contains_address(&addrs::SVM_ROUTER));
        assert!(precompiles.contains_address(&addrs::CROSS_CHAIN_MESSAGE_PASSER));
        // Standard ecrecover at 0x01
        assert!(precompiles.contains_address(&Address::with_last_byte(1)));
        // Unknown
        assert!(!precompiles.contains_address(&Address::with_last_byte(0xff)));
    }

    #[test]
    fn warm_addresses_include_custom() {
        let precompiles = MonmouthPrecompiles::new(SpecId::PRAGUE);
        let warm: Vec<Address> = precompiles.all_warm_addresses().collect();
        for addr in &MonmouthPrecompiles::CUSTOM_ADDRESSES {
            assert!(warm.contains(addr), "missing custom address {addr}");
        }
        // Also includes standard ecrecover
        assert!(warm.contains(&Address::with_last_byte(1)));
    }

    #[test]
    fn ai_inference_returns_data() {
        let output = execute_ai_inference(&[0x01, 0x02, 0x03]);
        assert!(!output.is_empty());
        assert_eq!(output.len(), 64);
        // First 32 bytes: success = true
        assert_eq!(output[31], 1);
    }

    #[test]
    fn vector_similarity_returns_score() {
        let output = execute_vector_similarity(&[]);
        assert_eq!(output.len(), 32);
    }

    #[test]
    fn intent_parser_returns_intent() {
        let output = execute_intent_parser(&[0xde, 0xad]);
        assert_eq!(output.len(), 96);
        assert_eq!(output[31], 1); // intentType = 1
    }

    #[test]
    fn svm_router_returns_success() {
        let output = execute_svm_router(&[]);
        assert_eq!(output.len(), 64);
        assert_eq!(output[31], 1); // success = true
    }

    #[test]
    fn cross_chain_message_passer_returns_nonce() {
        let output = execute_cross_chain_message_passer(&[0x01]);
        assert_eq!(output.len(), 32);
        assert_eq!(output[31], 1); // nonce = 1
    }

    #[test]
    fn execute_custom_out_of_gas() {
        // AI inference needs 10000 gas, give it only 100
        let result = MonmouthPrecompiles::execute_custom(&addrs::AI_INFERENCE, &[], 100);
        assert_eq!(result.result, InstructionResult::PrecompileOOG);
    }

    #[test]
    fn execute_custom_sufficient_gas() {
        let result = MonmouthPrecompiles::execute_custom(&addrs::AI_INFERENCE, &[], 100_000);
        assert_eq!(result.result, InstructionResult::Return);
        assert!(!result.output.is_empty());
    }
}
