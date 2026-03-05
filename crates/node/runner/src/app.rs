//! REVM-based consensus application implementation.

use std::{
    collections::BTreeSet,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use alloy_consensus::Header;
use alloy_primitives::{Address, B256, Bytes};
use commonware_consensus::{
    Application, Block as _, VerifyingApplication, marshal::ingress::mailbox::AncestorStream,
    simplex::types::Context,
};
use commonware_cryptography::{Committable as _, certificate::Scheme as CertScheme};
use commonware_runtime::{Clock, Metrics, Spawner};
use futures::StreamExt;
use monmouth_consensus::{BlockExecution, SnapshotStore, components::InMemorySnapshotStore};
use monmouth_domain::{Block, ConsensusDigest, StateRoot};
use monmouth_executor::{BlockContext, BlockExecutor};
use monmouth_ledger::LedgerService;
use monmouth_overlay::OverlayState;
use monmouth_qmdb_ledger::QmdbState;
use monmouth_rpc::NodeState;
use monmouth_svm::{SvmExecutor, SvmStateStore};
use rand::Rng;
use tracing::{info, trace, warn};

use crate::routing::partition_by_vm_target;

/// Dual-VM consensus application (REVM + optional SVM).
#[derive(Clone)]
pub struct RevmApplication<S, E> {
    ledger: LedgerService,
    executor: E,
    max_txs: usize,
    gas_limit: u64,
    node_state: Option<NodeState>,
    /// Optional SVM executor (None when SVM is disabled).
    svm_executor: Option<SvmExecutor>,
    /// Optional SVM state store for tracking account state.
    svm_store: Option<SvmStateStore>,
    _scheme: std::marker::PhantomData<S>,
}

impl<S, E> std::fmt::Debug for RevmApplication<S, E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RevmApplication")
            .field("max_txs", &self.max_txs)
            .field("gas_limit", &self.gas_limit)
            .finish_non_exhaustive()
    }
}

