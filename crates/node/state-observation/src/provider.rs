//! Pluggable state provider trait.

use monmouth_agent_types::{StateQuery, StateQueryResult};

use crate::StateObservationError;

/// Trait for pluggable state backends.
///
/// Implementors provide access to chain state (balances, nonces, storage,
/// contract calls) through a uniform interface. The [`StateObserver`] delegates
/// all actual state reads to a provider.
///
/// # Object Safety
///
/// This trait is object-safe via `async_trait`, allowing runtime polymorphism
/// for different backends (REVM, remote RPC, cached overlay, etc.).
#[async_trait::async_trait]
pub trait StateProvider: Send + Sync + std::fmt::Debug {
    /// Execute a single state query.
    async fn execute(&self, query: &StateQuery) -> Result<StateQueryResult, StateObservationError>;
}

/// A no-op state provider that returns errors for all queries.
///
/// Used as a default when no real state backend is configured.
#[derive(Debug, Clone, Copy)]
pub struct NoopStateProvider;

#[async_trait::async_trait]
impl StateProvider for NoopStateProvider {
    async fn execute(&self, query: &StateQuery) -> Result<StateQueryResult, StateObservationError> {
        let query_type = match query {
            StateQuery::Balance { .. } => "balance",
            StateQuery::Nonce { .. } => "nonce",
            StateQuery::Code { .. } => "code",
            StateQuery::Storage { .. } => "storage",
            StateQuery::Erc20Balance { .. } => "erc20Balance",
            StateQuery::ContractState { .. } => "contractState",
            StateQuery::MultiBalance { .. } => "multiBalance",
        };
        Err(StateObservationError::UnsupportedQuery(format!(
            "{query_type}: no state provider configured"
        )))
    }
}
