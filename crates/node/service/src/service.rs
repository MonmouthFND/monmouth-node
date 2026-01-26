//! Kora node service implementation.

use kora_config::NodeConfig;

/// The main kora node service.
///
/// This service orchestrates all node components including:
/// - Consensus engine (simplex)
/// - Execution layer (revm)
/// - Network layer (p2p, rpc)
/// - Storage backend
#[derive(Debug)]
pub struct KoraNodeService {
    /// The node configuration.
    config: NodeConfig,
}

impl KoraNodeService {
    /// Create a new [`KoraNodeService`] with the given configuration.
    pub const fn new(config: NodeConfig) -> Self {
        Self { config }
    }

    /// Run the kora node service.
    ///
    /// This method starts all node components and blocks until shutdown.
    pub async fn run(self) -> eyre::Result<()> {
        tracing::info!(chain_id = self.config.chain_id, "Starting kora node service");

        // TODO: Initialize and run node components:
        // - Consensus engine (simplex)
        // - Execution layer (revm)
        // - Network layer (p2p, rpc)
        // - Storage backend

        tracing::info!("Kora node service started (stub)");

        Ok(())
    }
}