impl<S, E> RevmApplication<S, E>
where
    E: BlockExecutor<OverlayState<QmdbState>, Tx = Bytes> + Clone,
{
    /// Create a new REVM application.
    pub const fn new(ledger: LedgerService, executor: E, max_txs: usize, gas_limit: u64) -> Self {
        Self {
            ledger,
            executor,
            max_txs,
            gas_limit,
            node_state: None,
            svm_executor: None,
            svm_store: None,
            _scheme: std::marker::PhantomData,
        }
    }

    /// Set the node state for tracking proposal metrics.
    #[must_use]
    pub fn with_node_state(mut self, state: NodeState) -> Self {
        self.node_state = Some(state);
        self
    }

    /// Enable the SVM executor for dual-VM block building.
    #[must_use]
    pub fn with_svm(mut self, executor: SvmExecutor, store: SvmStateStore) -> Self {
        self.svm_executor = Some(executor);
        self.svm_store = Some(store);
        self
    }

    fn block_context(&self, height: u64, timestamp: u64, prevrandao: B256) -> BlockContext {
        let header = Header {
            number: height,
            timestamp,
            gas_limit: self.gas_limit,
            beneficiary: Address::ZERO,
            base_fee_per_gas: Some(0),
            ..Default::default()
        };
        BlockContext::new(header, B256::ZERO, prevrandao)
    }

    async fn get_prevrandao(&self, parent_digest: ConsensusDigest) -> B256 {
        self.ledger.seed_for_parent(parent_digest).await.unwrap_or(B256::ZERO)
    }

    async fn build_block(&self, parent: &Block) -> Option<Block> {
        use monmouth_consensus::Mempool as _;

        let start = Instant::now();
        let parent_digest = parent.commitment();
        let parent_snapshot = self.ledger.parent_snapshot(parent_digest).await?;
        let snapshot_elapsed = start.elapsed();

        let (_, mempool, snapshots) = self.ledger.proposal_components().await;
        let excluded = self.collect_pending_tx_ids(&snapshots, parent_digest);
        let txs = mempool.build(self.max_txs, &excluded);

        let prevrandao = self.get_prevrandao(parent_digest).await;
        let Some(height) = parent.height.checked_add(1) else {
            warn!("height overflow at {}", parent.height);
            return None;
        };
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        let context = self.block_context(height, timestamp, prevrandao);

        // Partition transactions by VM target
        let partitioned = partition_by_vm_target(&txs);
        let evm_txs_bytes: Vec<Bytes> =
            partitioned.evm.iter().map(|(_, tx)| tx.bytes.clone()).collect();

        let exec_start = Instant::now();
        let outcome =
            self.executor.execute(&parent_snapshot.state, &context, &evm_txs_bytes).ok()?;
        let exec_elapsed = exec_start.elapsed();

        // Execute SVM transactions and compute state root
        let svm_state_root = if let (Some(svm_store), Some(svm_executor)) =
            (&self.svm_store, &self.svm_executor)
        {
            if partitioned.svm.is_empty() {
                None
            } else {
                // Deserialize raw Solana tx bytes → SanitizedTransaction
                let mut sanitized_txs = Vec::new();
                for (_, raw_bytes) in &partitioned.svm {
                    match monmouth_svm::deserialize_svm_tx(raw_bytes) {
                        Ok(stx) => sanitized_txs.push(stx),
                        Err(e) => {
                            warn!(error = ?e, "skipping malformed SVM tx");
                            continue;
                        }
                    }
                }

                if sanitized_txs.is_empty() {
                    None
                } else {
                    let bridge = match svm_store.to_bridge() {
                        Ok(b) => b,
                        Err(e) => {
                            warn!(error = ?e, "SVM bridge creation failed in build_block");
                            return None;
                        }
                    };
                    match svm_executor.execute(
                        &bridge,
                        height,
                        timestamp,
                        &sanitized_txs,
                        Some(prevrandao.0),
                    ) {
                        Ok(svm_outcome) => {
                            let root = match svm_store.compute_root(&svm_outcome.changes) {
                                Ok(r) => r,
                                Err(e) => {
                                    warn!(error = ?e, "SVM root computation failed in build_block");
                                    return None;
                                }
                            };
                            if let Err(e) = svm_store.apply_changes(&svm_outcome.changes) {
                                warn!(error = ?e, "SVM apply_changes failed in build_block");
                                return None;
                            }
                            if root == B256::ZERO { None } else { Some(StateRoot(root)) }
                        }
                        Err(e) => {
                            warn!(error = ?e, "SVM execution failed in build_block");
                            None
                        }
                    }
                }
            }
        } else {
            None
        };

        let root_start = Instant::now();
        let state_root = self
            .ledger
            .compute_root_from_store(parent_digest, outcome.changes.clone())
            .await
            .ok()?;
        let root_elapsed = root_start.elapsed();

        let block = Block {
            parent: parent.id(),
            height,
            timestamp,
            prevrandao,
            state_root,
            svm_state_root,
            txs,
        };

        let merged_changes = parent_snapshot.state.merge_changes(outcome.changes.clone());
        let next_state = OverlayState::new(parent_snapshot.state.base(), merged_changes);
        let block_digest = block.commitment();

        self.ledger
            .insert_snapshot(
                block_digest,
                parent_digest,
                next_state,
                state_root,
                outcome.changes,
                &block.txs,
            )
            .await;
        self.ledger.store_block(block_digest, block.clone()).await;

        let total_elapsed = start.elapsed();
        info!(
            ?block_digest,
            height,
            txs = block.txs.len(),
            snapshot_ms = snapshot_elapsed.as_millis(),
            exec_ms = exec_elapsed.as_millis(),
            root_ms = root_elapsed.as_millis(),
            total_ms = total_elapsed.as_millis(),
            "built block"
        );
        Some(block)
    }

    async fn verify_block(&self, block: &Block) -> bool {
        let start = Instant::now();
        let digest = block.commitment();
        let parent_digest = block.parent();

        if self.ledger.query_state_root(digest).await.is_some() {
            trace!(?digest, "block already verified");
            return true;
        }

        let Some(parent_snapshot) = self.ledger.parent_snapshot(parent_digest).await else {
            warn!(?digest, ?parent_digest, height = block.height, "missing parent snapshot");
            return false;
        };
        let snapshot_elapsed = start.elapsed();

        // Validate timestamp: must not be zero (except genesis), must not be
        // in the far future (10-second tolerance), and must not precede the
        // parent block's timestamp (monotonicity).
        if block.height > 0 {
            let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
            if block.timestamp == 0 {
                warn!(?digest, "block has zero timestamp");
                return false;
            }
            if block.timestamp > now + 10 {
                warn!(
                    ?digest,
                    block_ts = block.timestamp,
                    now,
                    "block timestamp too far in future"
                );
                return false;
            }
            if let Some(parent_block) = self.ledger.query_block(parent_digest).await
                && block.timestamp < parent_block.timestamp
            {
                warn!(
                    ?digest,
                    block_ts = block.timestamp,
                    parent_ts = parent_block.timestamp,
                    "block timestamp precedes parent"
                );
                return false;
            }
        }

        let context = self.block_context(block.height, block.timestamp, block.prevrandao);

        // Partition transactions — only EVM txs go to the EVM executor.
        // Passing SVM-targeted envelope bytes to REVM would cause RLP decode errors.
        let partitioned = partition_by_vm_target(&block.txs);
        let evm_txs: Vec<monmouth_domain::Tx> =
            partitioned.evm.iter().map(|(_, tx)| tx.clone()).collect();

        let exec_start = Instant::now();
        let execution =
            match BlockExecution::execute(&parent_snapshot, &self.executor, &context, &evm_txs)
                .await
            {
                Ok(result) => result,
                Err(err) => {
                    warn!(?digest, error = ?err, "EVM execution failed");
                    return false;
                }
            };
        let exec_elapsed = exec_start.elapsed();

        let root_start = Instant::now();
        let state_root = match self
            .ledger
            .compute_root_from_store(parent_digest, execution.outcome.changes.clone())
            .await
        {
            Ok(root) => root,
            Err(err) => {
                warn!(?digest, error = ?err, "compute root failed");
                return false;
            }
        };
        let root_elapsed = root_start.elapsed();

        if state_root != block.state_root {
            warn!(
                ?digest,
                expected = ?block.state_root,
                computed = ?state_root,
                "state root mismatch"
            );
            return false;
        }

        // Verify SVM state root if present in the block
        if let Some(expected_svm_root) = &block.svm_state_root {
            if let (Some(svm_store), Some(svm_executor)) = (&self.svm_store, &self.svm_executor) {
                // Deserialize SVM txs — any failure means the block is invalid
                let mut sanitized_txs = Vec::new();
                for (_, raw_bytes) in &partitioned.svm {
                    match monmouth_svm::deserialize_svm_tx(raw_bytes) {
                        Ok(stx) => sanitized_txs.push(stx),
                        Err(e) => {
                            warn!(?digest, error = ?e, "invalid SVM tx in block");
                            return false;
                        }
                    }
                }

                let bridge = match svm_store.to_bridge() {
                    Ok(b) => b,
                    Err(e) => {
                        warn!(?digest, error = ?e, "SVM bridge creation failed during verify");
                        return false;
                    }
                };
                let svm_outcome = match svm_executor.execute(
                    &bridge,
                    block.height,
                    block.timestamp,
                    &sanitized_txs,
                    Some(block.prevrandao.0),
                ) {
                    Ok(o) => o,
                    Err(e) => {
                        warn!(?digest, error = ?e, "SVM execution failed during verify");
                        return false;
                    }
                };

                let computed_svm_root = match svm_store.compute_root(&svm_outcome.changes) {
                    Ok(r) => StateRoot(r),
                    Err(e) => {
                        warn!(?digest, error = ?e, "SVM root computation failed during verify");
                        return false;
                    }
                };
                if computed_svm_root != *expected_svm_root {
                    warn!(
                        ?digest,
                        expected = ?expected_svm_root,
                        computed = ?computed_svm_root,
                        "svm state root mismatch"
                    );
                    return false;
                }
                if let Err(e) = svm_store.apply_changes(&svm_outcome.changes) {
                    warn!(?digest, error = ?e, "SVM apply_changes failed during verify");
                    return false;
                }
            } else {
                warn!(?digest, "block has svm_state_root but SVM is not enabled");
                return false;
            }
        }

        let merged_changes = parent_snapshot.state.merge_changes(execution.outcome.changes.clone());
        let next_state = OverlayState::new(parent_snapshot.state.base(), merged_changes);

        self.ledger
            .insert_snapshot(
                digest,
                parent_digest,
                next_state,
                state_root,
                execution.outcome.changes,
                &block.txs,
            )
            .await;
        self.ledger.store_block(digest, block.clone()).await;

        let total_elapsed = start.elapsed();
        info!(
            ?digest,
            height = block.height,
            txs = block.txs.len(),
            snapshot_ms = snapshot_elapsed.as_millis(),
            exec_ms = exec_elapsed.as_millis(),
            root_ms = root_elapsed.as_millis(),
            total_ms = total_elapsed.as_millis(),
            "verified block"
        );
        true
    }

    fn collect_pending_tx_ids(
        &self,
        snapshots: &InMemorySnapshotStore<OverlayState<QmdbState>>,
        from: ConsensusDigest,
    ) -> BTreeSet<monmouth_consensus::TxId> {
        let mut excluded = BTreeSet::new();
        let mut current = Some(from);

        while let Some(digest) = current {
            if snapshots.is_persisted(&digest) {
                break;
            }
            let Some(snapshot) = snapshots.get(&digest) else {
                break;
            };
            excluded.extend(snapshot.tx_ids.iter().copied());
            current = snapshot.parent;
        }

        excluded
    }
}

