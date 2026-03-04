//! Core types for the agent transaction envelope.

use monmouth_agent_types::{IntentDeclaration, SessionId, VmTarget};
use serde::{Deserialize, Serialize};

/// An extended transaction envelope that wraps a standard EIP-2718 transaction
/// with Monmouth-specific metadata: VM routing, module hints, session
/// delegation, and intent declarations.
///
/// The envelope is serialised with a `0x4d` magic byte prefix, followed by a
/// JSON header and the raw inner transaction bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentTxEnvelope {
    /// Target virtual machine for transaction routing.
    pub vm_target: VmTarget,

    /// Optional hint for which native module should handle the transaction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module_hint: Option<String>,

    /// Optional session identifier for delegated execution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<SessionId>,

    /// Optional declared intent accompanying the transaction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<IntentDeclaration>,

    /// Raw inner transaction bytes (e.g. RLP-encoded EIP-1559 tx).
    #[serde(with = "hex_bytes")]
    pub inner_tx: Vec<u8>,

    /// Full raw envelope bytes (magic + header length + header + inner tx).
    /// Populated during decoding; during encoding this is computed.
    #[serde(skip)]
    pub raw: Vec<u8>,
}

/// Serde helper for hex-encoding `Vec<u8>` fields.
mod hex_bytes {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    /// Serialise bytes as a `0x`-prefixed hex string.
    pub(crate) fn serialize<S: Serializer>(bytes: &Vec<u8>, s: S) -> Result<S::Ok, S::Error> {
        let hex_str = format!("0x{}", hex::encode(bytes));
        hex_str.serialize(s)
    }

    /// Deserialise a `0x`-prefixed hex string into bytes.
    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let s = String::deserialize(d)?;
        let s = s.strip_prefix("0x").unwrap_or(&s);
        hex::decode(s).map_err(serde::de::Error::custom)
    }
}

/// JSON header portion of the envelope (everything except `inner_tx` and `raw`).
///
/// This is the structure that gets serialised into the envelope header between
/// the magic byte + length prefix and the inner transaction bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EnvelopeHeader {
    /// Target virtual machine.
    pub(crate) vm_target: VmTarget,

    /// Optional module hint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) module_hint: Option<String>,

    /// Optional session identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) session_id: Option<SessionId>,

    /// Optional declared intent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) intent: Option<IntentDeclaration>,
}

#[cfg(test)]
mod tests {
    use alloy_primitives::B256;
    use monmouth_agent_types::VmTarget;

    use super::*;

    #[test]
    fn envelope_json_roundtrip() {
        let envelope = AgentTxEnvelope {
            vm_target: VmTarget::Evm,
            module_hint: Some("sim.preview".to_string()),
            session_id: Some(SessionId(B256::ZERO)),
            intent: Some(IntentDeclaration {
                description: "Transfer tokens".to_string(),
                intent_type: "transfer".to_string(),
                expected_outcome: "100 tokens sent".to_string(),
            }),
            inner_tx: vec![0x02, 0xf8, 0x50],
            raw: Vec::new(),
        };

        let json = serde_json::to_string(&envelope).unwrap();
        let parsed: AgentTxEnvelope = serde_json::from_str(&json).unwrap();

        assert_eq!(envelope.vm_target, parsed.vm_target);
        assert_eq!(envelope.module_hint, parsed.module_hint);
        assert_eq!(envelope.session_id, parsed.session_id);
        assert_eq!(envelope.intent, parsed.intent);
        assert_eq!(envelope.inner_tx, parsed.inner_tx);
    }

    #[test]
    fn envelope_without_optional_fields() {
        let envelope = AgentTxEnvelope {
            vm_target: VmTarget::Svm,
            module_hint: None,
            session_id: None,
            intent: None,
            inner_tx: vec![0xaa, 0xbb],
            raw: Vec::new(),
        };

        let json = serde_json::to_string(&envelope).unwrap();
        assert!(!json.contains("moduleHint"));
        assert!(!json.contains("sessionId"));
        assert!(!json.contains("intent"));

        let parsed: AgentTxEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(envelope.vm_target, parsed.vm_target);
        assert_eq!(envelope.inner_tx, parsed.inner_tx);
    }
}
