//! REVM node runner implementing the NodeRunner trait.

use std::fmt;
use std::time::Duration;

use alloy_consensus::Header;
use alloy_primitives::Address;
use anyhow::Context as _;
use commonware_consensus::{
    Reporters,
    application::marshaled::Marshaled,
    simplex::{self, elector::Random},
    types::{Epoch, FixedEpocher, ViewDelta},
};
use commonware_cryptography::bls12381::primitives::variant::MinSig;
use commonware_p2p::simulated;
use commonware_parallel::Sequential;
use commonware_runtime::{Metrics as _, Spawner as _};
use commonware_utils::{NZU64, NZUsize};
use futures::{StreamExt as _, channel::mpsc};
use kora_domain::{Block, BootstrapConfig, FinalizationEvent, LedgerEvent, PublicKey};
use kora_executor::{BlockContext, RevmExecutor};
use kora_ledger::{LedgerService, LedgerView};
use kora_reporters::{BlockContextProvider, FinalizedReporter, SeedReporter};
use kora_service::{NodeRunContext, NodeRunner};
use kora_transport_sim::{SimContext, register_node_channels};

use crate::{
    app::RevmApplication,
    chain::{BLOCK_GAS_LIMIT, CHAIN_ID},
    handle::NodeHandle,
    node::{
        ThresholdScheme,
        config::{
            EPOCH_LENGTH, MAILBOX_SIZE, PARTITION_PREFIX, block_codec_cfg, default_buffer_pool,
            default_quota,
        },
        marshal::{MarshalStart, start_marshal},
    },
    observers::LedgerObservers,
};

#[derive(Debug)]
pub(crate) struct RunnerError(anyhow::Error);

impl fmt::Display for RunnerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for RunnerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.0.source()
    }
}

impl From<anyhow::Error> for RunnerError {
    fn from(e: anyhow::Error) -> Self {
        Self(e)
    }
}

#[derive(Clone, Debug)]
struct RevmContextProvider {
    gas_limit: u64,
}

impl RevmContextProvider {
    const fn new(gas_limit: u64) -> Self {
        Self { gas_limit }
    }
}

impl BlockContextProvider for RevmContextProvider {
    fn context(&self, block: &Block) -> BlockContext {
        let header = Header {
            number: block.height,
            timestamp: block.height,
            gas_limit: self.gas_limit,
            beneficiary: Address::ZERO,
            base_fee_per_gas: Some(0),
            ..Default::default()
        };
        BlockContext::new(header, block.prevrandao)
    }
}

/// REVM node runner configuration.
#[derive(Clone)]
pub(crate) struct RevmNodeRunner {
    pub(crate) index: usize,
    pub(crate) public_key: PublicKey,
    pub(crate) scheme: ThresholdScheme,
    pub(crate) bootstrap: BootstrapConfig,
    pub(crate) finalized_tx: mpsc::UnboundedSender<FinalizationEvent>,
    pub(crate) manager: simulated::Manager<PublicKey, SimContext>,
}

impl NodeRunner for RevmNodeRunner {
    type Transport = simulated::Control<PublicKey, SimContext>;
    type Handle = NodeHandle;
    type Error = RunnerError;

    async fn run(
        &self,
        ctx: NodeRunContext<Self::Transport>,
    ) -> Result<Self::Handle, Self::Error> {
        let (context, _config, mut control) = ctx.into_parts();

        let quota = default_quota();
        let buffer_pool = default_buffer_pool();
        let partition_prefix = PARTITION_PREFIX;
        let index = self.index;

        let blocker = control.clone();

        let channels = register_node_channels(&mut control, quota)
            .await
            .map_err(|e| anyhow::anyhow!("channel registration failed: {e}"))?;

        let block_cfg = block_codec_cfg();
        let state = LedgerView::init(
            context.with_label(&format!("state_{index}")),
            buffer_pool.clone(),
            format!("{partition_prefix}-qmdb-{index}"),
            self.bootstrap.genesis_alloc.clone(),
        )
        .await
        .context("init qmdb")?;

        let ledger = LedgerService::new(state.clone());
        LedgerObservers::spawn(ledger.clone(), context.clone());
        let mut domain_events = ledger.subscribe();
        let finalized_tx_clone = self.finalized_tx.clone();
        let node_id = index as u32;
        let event_context = context.clone();
        event_context.spawn(move |_| async move {
            while let Some(event) = domain_events.next().await {
                if let LedgerEvent::SnapshotPersisted(digest) = event {
                    let _ = finalized_tx_clone.unbounded_send((node_id, digest));
                }
            }
        });
        let handle = NodeHandle::new(ledger.clone());
        let app = RevmApplication::<ThresholdScheme>::new(block_cfg.max_txs, state.clone());

        let executor = RevmExecutor::new(CHAIN_ID);
        let context_provider = RevmContextProvider::new(BLOCK_GAS_LIMIT);
        let finalized_reporter =
            FinalizedReporter::new(ledger.clone(), context.clone(), executor, context_provider);

        let marshal_mailbox = start_marshal(
            &context,
            MarshalStart {
                index,
                partition_prefix: partition_prefix.to_string(),
                public_key: self.public_key.clone(),
                control: control.clone(),
                manager: self.manager.clone(),
                scheme: self.scheme.clone(),
                buffer_pool: buffer_pool.clone(),
                block_codec_config: block_cfg,
                blocks: channels.marshal.blocks,
                backfill: channels.marshal.backfill,
                application: finalized_reporter,
            },
        )
        .await?;

        let epocher = FixedEpocher::new(NZU64!(EPOCH_LENGTH));
        let marshaled = Marshaled::new(
            context.with_label(&format!("marshaled_{index}")),
            app,
            marshal_mailbox.clone(),
            epocher,
        );

        let seed_reporter = SeedReporter::<MinSig>::new(ledger.clone());
        let reporter = Reporters::from((seed_reporter, marshal_mailbox.clone()));

        for tx in &self.bootstrap.bootstrap_txs {
            let _ = handle.submit_tx(tx.clone()).await;
        }

        let engine = simplex::Engine::new(
            context.with_label(&format!("engine_{index}")),
            simplex::Config {
                scheme: self.scheme.clone(),
                elector: Random,
                blocker,
                automaton: marshaled.clone(),
                relay: marshaled,
                reporter,
                strategy: Sequential,
                partition: format!("{partition_prefix}-{index}"),
                mailbox_size: MAILBOX_SIZE,
                epoch: Epoch::zero(),
                replay_buffer: NZUsize!(1024 * 1024),
                write_buffer: NZUsize!(1024 * 1024),
                leader_timeout: Duration::from_secs(1),
                notarization_timeout: Duration::from_secs(2),
                nullify_retry: Duration::from_secs(5),
                fetch_timeout: Duration::from_secs(1),
                activity_timeout: ViewDelta::new(20),
                skip_timeout: ViewDelta::new(10),
                fetch_concurrent: 8,
                buffer_pool,
            },
        );
        engine.start(
            channels.simplex.votes,
            channels.simplex.certs,
            channels.simplex.resolver,
        );

        Ok(handle)
    }
}
