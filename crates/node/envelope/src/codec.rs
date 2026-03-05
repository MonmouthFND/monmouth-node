//! Encoding and decoding logic for Monmouth agent transaction envelopes.
//!
//! ## Wire format
//!
//! ```text
//! | Byte 0   | Bytes 1..5       | Bytes 5..5+len      | Remaining bytes |
//! |----------|------------------|---------------------|-----------------|
//! | 0x4d     | u32 BE hdr len   | JSON header         | inner tx bytes  |
//! ```

use tracing::debug;

use crate::{
    error::EnvelopeError,
    types::{AgentTxEnvelope, EnvelopeHeader},
};

/// Magic byte that identifies a Monmouth agent transaction envelope.
///
/// ASCII 'M' (0x4d) — chosen to avoid collision with existing EIP-2718
/// transaction type bytes (0x00..0x7f range is reserved, but 0x4d is
/// currently unused and easily identifiable).
pub const MONMOUTH_MAGIC: u8 = 0x4d;

/// Minimum valid envelope size: 1 (magic) + 4 (header length) = 5 bytes.
const MIN_ENVELOPE_SIZE: usize = 5;

/// Maximum allowed header size (64 KiB).
///
/// Headers are JSON-encoded metadata and should never approach this limit
/// in normal operation. This prevents a malicious envelope from declaring
/// an enormous header that causes excessive memory allocation.
const MAX_HEADER_SIZE: usize = 64 * 1024;

/// Check whether a raw byte slice begins with the Monmouth magic byte.
///
/// This is a cheap check that can be used to decide whether to attempt
/// full envelope decoding or to treat the bytes as a standard transaction.
#[must_use]
pub const fn is_monmouth_envelope(raw: &[u8]) -> bool {
    !raw.is_empty() && raw[0] == MONMOUTH_MAGIC
}

/// Decode a raw byte slice into an [`AgentTxEnvelope`].
///
/// # Wire format
///
/// - Byte 0: `0x4d` (magic)
/// - Bytes 1..5: `u32` big-endian length of the JSON header
/// - Bytes 5..5+len: JSON-encoded [`EnvelopeHeader`]
/// - Remaining bytes: raw inner transaction
///
/// # Errors
///
/// Returns [`EnvelopeError::InvalidMagicByte`] if the first byte is not `0x4d`.
/// Returns [`EnvelopeError::DecodeFailed`] if the envelope is too short or the
/// JSON header cannot be parsed.
pub fn decode_agent_envelope(raw: &[u8]) -> Result<AgentTxEnvelope, EnvelopeError> {
    if raw.len() < MIN_ENVELOPE_SIZE {
        return Err(EnvelopeError::DecodeFailed(format!(
            "envelope too short: {} bytes, minimum is {MIN_ENVELOPE_SIZE}",
            raw.len()
        )));
    }

    if raw[0] != MONMOUTH_MAGIC {
        return Err(EnvelopeError::InvalidMagicByte(raw[0]));
    }

    // Read 4-byte big-endian header length.
    let header_len = u32::from_be_bytes([raw[1], raw[2], raw[3], raw[4]]) as usize;

    if header_len > MAX_HEADER_SIZE {
        return Err(EnvelopeError::DecodeFailed(format!(
            "header length {header_len} exceeds maximum of {MAX_HEADER_SIZE} bytes"
        )));
    }

    let header_end = 5 + header_len;
    if raw.len() < header_end {
        return Err(EnvelopeError::DecodeFailed(format!(
            "envelope truncated: declared header length {header_len}, but only {} bytes available",
            raw.len() - 5
        )));
    }

    let header_bytes = &raw[5..header_end];
    let header: EnvelopeHeader = serde_json::from_slice(header_bytes)
        .map_err(|e| EnvelopeError::DecodeFailed(format!("failed to parse JSON header: {e}")))?;

    let inner_tx = raw[header_end..].to_vec();

    debug!(
        vm_target = %header.vm_target,
        module_hint = ?header.module_hint,
        session_id = ?header.session_id,
        inner_tx_len = inner_tx.len(),
        "Decoded Monmouth agent envelope"
    );

    Ok(AgentTxEnvelope {
        vm_target: header.vm_target,
        module_hint: header.module_hint,
        session_id: header.session_id,
        intent: header.intent,
        inner_tx,
        raw: raw.to_vec(),
    })
}