impl<Env, S, E> Application<Env> for RevmApplication<S, E>
where
    Env: Rng + Spawner + Metrics + Clock,
    S: CertScheme + Send + Sync + 'static,
    E: BlockExecutor<OverlayState<QmdbState>, Tx = Bytes> + Clone + Send + Sync + 'static,
{
    type SigningScheme = S;
    type Context = Context<ConsensusDigest, S::PublicKey>;
    type Block = Block;

    fn genesis(&mut self) -> impl std::future::Future<Output = Self::Block> + Send {
        async move { self.ledger.genesis_block() }
    }

    fn propose(
        &mut self,
        _context: (Env, Self::Context),
        mut ancestry: AncestorStream<Self::SigningScheme, Self::Block>,
    ) -> impl std::future::Future<Output = Option<Self::Block>> + Send {
        let node_state = self.node_state.clone();
        async move {
            let start = Instant::now();
            let parent = ancestry.next().await?;
            let ancestry_elapsed = start.elapsed();

            let build_start = Instant::now();
            let block = self.build_block(&parent).await;
            let build_elapsed = build_start.elapsed();

            if let Some(ref b) = block {
                if let Some(ref state) = node_state {
                    state.inc_proposed();
                }
                info!(
                    height = b.height,
                    ancestry_ms = ancestry_elapsed.as_millis(),
                    build_ms = build_elapsed.as_millis(),
                    total_ms = start.elapsed().as_millis(),
                    "propose complete"
                );
            }

            block
        }
    }
}

