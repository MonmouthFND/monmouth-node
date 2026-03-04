//! Shared types for Monmouth agent-native modules.
//!
//! Contains types that are used across two or more native module crates,
//! providing a single source of truth for agent identifiers, VM targets,
//! session grants, policy rules, job definitions, and more.

#![doc(issue_tracker_base_url = "https://github.com/MonmouthFND/monmouth-node/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use std::fmt;

use alloy_primitives::{Address, B256, U256};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Agent identity
// ---------------------------------------------------------------------------

/// Strongly-typed agent address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AgentId(pub Address);

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Address> for AgentId {
    fn from(addr: Address) -> Self {
        Self(addr)
    }
}

impl From<AgentId> for Address {
    fn from(id: AgentId) -> Self {
        id.0
    }
}

// ---------------------------------------------------------------------------
// VM target
// ---------------------------------------------------------------------------

/// Target virtual machine for transaction routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VmTarget {
    /// Ethereum Virtual Machine.
    Evm,
    /// Solana Virtual Machine.
    Svm,
}

impl Default for VmTarget {
    fn default() -> Self {
        Self::Evm
    }
}

impl fmt::Display for VmTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Evm => write!(f, "evm"),
            Self::Svm => write!(f, "svm"),
        }
    }
}

// ---------------------------------------------------------------------------
// Intent declarations
// ---------------------------------------------------------------------------

/// Declared intent accompanying a transaction for audit trail purposes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntentDeclaration {
    /// Human-readable description of the intended action.
    pub description: String,
    /// Categorisation of the intent (e.g. "swap", "transfer", "stake").
    pub intent_type: String,
    /// Expected outcome description.
    pub expected_outcome: String,
}

// ---------------------------------------------------------------------------
// Delegation / Session keys
// ---------------------------------------------------------------------------

/// Unique identifier for a delegation session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionId(pub B256);

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A grant of scoped, time-limited permissions from an owner to a delegate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionGrant {
    /// The address that owns the session and grants permissions.
    pub owner: Address,
    /// The address that receives delegated permissions.
    pub delegate: Address,
    /// List of capability IDs the delegate may invoke.
    pub capabilities: Vec<String>,
    /// Maximum total spend in wei for this session.
    pub spending_limit_wei: U256,
    /// Unix timestamp when the session expires.
    pub expires_at: u64,
    /// Nonce to prevent replay attacks.
    pub nonce: u64,
}

// ---------------------------------------------------------------------------
// Policy engine
// ---------------------------------------------------------------------------

/// Unique identifier for a policy rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PolicyRuleId(pub B256);

impl fmt::Display for PolicyRuleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Decision returned by the policy engine after evaluating a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PolicyAction {
    /// The action is allowed.
    Allow,
    /// The action is denied.
    Deny,
    /// The action requires human confirmation before proceeding.
    RequireConfirmation,
}

/// A spending cap constraint for the policy engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpendingCap {
    /// Maximum spend in wei within the window.
    pub max_wei: U256,
    /// Window duration in seconds.
    pub window_secs: u64,
}

/// A rate limit constraint for the policy engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RateLimitRule {
    /// Maximum number of operations within the window.
    pub max_ops: u64,
    /// Window duration in seconds.
    pub window_secs: u64,
}

/// A policy rule that the engine evaluates for each agent action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyRule {
    /// Unique rule identifier.
    pub id: PolicyRuleId,
    /// Optional agent this rule applies to (`None` = global).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<AgentId>,
    /// Optional capability ID filter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability_id: Option<String>,
    /// The action to take when this rule matches.
    pub action: PolicyAction,
    /// Optional spending cap.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spending_cap: Option<SpendingCap>,
    /// Optional rate limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate_limit: Option<RateLimitRule>,
}

/// Result of policy evaluation with the matched rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyDecision {
    /// The decided action.
    pub action: PolicyAction,
    /// ID of the rule that determined the decision, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matched_rule: Option<PolicyRuleId>,
    /// Human-readable reason for the decision.
    pub reason: String,
}

// ---------------------------------------------------------------------------
// Coordination / Jobs
// ---------------------------------------------------------------------------

