//! Monmouth-specific JSON-RPC API implementation.

use std::sync::Arc;

use jsonrpsee::{core::RpcResult, proc_macros::rpc};

use crate::state::{NodeState, NodeStatus};

/// Monmouth-specific JSON-RPC API trait.
///
/// Provides methods specific to Monmouth node operations.
#[rpc(server, namespace = "monmouth")]
pub trait MonmouthApi {
    /// Returns the current node status including consensus information.
    #[method(name = "nodeStatus")]
    async fn node_status(&self) -> RpcResult<NodeStatus>;
}

/// Implementation of the Monmouth RPC API.
#[derive(Debug)]
pub struct MonmouthApiImpl {
    state: Arc<NodeState>,
}

impl MonmouthApiImpl {
    /// Create a new Monmouth API implementation.
    #[must_use]
    pub const fn new(state: Arc<NodeState>) -> Self {
        Self { state }
    }
}

#[jsonrpsee::core::async_trait]
impl MonmouthApiServer for MonmouthApiImpl {
    async fn node_status(&self) -> RpcResult<NodeStatus> {
        Ok(self.state.status())
    }
}
