//! Agent-aware transaction classification.
//!
//! Classifies transactions before REVM execution based on function selectors,
//! target addresses, and calldata patterns. This enables the Monmouth node to
//! route transactions to appropriate execution environments.

use alloy_primitives::{Address, Bytes, address};

/// Well-known ERC-8004 registry addresses on Monmouth.
pub mod registries {
    use super::*;

    /// IdentityRegistry contract address.
    pub const IDENTITY_REGISTRY: Address = address!("0x8004000000000000000000000000000000000001");
    /// ReputationRegistry contract address.
    pub const REPUTATION_REGISTRY: Address = address!("0x8004000000000000000000000000000000000002");
    /// ValidationRegistry contract address.
    pub const VALIDATION_REGISTRY: Address = address!("0x8004000000000000000000000000000000000003");
}

/// Well-known function selectors for classification heuristics.
mod selectors {
    /// ERC-8004 IdentityRegistry.register(string)
    pub(super) const REGISTER_AGENT: [u8; 4] = [0x1a, 0xa3, 0xa0, 0x08];
    /// ERC-8004 ReputationRegistry.giveFeedback(...)
    pub(super) const GIVE_FEEDBACK: [u8; 4] = [0x2b, 0x4d, 0x7c, 0xf5];
    /// ERC-8004 ValidationRegistry.validationRequest(...)
    pub(super) const VALIDATION_REQUEST: [u8; 4] = [0x3c, 0x5e, 0x8d, 0x06];
    /// SVM Router precompile selector
    pub(super) const SVM_ROUTE: [u8; 4] = [0x53, 0x56, 0x4d, 0x00]; // "SVM\0"
    /// Vector similarity search selector
    pub(super) const VECTOR_SEARCH: [u8; 4] = [0x76, 0x65, 0x63, 0x73]; // "vecs"
    /// Intent parser selector
    pub(super) const PARSE_INTENT: [u8; 4] = [0x69, 0x6e, 0x74, 0x70]; // "intp"
    /// AI inference selector
    pub(super) const AI_INFER: [u8; 4] = [0x61, 0x69, 0x6e, 0x66]; // "ainf"
}

/// Precompile addresses for agent-native operations.
pub mod precompiles {
    use super::*;

    /// AI Inference precompile.
    pub const AI_INFERENCE: Address = address!("0x0000000000000000000000000000000000001000");
    /// Vector Similarity precompile.
    pub const VECTOR_SIMILARITY: Address = address!("0x0000000000000000000000000000000000001001");
    /// Intent Parser precompile.
    pub const INTENT_PARSER: Address = address!("0x0000000000000000000000000000000000001002");
    /// SVM Router precompile.
    pub const SVM_ROUTER: Address = address!("0x0000000000000000000000000000000000001003");
    /// Cross-Chain Message Passer precompile.
    pub const CROSS_CHAIN_MESSAGE_PASSER: Address = address!("0x0000000000000000000000000000000000004200");
}

/// Classification of a transaction before execution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransactionClassification {
    /// Standard EVM execution - no special routing needed.
    PureEvm,
    /// Needs Solana VM execution routing.
    SvmRouted,
    /// Multi-chain operation requiring cross-chain coordination.
    HybridCrossChain,
    /// Needs RAG context injection before execution.
    RagEnhanced,
    /// Agent-to-agent commerce (interacts with ERC-8004 registries).
    AgentToAgent,
}

impl std::fmt::Display for TransactionClassification {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PureEvm => write!(f, "PureEvm"),
            Self::SvmRouted => write!(f, "SvmRouted"),
            Self::HybridCrossChain => write!(f, "HybridCrossChain"),
            Self::RagEnhanced => write!(f, "RagEnhanced"),
            Self::AgentToAgent => write!(f, "AgentToAgent"),
        }
    }
}

/// Result of transaction classification with confidence score.
#[derive(Clone, Debug)]
pub struct ClassificationResult {
    /// The classification determined for this transaction.
    pub classification: TransactionClassification,
    /// Confidence score from 0.0 to 1.0.
    pub confidence: f64,
    /// Human-readable reason for the classification.
    pub reason: String,
}

/// Configuration for the transaction classifier.
#[derive(Clone, Debug)]
pub struct ClassifierConfig {
    /// Minimum confidence threshold to accept a non-PureEvm classification.
    /// Below this threshold, transactions fall back to PureEvm.
    pub confidence_threshold: f64,
    /// Whether classification is enabled.
    pub enabled: bool,
}

impl Default for ClassifierConfig {
    fn default() -> Self {
        Self { confidence_threshold: 0.7, enabled: true }
    }
}

/// Agent-aware transaction classifier.
///
/// Inspects transaction target addresses, function selectors, and calldata
/// to classify transactions before they hit the EVM.
#[derive(Clone, Debug)]
pub struct TransactionClassifier {
    config: ClassifierConfig,
}

impl TransactionClassifier {
    /// Create a new classifier with the given configuration.
    #[must_use]
    pub const fn new(config: ClassifierConfig) -> Self {
        Self { config }
    }