/// Unique identifier for a coordination job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct JobId(pub B256);

impl fmt::Display for JobId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Status of a coordination job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum JobStatus {
    /// Job has been proposed but not yet accepted.
    Proposed,
    /// Job has been accepted by an executor.
    Accepted,
    /// Job is currently being executed.
    Executing,
    /// Job execution completed successfully.
    Completed,
    /// Job is under dispute.
    Disputed,
    /// Job has been settled (payment released).
    Settled,
    /// Job has been cancelled.
    Cancelled,
}

/// A coordination job between agents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    /// Unique job identifier.
    pub id: JobId,
    /// Agent proposing the job.
    pub proposer: AgentId,
    /// Agent executing the job (set on acceptance).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub executor: Option<AgentId>,
    /// Current status of the job.
    pub status: JobStatus,
    /// Description of the work to be done.
    pub description: String,
    /// Capability ID required for this job.
    pub capability_id: String,
    /// Payment amount in wei.
    pub payment_wei: U256,
    /// Amount held in escrow.
    pub escrow_held: U256,
    /// Optional result hash (set on completion).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_hash: Option<B256>,
    /// Deadline as unix timestamp.
    pub deadline: u64,
    /// Creation timestamp.
    pub created_at: u64,
}

/// Escrow entry tracking funds held for a job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EscrowEntry {
    /// The associated job ID.
    pub job_id: JobId,
    /// Address holding the escrow.
    pub holder: AgentId,
    /// Amount held in wei.
    pub amount: U256,
    /// Whether the escrow has been released.
    pub released: bool,
}

// ---------------------------------------------------------------------------
// Attestation
// ---------------------------------------------------------------------------

/// Unique identifier for an attestation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AttestationId(pub B256);

impl fmt::Display for AttestationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Type of cryptographic attestation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AttestationType {
    /// Ed25519 digital signature.
    Ed25519Signature,
    /// Secp256k1 digital signature (Ethereum-style).
    Secp256k1Signature,
    /// Trusted Execution Environment quote.
    TeeQuote,
    /// Zero-knowledge proof.
    ZkProof,
}

/// A cryptographic attestation of off-chain computation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attestation {
    /// Unique attestation identifier.
    pub id: AttestationId,
    /// Type of attestation.
    pub attestation_type: AttestationType,
    /// Address of the attester.
    pub attester: Address,
    /// Hash of the subject being attested.
    pub subject_hash: B256,
    /// Raw attestation payload (signature, proof, etc.).
    pub payload: Vec<u8>,
    /// Timestamp when the attestation was created.
    pub timestamp: u64,
    /// Whether the attestation has been cryptographically verified.
    pub verified: bool,
}

// ---------------------------------------------------------------------------
// Conditional subscriptions
// ---------------------------------------------------------------------------

/// Unique identifier for a conditional subscription.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SubscriptionId(pub B256);

impl fmt::Display for SubscriptionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Condition that triggers a subscription.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum TriggerCondition {
    /// Triggered when an account balance drops below a threshold.
    BalanceBelow {
        /// Account to monitor.
        address: Address,
        /// Threshold in wei.
        threshold: U256,
    },
    /// Triggered when an account balance exceeds a threshold.
    BalanceAbove {
        /// Account to monitor.
        address: Address,
        /// Threshold in wei.
        threshold: U256,
    },
    /// Triggered when a specific storage slot changes.
    StorageChanged {
        /// Contract address.
        address: Address,
        /// Storage slot key.
        slot: B256,
    },
    /// Triggered when a specific event is emitted.
    EventEmitted {
        /// Contract address.
        address: Address,
        /// Event topic0 (signature hash).
        topic0: B256,
    },
    /// Triggered at a specific block number.
    BlockNumber {
        /// Target block number.
        block: u64,
    },
    /// Triggered when gas price drops below a threshold.
    GasPriceBelow {
        /// Gas price threshold in wei.
        threshold: U256,
    },
}

