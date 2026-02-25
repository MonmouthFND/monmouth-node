//! Ethereum filter/polling API implementation.
//!
//! Provides `eth_newFilter`, `eth_newBlockFilter`, `eth_newPendingTransactionFilter`,
//! `eth_getFilterChanges`, `eth_getFilterLogs`, and `eth_uninstallFilter`.

use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use alloy_primitives::{U64, U256};
use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use parking_lot::Mutex;
use tracing::debug;

use crate::{
    error::{RpcError, codes},
    state_provider::StateProvider,
    types::{BlockNumberOrTag, RpcLog, RpcLogFilter},
};

/// Configuration for the filter manager.
#[derive(Clone, Debug)]
pub struct FilterConfig {
    /// How long a filter may go without being polled before removal (default: 5 min).
    pub filter_ttl: Duration,
    /// Maximum logs returned per `getFilterChanges`/`getFilterLogs` call.
    pub max_logs_per_response: usize,
    /// Maximum concurrent active filters.
    pub max_filters: usize,
    /// How often the background expiry sweep runs.
    pub sweep_interval: Duration,
}

impl Default for FilterConfig {
    fn default() -> Self {
        Self {
            filter_ttl: Duration::from_secs(300),
            max_logs_per_response: 10_000,
            max_filters: 10_000,
            sweep_interval: Duration::from_secs(30),
        }
    }
}

/// The type of an active filter.
#[derive(Clone, Debug)]
enum FilterKind {
    /// Log filter with creation-time criteria.
    Log(RpcLogFilter),
    /// Block filter (returns new block hashes).
    Block,
    /// Pending transaction filter (returns new tx hashes).
    PendingTransaction,
}

/// One installed filter entry.
struct ActiveFilter {
    kind: FilterKind,
    /// Next block to include in `getFilterChanges`. Set to `head + 1` at creation.
    next_block: u64,
    /// Last time this filter was polled (or creation time).
    last_polled: Instant,
}

/// Thread-safe filter registry.
#[derive(Clone, Default)]
struct FilterRegistry {
    inner: Arc<Mutex<HashMap<U256, ActiveFilter>>>,
}

impl FilterRegistry {
    fn install(&self, kind: FilterKind, next_block: u64, max_filters: usize) -> Option<U256> {
        let mut map = self.inner.lock();
        if map.len() >= max_filters {
            return None;
        }
        let id = generate_filter_id();
        map.insert(id, ActiveFilter { kind, next_block, last_polled: Instant::now() });
        Some(id)
    }

    fn uninstall(&self, id: U256) -> bool {
        self.inner.lock().remove(&id).is_some()
    }

    fn sweep_expired(&self, ttl: Duration) {
        let now = Instant::now();
        let mut map = self.inner.lock();
        let before = map.len();
        map.retain(|_, f| now.duration_since(f.last_polled) < ttl);
        let removed = before - map.len();
        if removed > 0 {
            debug!(removed, remaining = map.len(), "swept expired filters");
        }
    }

    /// Advance watermark, return (kind, old_next_block). Returns None if filter not found.
    fn advance_watermark(&self, id: U256, current_head: u64) -> Option<(FilterKind, u64)> {
        let mut map = self.inner.lock();
        let filter = map.get_mut(&id)?;
        let old_next = filter.next_block;
        filter.next_block = current_head.saturating_add(1);
        filter.last_polled = Instant::now();
        Some((filter.kind.clone(), old_next))
    }

    /// Get filter kind without modifying watermark (for `getFilterLogs`).
    fn get_kind(&self, id: U256) -> Option<FilterKind> {
        let map = self.inner.lock();
        map.get(&id).map(|f| f.kind.clone())
    }
}

/// Generate a random 128-bit filter ID as U256.
fn generate_filter_id() -> U256 {
    use std::time::SystemTime;

    // Simple unique ID using timestamp + counter to avoid adding rand dependency.
    // For production public nodes, replace with crypto-random.
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let ts = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_nanos()
        as u64;
    let count = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    U256::from(ts) << 64 | U256::from(count)
}

/// Format a U256 filter ID as hex string.
fn filter_id_to_hex(id: U256) -> String {
    format!("{:#x}", id)
}

/// Parse a hex filter ID string into U256.
fn parse_filter_id(id: &str) -> Result<U256, RpcError> {
    let hex = id.strip_prefix("0x").unwrap_or(id);
    U256::from_str_radix(hex, 16)
        .map_err(|_| RpcError::InvalidBlockNumber(format!("invalid filter id: {id}")))
}

/// The six Ethereum filter/polling methods.
#[rpc(server, namespace = "eth")]
pub trait EthFilterApi {
    /// Install a log filter.
    #[method(name = "newFilter")]
    async fn new_filter(&self, filter: RpcLogFilter) -> RpcResult<String>;

    /// Install a block filter.
    #[method(name = "newBlockFilter")]
    async fn new_block_filter(&self) -> RpcResult<String>;

