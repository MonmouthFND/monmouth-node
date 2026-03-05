//! State observer that executes structured queries against a provider.

use std::sync::Arc;

use monmouth_agent_types::{StateQuery, StateQueryResult};
use tracing::{debug, info};

use crate::{StateObservationError, StateProvider};

/// Default maximum batch size for multi-query requests.
pub const DEFAULT_MAX_BATCH_SIZE: usize = 100;

/// Executes structured state queries against a pluggable backend.
///
/// The observer wraps a [`StateProvider`] and adds batching support,
/// logging, and size limits.
#[derive(Debug)]
pub struct StateObserver {
    provider: Arc<dyn StateProvider>,
    max_batch_size: usize,
}

impl StateObserver {
    /// Create a new observer backed by the given provider.
    pub fn new(provider: Arc<dyn StateProvider>) -> Self {
        info!("State observer initialised");
        Self { provider, max_batch_size: DEFAULT_MAX_BATCH_SIZE }
    }

    /// Set the maximum batch query size.
    #[must_use]
    pub const fn with_max_batch_size(mut self, max: usize) -> Self {
        self.max_batch_size = max;
        self
    }

    /// Execute a single state query.
    ///
    /// # Errors
    ///
    /// Returns an error if the provider fails or the query is unsupported.
    pub async fn query(&self, query: &StateQuery) -> Result<StateQueryResult, StateObservationError> {
        debug!(?query, "Executing state query");
        self.provider.execute(query).await
    }

    /// Execute a batch of state queries.
    ///
    /// Queries are executed sequentially against the provider. If any query
    /// fails, an error result is returned for that position.
    ///
    /// # Errors
    ///
    /// Returns [`StateObservationError::BatchTooLarge`] if the batch exceeds
    /// the configured maximum size.
    pub async fn query_batch(
        &self,
        queries: &[StateQuery],
    ) -> Result<Vec<StateQueryResult>, StateObservationError> {
        if queries.len() > self.max_batch_size {
            return Err(StateObservationError::BatchTooLarge {
                size: queries.len(),
                max: self.max_batch_size,
            });
        }

        debug!(count = queries.len(), "Executing batch state query");

        let mut results = Vec::with_capacity(queries.len());
        for query in queries {
            let result = match self.provider.execute(query).await {
                Ok(r) => r,
                Err(e) => StateQueryResult::Error { message: e.to_string() },
            };
            results.push(result);
        }
        Ok(results)
    }

    /// Returns the maximum batch size.
    #[must_use]
    pub const fn max_batch_size(&self) -> usize {
        self.max_batch_size
    }
}

#[cfg(test)]
mod tests {
    use alloy_primitives::{Address, U256};
    use monmouth_agent_types::StateQuery;

    use super::*;
    use crate::NoopStateProvider;

    /// A test provider that returns fixed values for balance queries.
    #[derive(Debug)]
    struct MockProvider;

    #[async_trait::async_trait]
    impl StateProvider for MockProvider {
        async fn execute(
            &self,
            query: &StateQuery,
        ) -> Result<StateQueryResult, StateObservationError> {
            match query {
                StateQuery::Balance { .. } => {
                    Ok(StateQueryResult::Balance { balance: U256::from(1_000_000u64) })
                }
                StateQuery::Nonce { .. } => Ok(StateQueryResult::Nonce { nonce: 42 }),
                StateQuery::Storage { .. } => {
                    Ok(StateQueryResult::Storage { value: alloy_primitives::B256::ZERO })
                }
                StateQuery::MultiBalance { addresses } => {
                    let balances = addresses.iter().map(|_| U256::from(500u64)).collect();
                    Ok(StateQueryResult::MultiBalance { balances })
                }
                _ => Err(StateObservationError::UnsupportedQuery("mock".to_string())),
            }
        }
    }

    fn mock_observer() -> StateObserver {
        StateObserver::new(Arc::new(MockProvider))
    }

    fn noop_observer() -> StateObserver {
        StateObserver::new(Arc::new(NoopStateProvider))
    }

    #[tokio::test]
    async fn query_balance() {
        let obs = mock_observer();
        let result = obs.query(&StateQuery::Balance { address: Address::ZERO }).await.unwrap();
        assert!(matches!(result, StateQueryResult::Balance { balance } if balance == U256::from(1_000_000u64)));
    }

    #[tokio::test]
    async fn query_nonce() {
        let obs = mock_observer();
        let result = obs.query(&StateQuery::Nonce { address: Address::ZERO }).await.unwrap();
        assert!(matches!(result, StateQueryResult::Nonce { nonce: 42 }));
    }

    #[tokio::test]
    async fn noop_provider_returns_unsupported() {
        let obs = noop_observer();
        let err = obs.query(&StateQuery::Balance { address: Address::ZERO }).await.unwrap_err();
        assert!(matches!(err, StateObservationError::UnsupportedQuery(_)));
    }

    #[tokio::test]
    async fn batch_query() {
        let obs = mock_observer();
        let queries = vec![
            StateQuery::Balance { address: Address::ZERO },
            StateQuery::Nonce { address: Address::repeat_byte(1) },
        ];
        let results = obs.query_batch(&queries).await.unwrap();
        assert_eq!(results.len(), 2);
        assert!(matches!(&results[0], StateQueryResult::Balance { .. }));
        assert!(matches!(&results[1], StateQueryResult::Nonce { .. }));
    }

    #[tokio::test]
    async fn batch_with_partial_failure() {
        let obs = mock_observer();
        let queries = vec![
            StateQuery::Balance { address: Address::ZERO },
            StateQuery::Code { address: Address::ZERO }, // unsupported by mock
            StateQuery::Nonce { address: Address::ZERO },
        ];
        let results = obs.query_batch(&queries).await.unwrap();
        assert_eq!(results.len(), 3);
        assert!(matches!(&results[0], StateQueryResult::Balance { .. }));
        assert!(matches!(&results[1], StateQueryResult::Error { .. }));
        assert!(matches!(&results[2], StateQueryResult::Nonce { .. }));
    }

    #[tokio::test]
    async fn batch_too_large() {
        let obs = mock_observer().with_max_batch_size(2);
        let queries = vec![
            StateQuery::Balance { address: Address::ZERO },
            StateQuery::Balance { address: Address::ZERO },
            StateQuery::Balance { address: Address::ZERO },
        ];
        let err = obs.query_batch(&queries).await.unwrap_err();
        assert!(matches!(err, StateObservationError::BatchTooLarge { size: 3, max: 2 }));
    }

    #[tokio::test]
    async fn multi_balance_query() {
        let obs = mock_observer();
        let query = StateQuery::MultiBalance {
            addresses: vec![Address::ZERO, Address::repeat_byte(1), Address::repeat_byte(2)],
        };
        let result = obs.query(&query).await.unwrap();
        if let StateQueryResult::MultiBalance { balances } = result {
            assert_eq!(balances.len(), 3);
            assert!(balances.iter().all(|b| *b == U256::from(500u64)));
        } else {
            panic!("expected MultiBalance result");
        }
    }

    #[test]
    fn error_codes() {
        assert_eq!(StateObservationError::ProviderError("x".into()).code(), -32850);
        assert_eq!(StateObservationError::AccountNotFound(Address::ZERO).code(), -32851);
        assert_eq!(StateObservationError::UnsupportedQuery("x".into()).code(), -32852);
        assert_eq!(
            StateObservationError::BatchTooLarge { size: 10, max: 5 }.code(),
            -32853
        );
    }
}