/// Encode an [`AgentTxEnvelope`] into its wire-format byte representation.
///
/// The resulting bytes follow the format:
/// - Byte 0: `0x4d` (magic)
/// - Bytes 1..5: `u32` big-endian length of the JSON header
/// - Bytes 5..5+len: JSON-encoded header
/// - Remaining bytes: inner transaction bytes
pub fn encode_agent_envelope(envelope: &AgentTxEnvelope) -> Result<Vec<u8>, EnvelopeError> {
    let header = EnvelopeHeader {
        vm_target: envelope.vm_target,
        module_hint: envelope.module_hint.clone(),
        session_id: envelope.session_id,
        intent: envelope.intent.clone(),
    };

    let header_json = serde_json::to_vec(&header)
        .map_err(|e| EnvelopeError::EncodeFailed(format!("header serialisation failed: {e}")))?;
    let header_len = header_json.len() as u32;

    let mut buf = Vec::with_capacity(1 + 4 + header_json.len() + envelope.inner_tx.len());
    buf.push(MONMOUTH_MAGIC);
    buf.extend_from_slice(&header_len.to_be_bytes());
    buf.extend_from_slice(&header_json);
    buf.extend_from_slice(&envelope.inner_tx);

    debug!(
        total_len = buf.len(),
        header_len = header_json.len(),
        inner_tx_len = envelope.inner_tx.len(),
        "Encoded Monmouth agent envelope"
    );

    Ok(buf)
}

#[cfg(test)]
mod tests {
    use alloy_primitives::B256;
    use monmouth_agent_types::{IntentDeclaration, SessionId, VmTarget};

    use super::*;

    fn sample_envelope() -> AgentTxEnvelope {
        AgentTxEnvelope {
            vm_target: VmTarget::Evm,
            module_hint: Some("sim.preview".to_string()),
            session_id: Some(SessionId(B256::repeat_byte(0x42))),
            intent: Some(IntentDeclaration {
                description: "Transfer ETH".to_string(),
                intent_type: "transfer".to_string(),
                expected_outcome: "1 ETH sent to recipient".to_string(),
            }),
            inner_tx: vec![0x02, 0xf8, 0x70, 0x82, 0x01],
            raw: Vec::new(),
        }
    }

    fn minimal_envelope() -> AgentTxEnvelope {
        AgentTxEnvelope {
            vm_target: VmTarget::Evm,
            module_hint: None,
            session_id: None,
            intent: None,
            inner_tx: vec![0x02, 0xf8],
            raw: Vec::new(),
        }
    }

    #[test]
    fn encode_decode_roundtrip() {
        let original = sample_envelope();
        let encoded = encode_agent_envelope(&original).unwrap();
        let decoded = decode_agent_envelope(&encoded).unwrap();

        assert_eq!(decoded.vm_target, original.vm_target);
        assert_eq!(decoded.module_hint, original.module_hint);
        assert_eq!(decoded.session_id, original.session_id);
        assert_eq!(decoded.intent, original.intent);
        assert_eq!(decoded.inner_tx, original.inner_tx);
        assert_eq!(decoded.raw, encoded);
    }

    #[test]
    fn encode_decode_minimal() {
        let original = minimal_envelope();
        let encoded = encode_agent_envelope(&original).unwrap();
        let decoded = decode_agent_envelope(&encoded).unwrap();

        assert_eq!(decoded.vm_target, VmTarget::Evm);
        assert_eq!(decoded.module_hint, None);
        assert_eq!(decoded.session_id, None);
        assert_eq!(decoded.intent, None);
        assert_eq!(decoded.inner_tx, original.inner_tx);
    }

    #[test]
    fn encode_decode_svm_target() {
        let mut envelope = minimal_envelope();
        envelope.vm_target = VmTarget::Svm;
        let encoded = encode_agent_envelope(&envelope).unwrap();
        let decoded = decode_agent_envelope(&encoded).unwrap();
        assert_eq!(decoded.vm_target, VmTarget::Svm);
    }

    #[test]
    fn magic_byte_is_first() {
        let envelope = minimal_envelope();
        let encoded = encode_agent_envelope(&envelope).unwrap();
        assert_eq!(encoded[0], MONMOUTH_MAGIC);
        assert_eq!(encoded[0], 0x4d);
    }