/// A conditional subscription that fires when trigger conditions are met.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConditionalSubscription {
    /// Unique subscription identifier.
    pub id: SubscriptionId,
    /// Owner agent.
    pub owner: AgentId,
    /// Condition that triggers this subscription.
    pub condition: TriggerCondition,
    /// Optional webhook URL to call on trigger.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub webhook_url: Option<String>,
    /// Maximum number of times this subscription can fire.
    pub max_triggers: u64,
    /// Number of times this subscription has fired.
    pub trigger_count: u64,
    /// Whether the subscription is currently active.
    pub active: bool,
    /// Creation timestamp.
    pub created_at: u64,
}

// ---------------------------------------------------------------------------
// Memory anchoring
// ---------------------------------------------------------------------------

/// Key for looking up a specific memory anchor.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryAnchorKey {
    /// The agent that created the anchor.
    pub agent: AgentId,
    /// Sequence number for the agent's anchors.
    pub sequence: u64,
}

/// An on-chain commitment to off-chain agent context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryAnchor {
    /// The agent that created this anchor.
    pub agent: AgentId,
    /// Monotonically increasing sequence number for this agent.
    pub sequence: u64,
    /// Hash of the off-chain content being anchored.
    pub content_hash: B256,
    /// Human-readable label for the anchor.
    pub label: String,
    /// Timestamp when the anchor was created.
    pub timestamp: u64,
}

// ---------------------------------------------------------------------------
// State observation
// ---------------------------------------------------------------------------

/// A structured state query for agent-friendly reads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum StateQuery {
    /// Query account balance.
    Balance {
        /// Account address.
        address: Address,
    },
    /// Query account nonce.
    Nonce {
        /// Account address.
        address: Address,
    },
    /// Query account code.
    Code {
        /// Account address.
        address: Address,
    },
    /// Query a specific storage slot.
    Storage {
        /// Contract address.
        address: Address,
        /// Storage slot key.
        slot: B256,
    },
    /// Query ERC-20 token balance for an account.
    Erc20Balance {
        /// ERC-20 token contract address.
        token: Address,
        /// Account to check balance for.
        account: Address,
    },
    /// Query a contract's state by calling a view function.
    ContractState {
        /// Contract address.
        address: Address,
        /// ABI-encoded calldata.
        calldata: Vec<u8>,
    },
    /// Query multiple account balances at once.
    MultiBalance {
        /// Account addresses.
        addresses: Vec<Address>,
    },
}

/// Result of a state query.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum StateQueryResult {
    /// Balance result in wei.
    Balance {
        /// The queried balance.
        balance: U256,
    },
    /// Nonce result.
    Nonce {
        /// The queried nonce.
        nonce: u64,
    },
    /// Code result as bytes.
    Code {
        /// The contract bytecode.
        code: Vec<u8>,
    },
    /// Storage slot value.
    Storage {
        /// The stored value.
        value: B256,
    },
    /// ERC-20 balance result.
    Erc20Balance {
        /// The token balance.
        balance: U256,
    },
    /// Raw return data from a contract call.
    ContractState {
        /// ABI-encoded return data.
        data: Vec<u8>,
    },
    /// Multiple balance results.
    MultiBalance {
        /// Balances in the same order as the queried addresses.
        balances: Vec<U256>,
    },
    /// Error result when a query fails.
    Error {
        /// Error message.
        message: String,
    },
}

// ---------------------------------------------------------------------------
// Simulation
// ---------------------------------------------------------------------------

/// Change to a single account during simulation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountDiff {
    /// Account address.
    pub address: Address,
    /// Balance change (positive = increase).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub balance_before: Option<U256>,
    /// Balance after.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub balance_after: Option<U256>,
    /// Nonce change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nonce_before: Option<u64>,
    /// Nonce after.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nonce_after: Option<u64>,
    /// Storage changes: slot → (old, new).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub storage_changes: Vec<StorageChange>,
}

/// A single storage slot change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageChange {
    /// Storage slot key.
    pub slot: B256,
    /// Value before.
    pub before: B256,
    /// Value after.
    pub after: B256,
}

