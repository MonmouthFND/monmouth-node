//! Ethereum pub/sub API implementation (WebSocket subscriptions).
//!
//! Provides `eth_subscribe` and `eth_unsubscribe` for real-time event streaming
//! over WebSocket connections. Supported subscription types:
//!
//! - `newHeads` — emits a block header on each new block
//! - `logs` — emits matching log entries as they are produced
//! - `newPendingTransactions` — emits pending transaction hashes
//! - `syncing` — emits sync status changes

use std::sync::Arc;

use jsonrpsee::{
    PendingSubscriptionSink, SubscriptionMessage, core::SubscriptionResult, proc_macros::rpc,
    types::ErrorObjectOwned,
};
use tokio::sync::broadcast;
use tracing::{debug, warn};

use crate::{
    error::codes,
    types::{RpcBlock, RpcLog, RpcLogFilter},
};

/// Broadcast channels for real-time chain events.
///
/// Clone this and hand it to both the RPC layer (subscriber side) and the
/// block-production / import pipeline (publisher side).
#[derive(Clone, Debug)]
pub struct EventBroadcaster {
    /// New block headers.
    new_heads: broadcast::Sender<Arc<RpcBlock>>,
    /// New logs (emitted per-log, already filtered by the producer if applicable).
    logs: broadcast::Sender<Arc<RpcLog>>,
    /// New pending transaction hashes.
    pending_txs: broadcast::Sender<[u8; 32]>,
}

impl Default for EventBroadcaster {
    fn default() -> Self {
        Self::new(1024)
    }
}

impl EventBroadcaster {
    /// Create a new broadcaster with the given channel capacity.
    pub fn new(capacity: usize) -> Self {
        let (new_heads, _) = broadcast::channel(capacity);
        let (logs, _) = broadcast::channel(capacity);
        let (pending_txs, _) = broadcast::channel(capacity);
        Self { new_heads, logs, pending_txs }
    }

    /// Publish a new block header. Called from the block production pipeline.
    pub fn send_new_head(&self, block: RpcBlock) {
        // Ignore send errors (no active subscribers).
        let _ = self.new_heads.send(Arc::new(block));
    }

    /// Publish a new log. Called from the block production pipeline.
    pub fn send_log(&self, log: RpcLog) {
        let _ = self.logs.send(Arc::new(log));
    }

    /// Publish a new pending transaction hash. Called from the mempool.
    pub fn send_pending_tx(&self, hash: [u8; 32]) {
        let _ = self.pending_txs.send(hash);
    }
}

/// Ethereum pub/sub subscription methods.
#[rpc(server, namespace = "eth")]
pub trait EthPubSubApi {
    /// Create a subscription. Returns a subscription ID.
    ///
    /// `kind` must be one of: `newHeads`, `logs`, `newPendingTransactions`, `syncing`.
    /// `params` is optional and only used for `logs` (provides a filter).
    #[subscription(name = "subscribe" => "subscription", unsubscribe = "unsubscribe", item = serde_json::Value)]
    async fn subscribe(
        &self,
        kind: String,
        params: Option<serde_json::Value>,
    ) -> SubscriptionResult;
}

/// Implementation of the Ethereum pub/sub API.
pub struct EthPubSubApiImpl {
    broadcaster: EventBroadcaster,
}

impl std::fmt::Debug for EthPubSubApiImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EthPubSubApiImpl").finish()
    }
}

impl EthPubSubApiImpl {
    /// Create a new pub/sub API implementation.
    pub const fn new(broadcaster: EventBroadcaster) -> Self {
        Self { broadcaster }
    }
}

#[jsonrpsee::core::async_trait]
impl EthPubSubApiServer for EthPubSubApiImpl {
    async fn subscribe(
        &self,
        pending: PendingSubscriptionSink,
        kind: String,
        params: Option<serde_json::Value>,
    ) -> SubscriptionResult {
        match kind.as_str() {
            "newHeads" => {
                let sink = pending.accept().await?;
                let mut rx = self.broadcaster.new_heads.subscribe();
                tokio::spawn(async move {
                    loop {
                        match rx.recv().await {
                            Ok(block) => {
                                let msg = SubscriptionMessage::from_json(&*block)
                                    .expect("RpcBlock is serializable");
                                if sink.send(msg).await.is_err() {
                                    break;
                                }
                            }
                            Err(broadcast::error::RecvError::Lagged(n)) => {
                                warn!(skipped = n, "newHeads subscriber lagged");
                            }
                            Err(broadcast::error::RecvError::Closed) => break,
                        }
                    }
                    debug!("newHeads subscription closed");
                });
                Ok(())
            }
            "logs" => {
                // Parse optional log filter from params.
                let filter: Option<RpcLogFilter> =
                    params.and_then(|v| serde_json::from_value(v).ok());

                let sink = pending.accept().await?;
                let mut rx = self.broadcaster.logs.subscribe();
                tokio::spawn(async move {
                    loop {
                        match rx.recv().await {
                            Ok(log) => {
                                if !matches_log_filter(&log, filter.as_ref()) {
                                    continue;
                                }
                                let msg = SubscriptionMessage::from_json(&*log)
                                    .expect("RpcLog is serializable");
                                if sink.send(msg).await.is_err() {
                                    break;
                                }
                            }
                            Err(broadcast::error::RecvError::Lagged(n)) => {
                                warn!(skipped = n, "logs subscriber lagged");
                            }
                            Err(broadcast::error::RecvError::Closed) => break,
                        }
                    }
                    debug!("logs subscription closed");
                });
                Ok(())
            }
            "newPendingTransactions" => {
                let sink = pending.accept().await?;
                let mut rx = self.broadcaster.pending_txs.subscribe();
                tokio::spawn(async move {
                    loop {
                        match rx.recv().await {
                            Ok(hash) => {
                                let hex = format!("0x{}", hex::encode(hash));
                                let msg = SubscriptionMessage::from_json(&hex)
                                    .expect("string is serializable");
                                if sink.send(msg).await.is_err() {
                                    break;
                                }
                            }
                            Err(broadcast::error::RecvError::Lagged(n)) => {
                                warn!(skipped = n, "pendingTx subscriber lagged");
                            }
                            Err(broadcast::error::RecvError::Closed) => break,
                        }
                    }
                    debug!("newPendingTransactions subscription closed");
                });
                Ok(())
            }
            "syncing" => {
                // Syncing is a simple boolean for now (not syncing).
                let sink = pending.accept().await?;
                let msg = SubscriptionMessage::from_json(&false).expect("bool is serializable");
                // Send initial status, then the subscription stays open but idle.
                let _ = sink.send(msg).await;
                Ok(())
            }
            _ => {
                pending
                    .reject(ErrorObjectOwned::owned(
                        codes::INVALID_PARAMS,
                        format!("unsupported subscription type: {kind}"),
                        None::<()>,
                    ))
                    .await;
                Ok(())
            }
        }
    }
}