    /// Create a classifier with default configuration.
    #[must_use]
    pub fn enabled() -> Self {
        Self::new(ClassifierConfig::default())
    }

    /// Create a disabled classifier that always returns PureEvm.
    #[must_use]
    pub fn disabled() -> Self {
        Self::new(ClassifierConfig { enabled: false, ..ClassifierConfig::default() })
    }

    /// Classify a transaction based on its target and calldata.
    ///
    /// Returns a [`ClassificationResult`] with the determined classification
    /// and confidence score. If the confidence is below the configured threshold,
    /// falls back to [`TransactionClassification::PureEvm`].
    pub fn classify(&self, to: Option<Address>, input: &Bytes) -> ClassificationResult {
        if !self.config.enabled {
            return ClassificationResult {
                classification: TransactionClassification::PureEvm,
                confidence: 1.0,
                reason: "classifier disabled".into(),
            };
        }

        let result = self.classify_inner(to, input);

        // Apply confidence threshold - fall back to PureEvm if below threshold
        if result.classification != TransactionClassification::PureEvm
            && result.confidence < self.config.confidence_threshold
        {
            tracing::debug!(
                original = %result.classification,
                confidence = result.confidence,
                threshold = self.config.confidence_threshold,
                "classification below confidence threshold, falling back to PureEvm"
            );
            return ClassificationResult {
                classification: TransactionClassification::PureEvm,
                confidence: result.confidence,
                reason: format!(
                    "below threshold (was {} at {:.2})",
                    result.classification, result.confidence
                ),
            };
        }

        tracing::debug!(
            classification = %result.classification,
            confidence = result.confidence,
            reason = %result.reason,
            "transaction classified"
        );

        result
    }