/// Result of a transaction simulation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SimulationResult {
    /// Whether the simulation succeeded.
    pub success: bool,
    /// Gas used by the transaction.
    pub gas_used: u64,
    /// State changes produced by the transaction.
    #[serde(default)]
    pub state_changes: Vec<AccountDiff>,
    /// Logs emitted during execution.
    #[serde(default)]
    pub logs: Vec<SimulationLog>,
    /// Projected state root after execution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub projected_state_root: Option<B256>,
    /// Error message if the simulation failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// A log emitted during simulation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SimulationLog {
    /// Contract address that emitted the log.
    pub address: Address,
    /// Log topics.
    pub topics: Vec<B256>,
    /// Log data.
    pub data: Vec<u8>,
}

/// Request to simulate a bundle of transactions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleSimRequest {
    /// Raw transactions to simulate in sequence.
    pub transactions: Vec<Vec<u8>>,
    /// Optional block number context.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block_number: Option<u64>,
}

// ---------------------------------------------------------------------------
// Intent receipts
// ---------------------------------------------------------------------------

/// Actual outcome of a transaction compared to declared intent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActualOutcome {
    /// Whether the transaction succeeded.
    pub success: bool,
    /// Gas used by the transaction.
    pub gas_used: u64,
    /// Summary of what actually happened.
    pub summary: String,
}

/// A receipt recording both the declared intent and actual outcome.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntentReceipt {
    /// Transaction hash.
    pub tx_hash: B256,
    /// Agent that submitted the transaction.
    pub agent: AgentId,
    /// The declared intent before execution.
    pub declared_intent: IntentDeclaration,
    /// The actual outcome after execution.
    pub actual_outcome: ActualOutcome,
    /// Score from 0.0 to 1.0 indicating how well the outcome matched intent.
    pub match_score: f64,
    /// Timestamp when the receipt was created.
    pub timestamp: u64,
    /// Block number in which the transaction was included.
    pub block_number: u64,
}

// ---------------------------------------------------------------------------
// SVM types
// ---------------------------------------------------------------------------

/// A Solana VM instruction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SvmInstruction {
    /// Program ID index into account_keys.
    pub program_id_index: u8,
    /// Indices into account_keys for instruction accounts.
    pub accounts: Vec<u8>,
    /// Instruction data.
    pub data: Vec<u8>,
}

/// A Solana VM message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SvmMessage {
    /// Account keys involved in the transaction.
    pub account_keys: Vec<Vec<u8>>,
    /// Recent blockhash.
    pub recent_blockhash: Vec<u8>,
    /// Instructions to execute.
    pub instructions: Vec<SvmInstruction>,
}

/// A Solana VM transaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SvmTransaction {
    /// Transaction signatures.
    pub signatures: Vec<Vec<u8>>,
    /// Transaction message.
    pub message: SvmMessage,
}

