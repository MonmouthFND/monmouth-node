//! Monmouth-specific JSON-RPC API implementation.

use std::sync::Arc;

use jsonrpsee::{core::RpcResult, proc_macros::rpc, types::ErrorObject};
use monmouth_capabilities::{Capability, CapabilityRegistry, CapabilitySchema, CapabilitySummary};
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
        Ok(self.svm_store.as_ref().map_or(
            SvmStatus { enabled: false, account_count: 0 },
            |store| SvmStatus { enabled: true, account_count: store.len() as u64 },
        ))
    }

    async fn svm_get_account(&self, pubkey: String) -> RpcResult<Option<SvmAccountInfo>> {
        let store = self
            .svm_store
            .as_ref()
            .ok_or_else(|| ErrorObject::owned(-32890, "SVM module is not enabled", None::<()>))?;
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
        let store = self
            .svm_store
            .as_ref()
            .ok_or_else(|| ErrorObject::owned(-32890, "SVM module is not enabled", None::<()>))?;
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

#[cfg(test)]
mod tests {
    use monmouth_capabilities::{Capability, CapabilitySchema, Permission, PermissionKind};
    use monmouth_svm::{SvmAccountUpdate, SvmChangeSet};

    use super::*;

    fn test_capability(id: &str) -> Capability {
        Capability {
            id: id.to_string(),
            name: format!("Test {id}"),
            description: "A test capability".to_string(),
            version: "1.0.0".to_string(),
            schema: CapabilitySchema {
                input: serde_json::json!({"type": "object"}),
                output: serde_json::json!({"type": "string"}),
            },
            permissions: vec![Permission { kind: PermissionKind::Execute, scope: "*".to_string() }],
            rate_limit: None,
            enabled: true,
        }
    }

    fn make_api() -> MonmouthApiImpl {
        let state = Arc::new(NodeState::new(7750, 0));
        let capabilities = CapabilityRegistry::default();
        MonmouthApiImpl::new(state, capabilities)
    }

    fn make_api_with_caps(caps: Vec<Capability>) -> MonmouthApiImpl {
        let state = Arc::new(NodeState::new(7750, 0));
        let capabilities = CapabilityRegistry::from_capabilities(caps).unwrap();
        MonmouthApiImpl::new(state, capabilities)
    }

    fn make_svm_store_with_account(pubkey: [u8; 32], update: SvmAccountUpdate) -> SvmStateStore {
        let store = SvmStateStore::new();
        let mut changes = SvmChangeSet::new();
        changes.insert(pubkey, update);
        store.apply_changes(&changes).expect("test store apply");
        store
    }

    // --- node_status ---

    #[tokio::test]
    async fn node_status_returns_chain_id() {
        let state = Arc::new(NodeState::new(7750, 2));
        let api = MonmouthApiImpl::new(state, CapabilityRegistry::default());
        let status = api.node_status().await.unwrap();
        assert_eq!(status.chain_id, 7750);
        assert_eq!(status.validator_index, 2);
    }

    // --- list_capabilities ---

    #[tokio::test]
    async fn list_capabilities_empty() {
        let api = make_api();
        let list = api.list_capabilities().await.unwrap();
        assert!(list.is_empty());
    }

    #[tokio::test]
    async fn list_capabilities_with_entries() {
        let api =
            make_api_with_caps(vec![test_capability("cap.alpha"), test_capability("cap.beta")]);
        let list = api.list_capabilities().await.unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].id, "cap.alpha");
        assert_eq!(list[1].id, "cap.beta");
    }

    // --- get_capability ---

    #[tokio::test]
    async fn get_capability_found() {
        let api = make_api_with_caps(vec![test_capability("cap.one")]);
        let cap = api.get_capability("cap.one".to_string()).await.unwrap();
        assert!(cap.is_some());
        assert_eq!(cap.unwrap().id, "cap.one");
    }

    #[tokio::test]
    async fn get_capability_not_found() {
        let api = make_api();
        let cap = api.get_capability("missing.cap".to_string()).await.unwrap();
        assert!(cap.is_none());
    }

    // --- get_capability_schema ---

    #[tokio::test]
    async fn get_capability_schema_found() {
        let api = make_api_with_caps(vec![test_capability("cap.schema")]);
        let schema = api.get_capability_schema("cap.schema".to_string()).await.unwrap();
        assert!(schema.is_some());
        assert_eq!(schema.unwrap().input, serde_json::json!({"type": "object"}));
    }

    #[tokio::test]
    async fn get_capability_schema_not_found() {
        let api = make_api();
        let schema = api.get_capability_schema("missing".to_string()).await.unwrap();
        assert!(schema.is_none());
    }

    // --- svm_status ---

    #[tokio::test]
    async fn svm_status_disabled() {
        let api = make_api();
        let status = api.svm_status().await.unwrap();
        assert!(!status.enabled);
        assert_eq!(status.account_count, 0);
    }

    #[tokio::test]
    async fn svm_status_enabled() {
        let store = make_svm_store_with_account(
            [1u8; 32],
            SvmAccountUpdate {
                lamports: 100,
                data: vec![],
                owner: [0u8; 32],
                executable: false,
                rent_epoch: 0,
            },
        );
        let api = make_api().with_svm_store(store);
        let status = api.svm_status().await.unwrap();
        assert!(status.enabled);
        assert_eq!(status.account_count, 1);
    }

    // --- svm_get_account ---

    #[tokio::test]
    async fn svm_get_account_found() {
        let pubkey = [0xAB; 32];
        let store = make_svm_store_with_account(
            pubkey,
            SvmAccountUpdate {
                lamports: 5000,
                data: vec![1, 2, 3],
                owner: [0xFF; 32],
                executable: false,
                rent_epoch: 42,
            },
        );
        let api = make_api().with_svm_store(store);
        let hex_key = format!("0x{}", hex::encode(pubkey));
        let acct = api.svm_get_account(hex_key).await.unwrap();
        assert!(acct.is_some());
        let acct = acct.unwrap();
        assert_eq!(acct.lamports, 5000);
        assert_eq!(acct.data_len, 3);
        assert!(!acct.executable);
        assert_eq!(acct.rent_epoch, 42);
    }

    #[tokio::test]
    async fn svm_get_account_not_found() {
        let store = SvmStateStore::new();
        let api = make_api().with_svm_store(store);
        let hex_key = format!("0x{}", hex::encode([0x99; 32]));
        let acct = api.svm_get_account(hex_key).await.unwrap();
        assert!(acct.is_none());
    }

    #[tokio::test]
    async fn svm_get_account_disabled() {
        let api = make_api();
        let hex_key = format!("0x{}", hex::encode([0x99; 32]));
        let err = api.svm_get_account(hex_key).await.unwrap_err();
        assert_eq!(err.code(), -32890);
    }

    // --- svm_get_program_info ---

    #[tokio::test]
    async fn svm_get_program_info_program() {
        let pubkey = [0xCC; 32];
        let store = make_svm_store_with_account(
            pubkey,
            SvmAccountUpdate {
                lamports: 1_000_000,
                data: vec![0xBF; 256],
                owner: [0xEE; 32],
                executable: true,
                rent_epoch: 0,
            },
        );
        let api = make_api().with_svm_store(store);
        let hex_key = format!("0x{}", hex::encode(pubkey));
        let info = api.svm_get_program_info(hex_key).await.unwrap();
        assert!(info.is_program);
        assert_eq!(info.data_len, 256);
    }

    #[tokio::test]
    async fn svm_get_program_info_not_found() {
        let store = SvmStateStore::new();
        let api = make_api().with_svm_store(store);
        let hex_key = format!("0x{}", hex::encode([0xDD; 32]));
        let info = api.svm_get_program_info(hex_key).await.unwrap();
        assert!(!info.is_program);
        assert_eq!(info.data_len, 0);
    }

    #[tokio::test]
    async fn svm_get_program_info_disabled() {
        let api = make_api();
        let hex_key = format!("0x{}", hex::encode([0xDD; 32]));
        let err = api.svm_get_program_info(hex_key).await.unwrap_err();
        assert_eq!(err.code(), -32890);
    }

    // --- parse_pubkey ---

    #[test]
    fn parse_pubkey_valid_with_prefix() {
        let key = [0xAB; 32];
        let hex_str = format!("0x{}", hex::encode(key));
        let parsed = MonmouthApiImpl::parse_pubkey(&hex_str).unwrap();
        assert_eq!(parsed, key);
    }

    #[test]
    fn parse_pubkey_valid_without_prefix() {
        let key = [0xCD; 32];
        let hex_str = hex::encode(key);
        let parsed = MonmouthApiImpl::parse_pubkey(&hex_str).unwrap();
        assert_eq!(parsed, key);
    }

    #[test]
    fn parse_pubkey_invalid_hex() {
        let err = MonmouthApiImpl::parse_pubkey("0xZZZZ").unwrap_err();
        assert_eq!(err.code(), -32602);
    }

    #[test]
    fn parse_pubkey_wrong_length() {
        let err = MonmouthApiImpl::parse_pubkey("0xaabb").unwrap_err();
        assert_eq!(err.code(), -32602);
    }
}