    /// Install a pending transaction filter.
    #[method(name = "newPendingTransactionFilter")]
    async fn new_pending_transaction_filter(&self) -> RpcResult<String>;

    /// Poll for changes since last poll.
    #[method(name = "getFilterChanges")]
    async fn get_filter_changes(&self, filter_id: String) -> RpcResult<serde_json::Value>;

    /// Get all logs matching the filter's original criteria.
    #[method(name = "getFilterLogs")]
    async fn get_filter_logs(&self, filter_id: String) -> RpcResult<Vec<RpcLog>>;

    /// Remove a filter.
    #[method(name = "uninstallFilter")]
    async fn uninstall_filter(&self, filter_id: String) -> RpcResult<bool>;
}

/// Implementation of the Ethereum filter API.
pub struct EthFilterApiImpl<S: StateProvider> {
    registry: FilterRegistry,
    state_provider: Arc<tokio::sync::RwLock<S>>,
    config: FilterConfig,
}

impl<S: StateProvider> std::fmt::Debug for EthFilterApiImpl<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EthFilterApiImpl").finish()
    }
}

impl<S: StateProvider + 'static> EthFilterApiImpl<S> {
    /// Create a new filter API and start the background sweep task.
    pub fn new(state_provider: Arc<tokio::sync::RwLock<S>>, config: FilterConfig) -> Self {
        let registry = FilterRegistry::default();

        // Background task to sweep expired filters.
        let registry_clone = registry.clone();
        let ttl = config.filter_ttl;
        let interval = config.sweep_interval;
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                ticker.tick().await;
                registry_clone.sweep_expired(ttl);
            }
        });

        Self { registry, state_provider, config }
    }

    async fn current_head(&self) -> Result<u64, RpcError> {
        self.state_provider.read().await.block_number().await
    }
}

