//! Monmouth-specific JSON-RPC API implementation.

use std::sync::Arc;

use jsonrpsee::{core::RpcResult, proc_macros::rpc, types::ErrorObject};
use monmouth_capabilities::{
    Capability, CapabilityRegistry, CapabilitySchema, CapabilitySummary,
};
use monmouth_svm::SvmStateStore;

use crate::state::{NodeState, NodeStatus};

/// SVM module status.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SvmStatus {
    /// Whether the SVM module is enabled.
    pub enabled: bool,
    /// Number of tracked SVM accounts.
    pub account_count: u64,
}

/// SVM account information returned by RPC.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SvmAccountInfo {
    /// Hex-encoded 32-byte public key.
    pub pubkey: String,
    /// Account balance in lamports.
    pub lamports: u64,
    /// Length of account data in bytes.
    pub data_len: u64,
    /// Hex-encoded 32-byte owner public key.
    pub owner: String,
    /// Whether this account is executable (a program).
    pub executable: bool,
    /// Rent epoch.
    pub rent_epoch: u64,
}

/// SVM program information.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SvmProgramInfo {
    /// Hex-encoded 32-byte public key.
    pub pubkey: String,
    /// Whether this account is a deployed program.
    pub is_program: bool,
    /// Hex-encoded owner public key (BPF loader for programs).
    pub owner: String,
    /// Length of program data.
    pub data_len: u64,
}

/// Monmouth-specific JSON-RPC API trait.
///
/// Provides methods specific to Monmouth node operations.
#[rpc(server, namespace = "monmouth")]
pub trait MonmouthApi {
    /// Returns the current node status including consensus information.
    #[method(name = "nodeStatus")]
    async fn node_status(&self) -> RpcResult<NodeStatus>;

    /// List all registered capabilities (lightweight summaries).
    #[method(name = "listCapabilities")]
    async fn list_capabilities(&self) -> RpcResult<Vec<CapabilitySummary>>;

    /// Get the full details of a capability by ID.
    #[method(name = "getCapability")]
    async fn get_capability(&self, id: String) -> RpcResult<Option<Capability>>;

    /// Get only the typed schema for a capability by ID.
    #[method(name = "getCapabilitySchema")]
    async fn get_capability_schema(&self, id: String) -> RpcResult<Option<CapabilitySchema>>;

    /// Returns the SVM module status.
    #[method(name = "svmStatus")]
    async fn svm_status(&self) -> RpcResult<SvmStatus>;

    /// Get an SVM account by hex-encoded 32-byte public key.
    #[method(name = "svmGetAccount")]
    async fn svm_get_account(&self, pubkey: String) -> RpcResult<Option<SvmAccountInfo>>;

    /// Get program information for an SVM account.
    #[method(name = "svmGetProgramInfo")]
    async fn svm_get_program_info(&self, pubkey: String) -> RpcResult<SvmProgramInfo>;
}

/// Implementation of the Monmouth RPC API.
#[derive(Debug)]
pub struct MonmouthApiImpl {
    state: Arc<NodeState>,
    capabilities: CapabilityRegistry,
    svm_store: Option<SvmStateStore>,
}

impl MonmouthApiImpl {
    /// Create a new Monmouth API implementation.
    #[must_use]
    pub const fn new(state: Arc<NodeState>, capabilities: CapabilityRegistry) -> Self {
        Self { state, capabilities, svm_store: None }
    }

    /// Enable SVM account queries via the given store.
    #[must_use]
    pub fn with_svm_store(mut self, store: SvmStateStore) -> Self {
        self.svm_store = Some(store);
        self
    }

    /// Parse a hex-encoded 32-byte public key.
    fn parse_pubkey(hex_str: &str) -> RpcResult<[u8; 32]> {
        let hex_str = hex_str.strip_prefix("0x").unwrap_or(hex_str);
        let bytes = hex::decode(hex_str).map_err(|e| {
            ErrorObject::owned(-32602, format!("invalid pubkey hex: {e}"), None::<()>)
        })?;
        if bytes.len() != 32 {
            return Err(ErrorObject::owned(
                -32602,
                format!("pubkey must be 32 bytes, got {}", bytes.len()),
                None::<()>,
            ));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(arr)
    }
}

#[jsonrpsee::core::async_trait]
impl MonmouthApiServer for MonmouthApiImpl {
    async fn node_status(&self) -> RpcResult<NodeStatus> {
        Ok(self.state.status())
    }

    async fn list_capabilities(&self) -> RpcResult<Vec<CapabilitySummary>> {
        Ok(self.capabilities.list())
    }

    async fn get_capability(&self, id: String) -> RpcResult<Option<Capability>> {
        Ok(self.capabilities.get(&id))
    }

    async fn get_capability_schema(&self, id: String) -> RpcResult<Option<CapabilitySchema>> {
        Ok(self.capabilities.get_schema(&id))
    }

    async fn svm_status(&self) -> RpcResult<SvmStatus> {
        match &self.svm_store {
            Some(store) => Ok(SvmStatus { enabled: true, account_count: store.len() as u64 }),
            None => Ok(SvmStatus { enabled: false, account_count: 0 }),
        }
    }

    async fn svm_get_account(&self, pubkey: String) -> RpcResult<Option<SvmAccountInfo>> {
        let store = self.svm_store.as_ref().ok_or_else(|| {
            ErrorObject::owned(-32890, "SVM module is not enabled", None::<()>)
        })?;
        let key = Self::parse_pubkey(&pubkey)?;
        Ok(store.get_account(&key).map(|acct| SvmAccountInfo {
            pubkey: format!("0x{}", hex::encode(key)),
            lamports: acct.lamports,
            data_len: acct.data.len() as u64,
            owner: format!("0x{}", hex::encode(acct.owner)),
            executable: acct.executable,
            rent_epoch: acct.rent_epoch,
        }))
    }

    async fn svm_get_program_info(&self, pubkey: String) -> RpcResult<SvmProgramInfo> {
        let store = self.svm_store.as_ref().ok_or_else(|| {
            ErrorObject::owned(-32890, "SVM module is not enabled", None::<()>)
        })?;
        let key = Self::parse_pubkey(&pubkey)?;
        match store.get_account(&key) {
            Some(acct) => Ok(SvmProgramInfo {
                pubkey: format!("0x{}", hex::encode(key)),
                is_program: acct.executable,
                owner: format!("0x{}", hex::encode(acct.owner)),
                data_len: acct.data.len() as u64,
            }),
            None => Ok(SvmProgramInfo {
                pubkey: format!("0x{}", hex::encode(key)),
                is_program: false,
                owner: String::new(),
                data_len: 0,
            }),
        }
    }
}