/// Check if a log matches the subscription's optional filter.
fn matches_log_filter(log: &RpcLog, filter: Option<&RpcLogFilter>) -> bool {
    let Some(filter) = filter else {
        return true; // No filter = all logs match.
    };

    // Check address filter.
    if let Some(ref addr_filter) = filter.address {
        let addrs = match addr_filter {
            crate::types::AddressFilter::Single(a) => vec![*a],
            crate::types::AddressFilter::Multiple(a) => a.clone(),
        };
        if !addrs.contains(&log.address) {
            return false;
        }
    }

    // Check topic filters (AND across positions, OR within each position).
    if let Some(ref topics) = filter.topics {
        for (i, topic_filter) in topics.iter().enumerate() {
            if let Some(tf) = topic_filter {
                let required = match tf {
                    crate::types::TopicFilter::Single(t) => vec![*t],
                    crate::types::TopicFilter::Multiple(t) => t.clone(),
                };
                // If the log doesn't have this topic position, it doesn't match.
                match log.topics.get(i) {
                    Some(log_topic) => {
                        if !required.contains(log_topic) {
                            return false;
                        }
                    }
                    None => return false,
                }
            }
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use alloy_primitives::{Address, B256, Bytes, U64};

    use super::*;

    fn make_log(address: Address, topics: Vec<B256>) -> RpcLog {
        RpcLog {
            address,
            topics,
            data: Bytes::new(),
            block_number: U64::from(1),
            transaction_hash: B256::ZERO,
            transaction_index: U64::ZERO,
            block_hash: B256::ZERO,
            log_index: U64::ZERO,
            removed: false,
        }
    }

    #[test]
    fn matches_no_filter() {
        let log = make_log(Address::ZERO, vec![]);
        assert!(matches_log_filter(&log, None));
    }

    #[test]
    fn matches_address_filter() {
        let addr = Address::repeat_byte(0x42);
        let log = make_log(addr, vec![]);
        let filter = RpcLogFilter {
            address: Some(crate::types::AddressFilter::Single(addr)),
            ..Default::default()
        };
        assert!(matches_log_filter(&log, Some(&filter)));

        let wrong_addr = Address::repeat_byte(0x01);
        let log2 = make_log(wrong_addr, vec![]);
        assert!(!matches_log_filter(&log2, Some(&filter)));
    }

    #[test]
    fn matches_topic_filter() {
        let topic = B256::repeat_byte(0xab);
        let log = make_log(Address::ZERO, vec![topic]);
        let filter = RpcLogFilter {
            topics: Some(vec![Some(crate::types::TopicFilter::Single(topic))]),
            ..Default::default()
        };
        assert!(matches_log_filter(&log, Some(&filter)));

        let wrong_topic = B256::repeat_byte(0x01);
        let log2 = make_log(Address::ZERO, vec![wrong_topic]);
        assert!(!matches_log_filter(&log2, Some(&filter)));
    }

    #[test]
    fn matches_null_topic_position() {
        let topic = B256::repeat_byte(0xab);
        let log = make_log(Address::ZERO, vec![topic]);
        // null in position 0 means "any topic"
        let filter = RpcLogFilter { topics: Some(vec![None]), ..Default::default() };
        assert!(matches_log_filter(&log, Some(&filter)));
    }

    #[test]
    fn broadcaster_default() {
        let b = EventBroadcaster::default();
        // Should not panic when sending with no subscribers.
        b.send_new_head(RpcBlock::default());
        b.send_log(RpcLog::default());
        b.send_pending_tx([0u8; 32]);
    }

    #[test]
    fn broadcaster_new_heads_received() {
        let b = EventBroadcaster::new(16);
        let mut rx = b.new_heads.subscribe();
        b.send_new_head(RpcBlock::default());
        let block = rx.try_recv().unwrap();
        assert_eq!(block.number, U64::ZERO);
    }

    #[test]
    fn broadcaster_logs_received() {
        let b = EventBroadcaster::new(16);
        let mut rx = b.logs.subscribe();
        let addr = Address::repeat_byte(0x42);
        b.send_log(RpcLog { address: addr, ..Default::default() });
        let log = rx.try_recv().unwrap();
        assert_eq!(log.address, addr);
    }

    #[test]
    fn broadcaster_pending_tx_received() {
        let b = EventBroadcaster::new(16);
        let mut rx = b.pending_txs.subscribe();
        let hash = [0xffu8; 32];
        b.send_pending_tx(hash);
        let received = rx.try_recv().unwrap();
        assert_eq!(received, hash);
    }
}
