//! Kora node service implementation.

use commonware_cryptography::Signer;
use commonware_p2p::Manager;
use commonware_runtime::{
    Runner,
    tokio::{self, Context},
};
use futures::future::try_join_all;
use kora_config::NodeConfig;
use kora_transport::NetworkConfigExt;

/// The main kora node service.
#[derive(Debug)]
pub struct KoraNodeService {
    config: NodeConfig,
}

impl KoraNodeService {
    /// Create a new [`KoraNodeService`].
    pub const fn new(config: NodeConfig) -> Self {
        Self { config }
    }

    /// Run the kora node service.
    pub fn run(self) -> eyre::Result<()> {
        let executor = tokio::Runner::default();
        executor.start(|context| async move { self.run_with_context(context).await })
    }

    /// Runs the kora node service with context.
    pub async fn run_with_context(self, context: Context) -> eyre::Result<()> {
        // Load validator identity
        let validator_key = self.config.validator_key()?;
        let validator = validator_key.public_key();
        tracing::info!(?validator, "loaded validator key");

        // Build transport from network config
        let mut transport = self
            .config
            .network
            .build_local_transport(validator_key, context.clone())
            .map_err(|e| eyre::eyre!("failed to build transport: {}", e))?;
        tracing::info!("network transport started");

        // Register validators with oracle
        let validators = self.config.consensus.build_validator_set()?;
        if !validators.is_empty() {
            transport.oracle.update(0, validators.try_into().expect("valid set")).await;
            tracing::info!("registered validators with oracle");
        }

        // TODO: Start simplex consensus engine
        // Requires: scheme, automaton, relay, reporter
        // let engine_handle = DefaultEngine::init(
        //     context.clone(),
        //     "consensus",
        //     scheme,
        //     &transport.oracle,
        //     automaton,
        //     relay,
        //     reporter,
        //     transport.simplex.votes,
        //     transport.simplex.certs,
        //     transport.simplex.resolver,
        // );

        // TODO: Start marshal block dissemination
        // Requires: archives, broadcast engine, peer resolver
        // let marshal_handle = ...

        tracing::info!(chain_id = self.config.chain_id, "kora node initialized");

        // Wait on all handles - service runs until any task fails or completes
        // TODO: Add engine_handle and marshal_handle to the vec
        if let Err(e) = try_join_all(vec![
            transport.handle,
            // engine_handle,
            // marshal_handle,
        ])
        .await
        {
            tracing::error!(?e, "service task failed");
            return Err(eyre::eyre!("service task failed: {:?}", e));
        }

        tracing::info!("kora node shutdown");
        Ok(())
    }
}