/// Result of SVM execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SvmExecutionResult {
    /// Whether execution succeeded.
    pub success: bool,
    /// Compute units consumed.
    pub compute_units_consumed: u64,
    /// Logs emitted during execution.
    #[serde(default)]
    pub logs: Vec<String>,
    /// Error message if execution failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Status response for the SVM module.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SvmStatusResponse {
    /// Whether the SVM module is enabled.
    pub enabled: bool,
    /// Compute budget limit.
    pub compute_budget: u64,
    /// Human-readable status message.
    pub status: String,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_id_roundtrip() {
        let id = AgentId(Address::ZERO);
        let json = serde_json::to_string(&id).unwrap();
        let parsed: AgentId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn vm_target_default_is_evm() {
        assert_eq!(VmTarget::default(), VmTarget::Evm);
    }

    #[test]
    fn vm_target_serialization() {
        assert_eq!(serde_json::to_string(&VmTarget::Evm).unwrap(), "\"evm\"");
        assert_eq!(serde_json::to_string(&VmTarget::Svm).unwrap(), "\"svm\"");
    }

    #[test]
    fn session_grant_roundtrip() {
        let grant = SessionGrant {
            owner: Address::ZERO,
            delegate: Address::repeat_byte(1),
            capabilities: vec!["sim.preview".to_string()],
            spending_limit_wei: U256::from(1_000_000u64),
            expires_at: 1_700_000_000,
            nonce: 1,
        };
        let json = serde_json::to_string(&grant).unwrap();
        let parsed: SessionGrant = serde_json::from_str(&json).unwrap();
        assert_eq!(grant, parsed);
    }

    #[test]
    fn policy_rule_roundtrip() {
        let rule = PolicyRule {
            id: PolicyRuleId(B256::ZERO),
            agent: Some(AgentId(Address::ZERO)),
            capability_id: Some("sim.preview".to_string()),
            action: PolicyAction::Allow,
            spending_cap: Some(SpendingCap {
                max_wei: U256::from(1_000_000u64),
                window_secs: 3600,
            }),
            rate_limit: Some(RateLimitRule { max_ops: 100, window_secs: 60 }),
        };
        let json = serde_json::to_string(&rule).unwrap();
        let parsed: PolicyRule = serde_json::from_str(&json).unwrap();
        assert_eq!(rule, parsed);
    }

    #[test]
    fn job_status_serialization() {
        assert_eq!(serde_json::to_string(&JobStatus::Proposed).unwrap(), "\"proposed\"");
        assert_eq!(serde_json::to_string(&JobStatus::Settled).unwrap(), "\"settled\"");
    }

    #[test]
    fn trigger_condition_serialization() {
        let cond = TriggerCondition::BalanceBelow {
            address: Address::ZERO,
            threshold: U256::from(100u64),
        };
        let json = serde_json::to_string(&cond).unwrap();
        assert!(json.contains("\"type\":\"balanceBelow\""));
        let parsed: TriggerCondition = serde_json::from_str(&json).unwrap();
        assert_eq!(cond, parsed);
    }

    #[test]
    fn memory_anchor_roundtrip() {
        let anchor = MemoryAnchor {
            agent: AgentId(Address::ZERO),
            sequence: 42,
            content_hash: B256::ZERO,
            label: "test-anchor".to_string(),
            timestamp: 1_700_000_000,
        };
        let json = serde_json::to_string(&anchor).unwrap();
        let parsed: MemoryAnchor = serde_json::from_str(&json).unwrap();
        assert_eq!(anchor, parsed);
    }

    #[test]
    fn state_query_tagged_enum() {
        let query = StateQuery::Balance { address: Address::ZERO };
        let json = serde_json::to_string(&query).unwrap();
        assert!(json.contains("\"type\":\"balance\""));
        let parsed: StateQuery = serde_json::from_str(&json).unwrap();
        assert_eq!(query, parsed);
    }

    #[test]
    fn simulation_result_roundtrip() {
        let result = SimulationResult {
            success: true,
            gas_used: 21000,
            state_changes: vec![],
            logs: vec![],
            projected_state_root: None,
            error: None,
        };
        let json = serde_json::to_string(&result).unwrap();
        let parsed: SimulationResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, parsed);
    }

    #[test]
    fn intent_receipt_roundtrip() {
        let receipt = IntentReceipt {
            tx_hash: B256::ZERO,
            agent: AgentId(Address::ZERO),
            declared_intent: IntentDeclaration {
                description: "Transfer tokens".to_string(),
                intent_type: "transfer".to_string(),
                expected_outcome: "100 tokens moved".to_string(),
            },
            actual_outcome: ActualOutcome {
                success: true,
                gas_used: 21000,
                summary: "100 tokens moved".to_string(),
            },
            match_score: 1.0,
            timestamp: 1_700_000_000,
            block_number: 100,
        };
        let json = serde_json::to_string(&receipt).unwrap();
        let parsed: IntentReceipt = serde_json::from_str(&json).unwrap();
        assert_eq!(receipt.tx_hash, parsed.tx_hash);
        assert!((receipt.match_score - parsed.match_score).abs() < f64::EPSILON);
    }

    #[test]
    fn attestation_type_serialization() {
        assert_eq!(
            serde_json::to_string(&AttestationType::Ed25519Signature).unwrap(),
            "\"ed25519Signature\""
        );
        assert_eq!(
            serde_json::to_string(&AttestationType::Secp256k1Signature).unwrap(),
            "\"secp256k1Signature\""
        );
    }

    #[test]
    fn svm_status_roundtrip() {
        let status = SvmStatusResponse {
            enabled: false,
            compute_budget: 200_000,
            status: "disabled".to_string(),
        };
        let json = serde_json::to_string(&status).unwrap();
        let parsed: SvmStatusResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(status, parsed);
    }
}
