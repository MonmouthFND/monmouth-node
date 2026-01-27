//! Consensus configuration.

use std::path::PathBuf;

use alloy_primitives::hex;
use commonware_codec::{FixedSize, ReadExt};
use commonware_cryptography::ed25519;
use serde::{Deserialize, Serialize};

use crate::ConfigError;

/// Default validator threshold.
pub const DEFAULT_THRESHOLD: u32 = 2;

/// Consensus layer configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConsensusConfig {
    /// Path to the validator key file.
    #[serde(default)]
    pub validator_key: Option<PathBuf>,

    /// Threshold for consensus (e.g., 2f+1 of 3f+1).
    #[serde(default = "default_threshold")]
    pub threshold: u32,

    /// List of participant public keys (hex-encoded).
    #[serde(
        default,
        serialize_with = "serialize_participants",
        deserialize_with = "deserialize_participants"
    )]
    pub participants: Vec<Vec<u8>>,
}

impl Default for ConsensusConfig {
    fn default() -> Self {
        Self { validator_key: None, threshold: DEFAULT_THRESHOLD, participants: Vec::new() }
    }
}

impl ConsensusConfig {
    /// Build the validator set from configured participants.
    ///
    /// Parses the hex-encoded participant public keys into [`ed25519::PublicKey`] values.
    /// Returns an empty set if no participants are configured.
    pub fn build_validator_set(&self) -> Result<Vec<ed25519::PublicKey>, ConfigError> {
        self.participants
            .iter()
            .map(|bytes| {
                if bytes.len() != ed25519::PublicKey::SIZE {
                    return Err(ConfigError::InvalidParticipantKeyLength(bytes.len()));
                }
                let mut buf = bytes.as_slice();
                ed25519::PublicKey::read(&mut buf).map_err(|_| ConfigError::InvalidParticipantKey)
            })
            .collect()
    }
}

const fn default_threshold() -> u32 {
    DEFAULT_THRESHOLD
}

fn serialize_participants<S>(participants: &[Vec<u8>], serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    use serde::ser::SerializeSeq;
    let mut seq = serializer.serialize_seq(Some(participants.len()))?;
    for p in participants {
        seq.serialize_element(&hex::encode(p))?;
    }
    seq.end()
}

fn deserialize_participants<'de, D>(deserializer: D) -> Result<Vec<Vec<u8>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let strings: Vec<String> = Vec::deserialize(deserializer)?;
    strings
        .into_iter()
        .map(|s| hex::decode(s.strip_prefix("0x").unwrap_or(&s)).map_err(serde::de::Error::custom))
        .collect()
}