    fn classify_inner(&self, to: Option<Address>, input: &Bytes) -> ClassificationResult {
        // Contract creation is always PureEvm
        let target = match to {
            Some(addr) => addr,
            None => {
                return ClassificationResult {
                    classification: TransactionClassification::PureEvm,
                    confidence: 1.0,
                    reason: "contract creation".into(),
                };
            }
        };

        // Check if targeting ERC-8004 registries
        if target == registries::IDENTITY_REGISTRY
            || target == registries::REPUTATION_REGISTRY
            || target == registries::VALIDATION_REGISTRY
        {
            return ClassificationResult {
                classification: TransactionClassification::AgentToAgent,
                confidence: 0.95,
                reason: format!("targets ERC-8004 registry at {target}"),
            };
        }

        // Check if targeting agent precompiles
        if target == precompiles::SVM_ROUTER {
            return ClassificationResult {
                classification: TransactionClassification::SvmRouted,
                confidence: 0.95,
                reason: "targets SVM Router precompile".into(),
            };
        }

        if target == precompiles::VECTOR_SIMILARITY || target == precompiles::AI_INFERENCE {
            return ClassificationResult {
                classification: TransactionClassification::RagEnhanced,
                confidence: 0.90,
                reason: format!("targets AI/RAG precompile at {target}"),
            };
        }

        if target == precompiles::CROSS_CHAIN_MESSAGE_PASSER {
            return ClassificationResult {
                classification: TransactionClassification::HybridCrossChain,
                confidence: 0.95,
                reason: "targets Cross-Chain Message Passer".into(),
            };
        }

        // Check function selectors in calldata
        if input.len() >= 4 {
            let selector: [u8; 4] = input[..4].try_into().unwrap_or_default();

            if selector == selectors::SVM_ROUTE {
                return ClassificationResult {
                    classification: TransactionClassification::SvmRouted,
                    confidence: 0.85,
                    reason: "SVM route function selector".into(),
                };
            }

            if selector == selectors::VECTOR_SEARCH || selector == selectors::AI_INFER {
                return ClassificationResult {
                    classification: TransactionClassification::RagEnhanced,
                    confidence: 0.80,
                    reason: "AI/RAG function selector".into(),
                };
            }

            if selector == selectors::PARSE_INTENT {
                return ClassificationResult {
                    classification: TransactionClassification::RagEnhanced,
                    confidence: 0.80,
                    reason: "intent parser function selector".into(),
                };
            }

            if selector == selectors::REGISTER_AGENT
                || selector == selectors::GIVE_FEEDBACK
                || selector == selectors::VALIDATION_REQUEST
            {
                return ClassificationResult {
                    classification: TransactionClassification::AgentToAgent,
                    confidence: 0.80,
                    reason: "ERC-8004 function selector".into(),
                };
            }
        }

        // Default: PureEvm
        ClassificationResult {
            classification: TransactionClassification::PureEvm,
            confidence: 1.0,
            reason: "no agent-specific patterns detected".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_contract_creation() {
        let classifier = TransactionClassifier::enabled();
        let result = classifier.classify(None, &Bytes::new());
        assert_eq!(result.classification, TransactionClassification::PureEvm);
        assert_eq!(result.confidence, 1.0);
    }

    #[test]
    fn classify_identity_registry() {
        let classifier = TransactionClassifier::enabled();
        let result = classifier.classify(Some(registries::IDENTITY_REGISTRY), &Bytes::new());
        assert_eq!(result.classification, TransactionClassification::AgentToAgent);
        assert!(result.confidence >= 0.9);
    }

    #[test]
    fn classify_reputation_registry() {
        let classifier = TransactionClassifier::enabled();
        let result = classifier.classify(Some(registries::REPUTATION_REGISTRY), &Bytes::new());
        assert_eq!(result.classification, TransactionClassification::AgentToAgent);
    }

    #[test]
    fn classify_validation_registry() {
        let classifier = TransactionClassifier::enabled();
        let result = classifier.classify(Some(registries::VALIDATION_REGISTRY), &Bytes::new());
        assert_eq!(result.classification, TransactionClassification::AgentToAgent);
    }

    #[test]
    fn classify_svm_router_precompile() {
        let classifier = TransactionClassifier::enabled();
        let result = classifier.classify(Some(precompiles::SVM_ROUTER), &Bytes::new());
        assert_eq!(result.classification, TransactionClassification::SvmRouted);
    }

    #[test]
    fn classify_vector_similarity_precompile() {
        let classifier = TransactionClassifier::enabled();
        let result = classifier.classify(Some(precompiles::VECTOR_SIMILARITY), &Bytes::new());
        assert_eq!(result.classification, TransactionClassification::RagEnhanced);
    }

    #[test]
    fn classify_ai_inference_precompile() {
        let classifier = TransactionClassifier::enabled();
        let result = classifier.classify(Some(precompiles::AI_INFERENCE), &Bytes::new());
        assert_eq!(result.classification, TransactionClassification::RagEnhanced);
    }

    #[test]
    fn classify_cross_chain_message_passer() {
        let classifier = TransactionClassifier::enabled();
        let result = classifier.classify(Some(precompiles::CROSS_CHAIN_MESSAGE_PASSER), &Bytes::new());
        assert_eq!(result.classification, TransactionClassification::HybridCrossChain);
    }

    #[test]
    fn classify_svm_function_selector() {
        let classifier = TransactionClassifier::enabled();
        let input = Bytes::from(selectors::SVM_ROUTE.to_vec());
        let result = classifier.classify(Some(Address::ZERO), &input);
        assert_eq!(result.classification, TransactionClassification::SvmRouted);
    }

    #[test]
    fn classify_vector_search_selector() {
        let classifier = TransactionClassifier::enabled();
        let input = Bytes::from(selectors::VECTOR_SEARCH.to_vec());
        let result = classifier.classify(Some(Address::ZERO), &input);
        assert_eq!(result.classification, TransactionClassification::RagEnhanced);
    }

    #[test]
    fn classify_register_agent_selector() {
        let classifier = TransactionClassifier::enabled();
        let input = Bytes::from(selectors::REGISTER_AGENT.to_vec());
        let result = classifier.classify(Some(Address::ZERO), &input);
        assert_eq!(result.classification, TransactionClassification::AgentToAgent);
    }

    #[test]
    fn classify_plain_transfer() {
        let classifier = TransactionClassifier::enabled();
        let result = classifier.classify(Some(Address::ZERO), &Bytes::new());
        assert_eq!(result.classification, TransactionClassification::PureEvm);
    }

    #[test]
    fn classify_unknown_calldata() {
        let classifier = TransactionClassifier::enabled();
        let input = Bytes::from(vec![0xde, 0xad, 0xbe, 0xef, 0x01, 0x02]);
        let result = classifier.classify(Some(Address::ZERO), &input);
        assert_eq!(result.classification, TransactionClassification::PureEvm);
    }

    #[test]
    fn classifier_disabled() {
        let classifier = TransactionClassifier::disabled();
        let result = classifier.classify(Some(registries::IDENTITY_REGISTRY), &Bytes::new());
        assert_eq!(result.classification, TransactionClassification::PureEvm);
        assert_eq!(result.reason, "classifier disabled");
    }

    #[test]
    fn confidence_threshold_fallback() {
        let config = ClassifierConfig { confidence_threshold: 0.99, enabled: true };
        let classifier = TransactionClassifier::new(config);
        // SVM selector has 0.85 confidence, which is below 0.99 threshold
        let input = Bytes::from(selectors::SVM_ROUTE.to_vec());
        let result = classifier.classify(Some(Address::ZERO), &input);
        assert_eq!(result.classification, TransactionClassification::PureEvm);
    }

    #[test]
    fn classification_display() {
        assert_eq!(TransactionClassification::PureEvm.to_string(), "PureEvm");
        assert_eq!(TransactionClassification::SvmRouted.to_string(), "SvmRouted");
        assert_eq!(TransactionClassification::HybridCrossChain.to_string(), "HybridCrossChain");
        assert_eq!(TransactionClassification::RagEnhanced.to_string(), "RagEnhanced");
        assert_eq!(TransactionClassification::AgentToAgent.to_string(), "AgentToAgent");
    }

    #[test]
    fn short_calldata_no_panic() {
        let classifier = TransactionClassifier::enabled();
        // Calldata shorter than 4 bytes should not panic
        let result = classifier.classify(Some(Address::ZERO), &Bytes::from(vec![0x01, 0x02]));
        assert_eq!(result.classification, TransactionClassification::PureEvm);
    }
}
