//! Simulation provider trait and noop implementation.

use monmouth_agent_types::{BundleSimRequest, SimulationResult};

use crate::SimulationError;

/// Object-safe trait for transaction simulation providers.
///
/// Implementations may delegate to an in-process REVM instance, a remote
/// simulation service, or any other backend that can produce deterministic
/// state diffs for a given transaction (or bundle of transactions).
pub trait SimulationProvider: Send + Sync + std::fmt::Debug + 'static {
    /// Simulate a single raw transaction.
    ///
    /// # Errors
    ///
    /// Returns a [`SimulationError`] if the transaction is invalid, execution
    /// fails, or the required state is unavailable.
    fn simulate_tx(&self, raw_tx: &[u8]) -> Result<SimulationResult, SimulationError>;

    /// Simulate a bundle of transactions executed in sequence.
    ///
    /// Each transaction in the bundle is applied on top of the state produced
    /// by the previous one.
    ///
    /// # Errors
    ///
    /// Returns a [`SimulationError`] if any transaction fails, the bundle is
    /// too large, or the required state is unavailable.
    fn simulate_bundle(
        &self,
        request: &BundleSimRequest,
    ) -> Result<SimulationResult, SimulationError>;
}

/// A no-op simulation provider that always returns an error.
///
/// Used as a placeholder when no real simulation backend is configured.
#[derive(Debug, Clone, Copy)]
pub struct NoopSimulationProvider;

impl SimulationProvider for NoopSimulationProvider {
    fn simulate_tx(&self, _raw_tx: &[u8]) -> Result<SimulationResult, SimulationError> {
        Err(SimulationError::StateUnavailable("no simulation provider configured".to_string()))
    }

    fn simulate_bundle(
        &self,
        _request: &BundleSimRequest,
    ) -> Result<SimulationResult, SimulationError> {
        Err(SimulationError::StateUnavailable("no simulation provider configured".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noop_simulate_tx_returns_error() {
        let provider = NoopSimulationProvider;
        let result = provider.simulate_tx(&[0x00, 0x01, 0x02]);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(matches!(err, SimulationError::StateUnavailable(_)));
        assert!(err.to_string().contains("no simulation provider configured"));
    }

    #[test]
    fn noop_simulate_bundle_returns_error() {
        let provider = NoopSimulationProvider;
        let request = BundleSimRequest { transactions: vec![vec![0x00]], block_number: None };
        let result = provider.simulate_bundle(&request);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(matches!(err, SimulationError::StateUnavailable(_)));
    }

    #[test]
    fn trait_object_safety() {
        // Verify the trait is object-safe by constructing a trait object.
        let provider = NoopSimulationProvider;
        let _boxed: Box<dyn SimulationProvider> = Box::new(provider);
    }

    #[test]
    fn noop_is_debug() {
        let provider = NoopSimulationProvider;
        let debug_str = format!("{provider:?}");
        assert_eq!(debug_str, "NoopSimulationProvider");
    }

    #[test]
    fn error_codes() {
        assert_eq!(SimulationError::ExecutionFailed("test".to_string()).code(), -32800);
        assert_eq!(SimulationError::InvalidTransaction("test".to_string()).code(), -32801);
        assert_eq!(SimulationError::BundleTooLarge { size: 10, max: 5 }.code(), -32802);
        assert_eq!(SimulationError::GasLimitExceeded { used: 100, limit: 50 }.code(), -32803);
        assert_eq!(SimulationError::Timeout.code(), -32804);
        assert_eq!(SimulationError::StateUnavailable("test".to_string()).code(), -32805);
    }
}