impl<Env, S, E> VerifyingApplication<Env> for RevmApplication<S, E>
where
    Env: Rng + Spawner + Metrics + Clock,
    S: CertScheme + Send + Sync + 'static,
    E: BlockExecutor<OverlayState<QmdbState>, Tx = Bytes> + Clone + Send + Sync + 'static,
{
    fn verify(
        &mut self,
        _context: (Env, Self::Context),
        mut ancestry: AncestorStream<Self::SigningScheme, Self::Block>,
    ) -> impl std::future::Future<Output = bool> + Send {
        async move {
            let start = Instant::now();

            // The ancestry stream yields tip-first (newest → oldest).
            // We only need to verify blocks that we haven't seen yet.
            // Collect blocks until we hit one we've already verified.
            let mut blocks_to_verify = Vec::new();
            while let Some(block) = ancestry.next().await {
                let digest = block.commitment();
                // Stop if we've already verified this block
                if self.ledger.query_state_root(digest).await.is_some() {
                    break;
                }
                blocks_to_verify.push(block);
            }
            let ancestry_elapsed = start.elapsed();

            if blocks_to_verify.is_empty() {
                // All blocks already verified
                trace!(ancestry_ms = ancestry_elapsed.as_millis(), "all blocks already verified");
                return true;
            }

            let block_count = blocks_to_verify.len();
            let tip_height = blocks_to_verify.first().map(|b| b.height).unwrap_or(0);

            // Verify from oldest (parent) to newest (tip)
            let verify_start = Instant::now();
            for block in blocks_to_verify.into_iter().rev() {
                if !self.verify_block(&block).await {
                    return false;
                }
            }
            let verify_elapsed = verify_start.elapsed();
            let total_elapsed = start.elapsed();

            info!(
                tip_height,
                block_count,
                ancestry_ms = ancestry_elapsed.as_millis(),
                verify_ms = verify_elapsed.as_millis(),
                total_ms = total_elapsed.as_millis(),
                "verify complete"
            );

            true
        }
    }
}