    #[test]
    fn is_monmouth_envelope_check() {
        let envelope = minimal_envelope();
        let encoded = encode_agent_envelope(&envelope).unwrap();
        assert!(is_monmouth_envelope(&encoded));

        // Standard EIP-1559 tx starts with 0x02.
        assert!(!is_monmouth_envelope(&[0x02, 0xf8, 0x70]));

        // Empty input.
        assert!(!is_monmouth_envelope(&[]));

        // Just the magic byte alone (too short for full decode, but passes check).
        assert!(is_monmouth_envelope(&[MONMOUTH_MAGIC]));
    }

    #[test]
    fn invalid_magic_byte() {
        let err = decode_agent_envelope(&[0x02, 0x00, 0x00, 0x00, 0x02, b'{', b'}']).unwrap_err();
        assert!(matches!(err, EnvelopeError::InvalidMagicByte(0x02)));
    }

    #[test]
    fn envelope_too_short() {
        let err = decode_agent_envelope(&[MONMOUTH_MAGIC, 0x00]).unwrap_err();
        assert!(matches!(err, EnvelopeError::DecodeFailed(_)));
    }

    #[test]
    fn truncated_header() {
        // Magic + header_len says 100 bytes but we only have a few.
        let mut raw = vec![MONMOUTH_MAGIC];
        raw.extend_from_slice(&100u32.to_be_bytes());
        raw.extend_from_slice(b"{}");
        let err = decode_agent_envelope(&raw).unwrap_err();
        assert!(matches!(err, EnvelopeError::DecodeFailed(_)));
    }

    #[test]
    fn invalid_json_header() {
        let bad_json = b"not valid json";
        let header_len = bad_json.len() as u32;
        let mut raw = vec![MONMOUTH_MAGIC];
        raw.extend_from_slice(&header_len.to_be_bytes());
        raw.extend_from_slice(bad_json);
        let err = decode_agent_envelope(&raw).unwrap_err();
        assert!(matches!(err, EnvelopeError::DecodeFailed(_)));
    }

    #[test]
    fn empty_inner_tx() {
        let envelope = AgentTxEnvelope {
            vm_target: VmTarget::Evm,
            module_hint: None,
            session_id: None,
            intent: None,
            inner_tx: vec![],
            raw: Vec::new(),
        };
        let encoded = encode_agent_envelope(&envelope).unwrap();
        let decoded = decode_agent_envelope(&encoded).unwrap();
        assert!(decoded.inner_tx.is_empty());
    }

    #[test]
    fn header_length_encoding() {
        let envelope = sample_envelope();
        let encoded = encode_agent_envelope(&envelope).unwrap();

        // Extract the header length from bytes 1..5.
        let header_len =
            u32::from_be_bytes([encoded[1], encoded[2], encoded[3], encoded[4]]) as usize;

        // The header JSON should be exactly at bytes 5..5+header_len.
        let header_bytes = &encoded[5..5 + header_len];
        let _header: serde_json::Value = serde_json::from_slice(header_bytes).unwrap();

        // Inner tx should be the rest.
        let inner_tx = &encoded[5 + header_len..];
        assert_eq!(inner_tx, &envelope.inner_tx);
    }

    #[test]
    fn oversized_header_rejected() {
        // Declare a header of 128 KiB (exceeds MAX_HEADER_SIZE of 64 KiB).
        let huge_len = 128 * 1024u32;
        let mut raw = vec![MONMOUTH_MAGIC];
        raw.extend_from_slice(&huge_len.to_be_bytes());
        raw.extend(vec![0u8; huge_len as usize]); // filler bytes
        let err = decode_agent_envelope(&raw).unwrap_err();
        assert!(matches!(err, EnvelopeError::DecodeFailed(_)));
        assert!(err.to_string().contains("exceeds maximum"));
    }

    #[test]
    fn error_codes() {
        assert_eq!(EnvelopeError::InvalidMagicByte(0).code(), -32600);
        assert_eq!(EnvelopeError::DecodeFailed(String::new()).code(), -32601);
        assert_eq!(EnvelopeError::MissingField(String::new()).code(), -32602);
        assert_eq!(EnvelopeError::InvalidVmTarget(String::new()).code(), -32603);
        assert_eq!(EnvelopeError::EncodeFailed(String::new()).code(), -32604);
    }
}