#[jsonrpsee::core::async_trait]
impl<S: StateProvider + 'static> EthFilterApiServer for EthFilterApiImpl<S> {
    async fn new_filter(&self, filter: RpcLogFilter) -> RpcResult<String> {
        let head =
            self.current_head().await.map_err(Into::<jsonrpsee::types::ErrorObjectOwned>::into)?;
        let id = self
            .registry
            .install(FilterKind::Log(filter), head.saturating_add(1), self.config.max_filters)
            .ok_or_else(|| {
                jsonrpsee::types::ErrorObjectOwned::owned(
                    codes::LIMIT_EXCEEDED,
                    "too many active filters",
                    None::<()>,
                )
            })?;
        Ok(filter_id_to_hex(id))
    }

    async fn new_block_filter(&self) -> RpcResult<String> {
        let head =
            self.current_head().await.map_err(Into::<jsonrpsee::types::ErrorObjectOwned>::into)?;
        let id = self
            .registry
            .install(FilterKind::Block, head.saturating_add(1), self.config.max_filters)
            .ok_or_else(|| {
                jsonrpsee::types::ErrorObjectOwned::owned(
                    codes::LIMIT_EXCEEDED,
                    "too many active filters",
                    None::<()>,
                )
            })?;
        Ok(filter_id_to_hex(id))
    }

    async fn new_pending_transaction_filter(&self) -> RpcResult<String> {
        let id = self
            .registry
            .install(FilterKind::PendingTransaction, 0, self.config.max_filters)
            .ok_or_else(|| {
                jsonrpsee::types::ErrorObjectOwned::owned(
                    codes::LIMIT_EXCEEDED,
                    "too many active filters",
                    None::<()>,
                )
            })?;
        Ok(filter_id_to_hex(id))
    }

    async fn get_filter_changes(&self, filter_id: String) -> RpcResult<serde_json::Value> {
        let id = parse_filter_id(&filter_id)
            .map_err(Into::<jsonrpsee::types::ErrorObjectOwned>::into)?;
        let head =
            self.current_head().await.map_err(Into::<jsonrpsee::types::ErrorObjectOwned>::into)?;

        let (kind, from_block) = self.registry.advance_watermark(id, head).ok_or_else(|| {
            jsonrpsee::types::ErrorObjectOwned::owned(
                codes::SERVER_ERROR,
                "filter not found",
                None::<()>,
            )
        })?;

        match kind {
            FilterKind::Log(criteria) => {
                if from_block > head {
                    return Ok(serde_json::Value::Array(vec![]));
                }
                // Build a range-bounded filter from the watermark.
                let mut range_filter = criteria;
                range_filter.from_block = Some(BlockNumberOrTag::Number(U64::from(from_block)));
                range_filter.to_block = Some(BlockNumberOrTag::Number(U64::from(head)));

                let provider = self.state_provider.read().await;
                let logs = provider
                    .get_logs(range_filter)
                    .await
                    .map_err(Into::<jsonrpsee::types::ErrorObjectOwned>::into)?;

                if logs.len() > self.config.max_logs_per_response {
                    return Err(jsonrpsee::types::ErrorObjectOwned::owned(
                        codes::LIMIT_EXCEEDED,
                        format!(
                            "query returned more than {} results",
                            self.config.max_logs_per_response
                        ),
                        None::<()>,
                    ));
                }

                Ok(serde_json::to_value(logs).expect("RpcLog is serializable"))
            }
            FilterKind::Block => {
                if from_block > head {
                    return Ok(serde_json::Value::Array(vec![]));
                }
                let provider = self.state_provider.read().await;
                let mut hashes = Vec::new();
                for num in from_block..=head {
                    if let Ok(Some(block)) =
                        provider.block_by_number(BlockNumberOrTag::Number(U64::from(num))).await
                    {
                        hashes.push(format!("{:#x}", block.hash));
                    }
                }
                Ok(serde_json::to_value(hashes).expect("Vec<String> is serializable"))
            }
            FilterKind::PendingTransaction => {
                // No mempool integration yet — return empty array.
                Ok(serde_json::Value::Array(vec![]))
            }
        }
    }

    async fn get_filter_logs(&self, filter_id: String) -> RpcResult<Vec<RpcLog>> {
        let id = parse_filter_id(&filter_id)
            .map_err(Into::<jsonrpsee::types::ErrorObjectOwned>::into)?;

        let kind = self.registry.get_kind(id).ok_or_else(|| {
            jsonrpsee::types::ErrorObjectOwned::owned(
                codes::SERVER_ERROR,
                "filter not found",
                None::<()>,
            )
        })?;

        match kind {
            FilterKind::Log(criteria) => {
                let provider = self.state_provider.read().await;
                let logs = provider
                    .get_logs(criteria)
                    .await
                    .map_err(Into::<jsonrpsee::types::ErrorObjectOwned>::into)?;
                if logs.len() > self.config.max_logs_per_response {
                    return Err(jsonrpsee::types::ErrorObjectOwned::owned(
                        codes::LIMIT_EXCEEDED,
                        format!(
                            "query returned more than {} results",
                            self.config.max_logs_per_response
                        ),
                        None::<()>,
                    ));
                }
                Ok(logs)
            }
            FilterKind::Block | FilterKind::PendingTransaction => {
                Err(jsonrpsee::types::ErrorObjectOwned::owned(
                    codes::INVALID_PARAMS,
                    "eth_getFilterLogs is only valid for log filters",
                    None::<()>,
                ))
            }
        }
    }

    async fn uninstall_filter(&self, filter_id: String) -> RpcResult<bool> {
        let id = parse_filter_id(&filter_id)
            .map_err(Into::<jsonrpsee::types::ErrorObjectOwned>::into)?;
        Ok(self.registry.uninstall(id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_id_roundtrip() {
        let id = generate_filter_id();
        let hex = filter_id_to_hex(id);
        let parsed = parse_filter_id(&hex).unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn filter_id_unique() {
        let a = generate_filter_id();
        let b = generate_filter_id();
        assert_ne!(a, b);
    }

    #[test]
    fn parse_invalid_filter_id() {
        assert!(parse_filter_id("not_hex").is_err());
    }

    #[test]
    fn registry_install_and_uninstall() {
        let registry = FilterRegistry::default();
        let id = registry.install(FilterKind::Block, 10, 100).unwrap();
        assert!(registry.uninstall(id));
        assert!(!registry.uninstall(id)); // already removed
    }

    #[test]
    fn registry_max_filters() {
        let registry = FilterRegistry::default();
        // Fill to max
        for _ in 0..3 {
            registry.install(FilterKind::Block, 0, 3).unwrap();
        }
        // Should fail
        assert!(registry.install(FilterKind::Block, 0, 3).is_none());
    }

    #[test]
    fn registry_advance_watermark() {
        let registry = FilterRegistry::default();
        let id = registry.install(FilterKind::Block, 5, 100).unwrap();

        let (_, old_next) = registry.advance_watermark(id, 10).unwrap();
        assert_eq!(old_next, 5);

        let (_, old_next) = registry.advance_watermark(id, 15).unwrap();
        assert_eq!(old_next, 11); // was advanced to 10+1=11
    }

    #[test]
    fn registry_advance_missing_filter() {
        let registry = FilterRegistry::default();
        assert!(registry.advance_watermark(U256::from(999), 10).is_none());
    }

    #[test]
    fn registry_sweep_expired() {
        let registry = FilterRegistry::default();
        let id = registry.install(FilterKind::Block, 0, 100).unwrap();

        // With 0 TTL, everything expires immediately
        registry.sweep_expired(Duration::ZERO);
        assert!(!registry.uninstall(id)); // was swept
    }

    #[test]
    fn registry_get_kind() {
        let registry = FilterRegistry::default();
        let id = registry.install(FilterKind::PendingTransaction, 0, 100).unwrap();
        let kind = registry.get_kind(id);
        assert!(matches!(kind, Some(FilterKind::PendingTransaction)));
    }

    #[test]
    fn filter_config_defaults() {
        let config = FilterConfig::default();
        assert_eq!(config.filter_ttl, Duration::from_secs(300));
        assert_eq!(config.max_logs_per_response, 10_000);
        assert_eq!(config.max_filters, 10_000);
    }
}
