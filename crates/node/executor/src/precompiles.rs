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
    /// Base gas for vector similarity: 200 + dimensions * 10.
    pub(super) const VECTOR_SIMILARITY_BASE: u64 = 200;
    /// Per-dimension gas for vector similarity.
    pub(super) const VECTOR_SIMILARITY_PER_DIM: u64 = 10;
    /// Base gas for intent parser stub.
    pub(super) const INTENT_PARSER_BASE: u64 = 5_000;
    /// Base gas for SVM router stub.
    pub(super) const SVM_ROUTER_BASE: u64 = 10_000;
    /// Base gas for cross-chain message passer.
    pub(super) const CROSS_CHAIN_MESSAGE_PASSER_BASE: u64 = 20_000;
    /// Maximum vector dimensions (DoS protection).
    pub(super) const MAX_VECTOR_DIMENSIONS: u32 = 2048;
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
        // Vector similarity has dynamic gas based on dimensions — handle separately.
        if *address == addrs::VECTOR_SIMILARITY {
            return execute_vector_similarity(input, gas_limit);
        }

        let (base_gas, output) = if *address == addrs::AI_INFERENCE {
            (gas::AI_INFERENCE_BASE, execute_ai_inference(input))
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
///
/// Computes the dot product of two fixed-point Q8.24 integer vectors.
/// For unit-normalized vectors, dot product equals cosine similarity.
///
/// Input layout (tight-packed):
///   [0..4]              uint32 dimensions N (big-endian, max 2048)
///   [4..4+N*4]          vector A as N x int32 (Q8.24 fixed-point, big-endian)
///   [4+N*4..4+2*N*4]    vector B as N x int32 (Q8.24 fixed-point, big-endian)
///
/// Output:
///   [0..32]             int256 dot product (Q16.48 scaled result, sign-extended)
///
/// Gas: 200 + N * 10
fn execute_vector_similarity(input: &[u8], gas_limit: u64) -> InterpreterResult {
    // Need at least 4 bytes for dimension count
    if input.len() < 4 {
        tracing::debug!("vector similarity: input too short for dimension count");
        return InterpreterResult {
            result: InstructionResult::PrecompileError,
            gas: Gas::new(gas_limit),
            output: Bytes::new(),
        };
    }

    let dimensions =
        u32::from_be_bytes([input[0], input[1], input[2], input[3]]);

    // Validate dimensions
    if dimensions == 0 || dimensions > gas::MAX_VECTOR_DIMENSIONS {
        tracing::debug!(dimensions, max = gas::MAX_VECTOR_DIMENSIONS, "vector similarity: invalid dimensions");
        return InterpreterResult {
            result: InstructionResult::PrecompileError,
            gas: Gas::new(gas_limit),
            output: Bytes::new(),
        };
    }

    let n = dimensions as usize;
    let expected_len = 4 + n * 4 * 2; // 4 bytes header + 2 vectors of N int32s
    if input.len() < expected_len {
        tracing::debug!(input_len = input.len(), expected = expected_len, "vector similarity: input too short");
        return InterpreterResult {
            result: InstructionResult::PrecompileError,
            gas: Gas::new(gas_limit),
            output: Bytes::new(),
        };
    }

    // Calculate and charge gas: BASE + N * PER_DIM
    let total_gas = gas::VECTOR_SIMILARITY_BASE + (dimensions as u64) * gas::VECTOR_SIMILARITY_PER_DIM;
    let mut gas = Gas::new(gas_limit);
    if !gas.record_cost(total_gas) {
        tracing::debug!(required = total_gas, limit = gas_limit, "vector similarity: out of gas");
        return InterpreterResult {
            result: InstructionResult::PrecompileOOG,
            gas,
            output: Bytes::new(),
        };
    }

    // Compute dot product using i64 accumulator to avoid overflow.
    // Each element is i32 (Q8.24), so each product is i64 (Q16.48).
    // Summing up to 2048 i64 values fits in i64 (max sum ~2048 * 2^62 < 2^73,
    // but Q8.24 elements are bounded to ~[-128, 128) so products are bounded
    // to ~2^14 * 2^48 = 2^62, and 2048 of those is ~2^73 which overflows i64).
    // Use i128 accumulator for safety.
    let mut dot_product: i128 = 0;
    let vec_a_start = 4;
    let vec_b_start = 4 + n * 4;

    for i in 0..n {
        let a_offset = vec_a_start + i * 4;
        let b_offset = vec_b_start + i * 4;

        let a = i32::from_be_bytes([
            input[a_offset],
            input[a_offset + 1],
            input[a_offset + 2],
            input[a_offset + 3],
        ]);
        let b = i32::from_be_bytes([
            input[b_offset],
            input[b_offset + 1],
            input[b_offset + 2],
            input[b_offset + 3],
        ]);

        dot_product += (a as i64 as i128) * (b as i64 as i128);
    }

    // Encode as int256 (sign-extended to 32 bytes, big-endian)
    let mut output = if dot_product < 0 { [0xffu8; 32] } else { [0u8; 32] };
    let bytes = dot_product.to_be_bytes(); // 16 bytes
    output[16..32].copy_from_slice(&bytes);

    tracing::debug!(
        dimensions,
        gas_used = total_gas,
        "vector similarity computed"
    );

    InterpreterResult { result: InstructionResult::Return, gas, output: Bytes::from(output.to_vec()) }
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
    fn vector_similarity_simple_dot_product() {
        // 2 dimensions, vectors [1.0, 0.5] dot [0.5, 1.0] in Q8.24
        // 1.0 in Q8.24 = 1 << 24 = 16777216 = 0x01000000
        // 0.5 in Q8.24 = 1 << 23 = 8388608  = 0x00800000
        let one_q24: i32 = 1 << 24;
        let half_q24: i32 = 1 << 23;
        let mut input = Vec::new();
        input.extend_from_slice(&2u32.to_be_bytes()); // dims = 2
        input.extend_from_slice(&one_q24.to_be_bytes()); // A[0] = 1.0
        input.extend_from_slice(&half_q24.to_be_bytes()); // A[1] = 0.5
        input.extend_from_slice(&half_q24.to_be_bytes()); // B[0] = 0.5
        input.extend_from_slice(&one_q24.to_be_bytes()); // B[1] = 1.0

        let result = execute_vector_similarity(&input, 100_000);
        assert_eq!(result.result, InstructionResult::Return);
        assert_eq!(result.output.len(), 32);

        // dot = 1.0*0.5 + 0.5*1.0 = 1.0 in real, which in Q16.48 is:
        // (1<<24)*(1<<23) + (1<<23)*(1<<24) = 2 * (1<<47) = 1<<48
        // That's 1.0 in Q16.48 format
        let expected: i128 = (one_q24 as i128) * (half_q24 as i128)
            + (half_q24 as i128) * (one_q24 as i128);
        let mut expected_bytes = [0u8; 32];
        let be = expected.to_be_bytes();
        expected_bytes[16..32].copy_from_slice(&be);
        assert_eq!(&result.output[..], &expected_bytes[..]);
    }

    #[test]
    fn vector_similarity_rejects_empty_input() {
        let result = execute_vector_similarity(&[], 100_000);
        assert_eq!(result.result, InstructionResult::PrecompileError);
    }

    #[test]
    fn vector_similarity_rejects_zero_dimensions() {
        let mut input = Vec::new();
        input.extend_from_slice(&0u32.to_be_bytes());
        let result = execute_vector_similarity(&input, 100_000);
        assert_eq!(result.result, InstructionResult::PrecompileError);
    }

    #[test]
    fn vector_similarity_rejects_excessive_dimensions() {
        let mut input = Vec::new();
        input.extend_from_slice(&3000u32.to_be_bytes()); // > MAX_VECTOR_DIMENSIONS
        let result = execute_vector_similarity(&input, 100_000);
        assert_eq!(result.result, InstructionResult::PrecompileError);
    }

    #[test]
    fn vector_similarity_dynamic_gas() {
        // 4 dimensions: gas = 200 + 4*10 = 240
        let mut input = Vec::new();
        input.extend_from_slice(&4u32.to_be_bytes());
        input.extend_from_slice(&[0u8; 4 * 4 * 2]); // zero vectors

        // With enough gas
        let result = execute_vector_similarity(&input, 240);
        assert_eq!(result.result, InstructionResult::Return);

        // With insufficient gas
        let result = execute_vector_similarity(&input, 239);
        assert_eq!(result.result, InstructionResult::PrecompileOOG);
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
