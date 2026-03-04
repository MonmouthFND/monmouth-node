//! SVM executor — wraps `TransactionBatchProcessor` for Monmouth.
//!
//! Provides a high-level `SvmExecutor` that takes Solana transactions
//! and produces an `SvmExecutionOutcome` with account changes and logs.

use std::sync::{Arc, RwLock};

use solana_account::{AccountSharedData, ReadableAccount};
use solana_clock::Clock;
use solana_epoch_schedule::EpochSchedule;
use solana_hash::Hash;
use solana_rent::Rent;
use solana_svm::transaction_processor::{
    TransactionBatchProcessor, TransactionProcessingConfig, TransactionProcessingEnvironment,
};
use solana_svm_transaction::svm_transaction::SVMTransaction;

use crate::account_bridge::SvmAccountBridge;
use crate::builtins::register_builtins;
use crate::changeset::{SvmAccountUpdate, SvmChangeSet};
use crate::fork_graph::MonmouthForkGraph;
use crate::sysvars;
use crate::SvmError;

/// Configuration for the SVM executor.
#[derive(Clone, Debug)]
pub struct SvmExecutorConfig {
    /// Maximum compute units per transaction.
    pub compute_budget: u64,
    /// Epoch length for clock sysvar computation.
    pub epoch_length: u64,
}

impl Default for SvmExecutorConfig {
    fn default() -> Self {
        Self { compute_budget: 200_000, epoch_length: sysvars::DEFAULT_EPOCH_LENGTH }
    }
}

/// Outcome of SVM execution for a batch of transactions.
#[derive(Clone, Debug, Default)]
pub struct SvmExecutionOutcome {
    /// Account state changes.
    pub changes: SvmChangeSet,
    /// Per-transaction results.
    pub tx_results: Vec<SvmTxResult>,
    /// Total compute units consumed across all transactions.
    pub compute_units_used: u64,
}

/// Result of a single SVM transaction execution.
#[derive(Clone, Debug)]
pub struct SvmTxResult {
    /// Transaction signature (first signature).
    pub signature: [u8; 64],
    /// Whether execution succeeded.
    pub success: bool,
    /// Compute units consumed by this transaction.
    pub compute_units: u64,
    /// Log messages emitted during execution.
    pub logs: Vec<String>,
    /// Error message if execution failed.
    pub error: Option<String>,
}

/// Monmouth SVM executor.
///
/// Wraps `TransactionBatchProcessor` with Monmouth-specific configuration.
/// Stateless — all account state comes from the `SvmAccountBridge` passed
/// to `execute()`.
#[derive(Debug)]
pub struct SvmExecutor {
    config: SvmExecutorConfig,
    fork_graph: Arc<RwLock<MonmouthForkGraph>>,
}

impl Clone for SvmExecutor {
    fn clone(&self) -> Self {
        Self { config: self.config.clone(), fork_graph: Arc::clone(&self.fork_graph) }
    }
}

impl SvmExecutor {
    /// Create a new SVM executor with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(SvmExecutorConfig::default())
    }

    /// Create a new SVM executor with the given configuration.
    #[must_use]
    pub fn with_config(config: SvmExecutorConfig) -> Self {
        let fork_graph = Arc::new(RwLock::new(MonmouthForkGraph::new(0)));
        Self { config, fork_graph }
    }

    /// Get the executor configuration.
    pub const fn config(&self) -> &SvmExecutorConfig {
        &self.config
    }

    /// Execute a batch of sanitized SVM transactions.
    ///
    /// # Arguments
    /// * `bridge` - Account state for the SVM to read from
    /// * `block_height` - Current block height (maps to Solana slot)
    /// * `block_timestamp` - Block timestamp (unix seconds)
    /// * `txs` - Sanitized transactions to execute
    ///
    /// # Returns
    /// Execution outcome containing state changes and per-tx results.
    pub fn execute<T: SVMTransaction>(
        &self,
        bridge: &SvmAccountBridge,
        block_height: u64,
        block_timestamp: u64,
        txs: &[T],
    ) -> Result<SvmExecutionOutcome, SvmError> {
        if txs.is_empty() {
            return Ok(SvmExecutionOutcome::default());
        }

        // Update fork graph slot
        if let Ok(fg) = self.fork_graph.read() {
            fg.set_slot(block_height);
        }

        // Create processor for this block
        let processor =
            TransactionBatchProcessor::<MonmouthForkGraph>::new_uninitialized(block_height, 0);

        // Wire fork graph into the program cache (required for program lookups).
        processor
            .global_program_cache
            .write()
            .unwrap()
            .set_fork_graph(Arc::downgrade(&self.fork_graph));

        // Register built-in programs
        register_builtins(&processor);

        // Populate sysvars
        let clock =
            sysvars::clock_from_block(block_height, block_timestamp, self.config.epoch_length)?;
        let rent = sysvars::default_rent();
        let epoch_schedule = sysvars::default_epoch_schedule();

        self.fill_sysvar_cache(&processor, &clock, &rent, &epoch_schedule);

        // Derive a deterministic blockhash from the block height.
        // Use the raw height bytes padded to 32 bytes — simple and deterministic.
        let mut hash_bytes = [0u8; 32];
        hash_bytes[..8].copy_from_slice(&block_height.to_le_bytes());
        hash_bytes[8..16].copy_from_slice(&block_timestamp.to_le_bytes());
        let blockhash = Hash::new_from_array(hash_bytes);

        let environment = TransactionProcessingEnvironment {
            blockhash,
            blockhash_lamports_per_signature: 5000, // Standard Solana fee
            epoch_total_stake: 1_000_000_000,
            feature_set: Default::default(),
            rent,
            ..Default::default()
        };

        let config = TransactionProcessingConfig {
            recording_config: solana_svm::transaction_processor::ExecutionRecordingConfig {
                enable_log_recording: true,
                enable_return_data_recording: false,
                enable_cpi_recording: false,
                enable_transaction_balance_recording: false,
            },
            ..Default::default()
        };

        // Prepare check results (all pass — validation happens upstream).
        // Each tx gets default compute budget and fee settings.
        let budget =
            solana_program_runtime::execution_budget::SVMTransactionExecutionBudget::new_with_defaults(false);
        let limits =
            solana_program_runtime::execution_budget::SVMTransactionExecutionAndFeeBudgetLimits {
                budget,
                loaded_accounts_data_size_limit: solana_program_runtime::execution_budget::MAX_LOADED_ACCOUNTS_DATA_SIZE_BYTES,
                fee_details: solana_fee_structure::FeeDetails::default(),
            };
        let check_results: Vec<solana_svm::account_loader::TransactionCheckResult> = txs
            .iter()
            .map(|_| {
                Ok(solana_svm::account_loader::CheckedTransactionDetails::new(None, limits))
            })
            .collect();

        // Execute the batch
        let output = processor.load_and_execute_sanitized_transactions(
            bridge,
            txs,
            check_results,
            &environment,
            &config,
        );

        // Extract results
        self.extract_outcome(txs, output)
    }

    /// Fill the processor's sysvar cache with Monmouth values.
    fn fill_sysvar_cache(
        &self,
        processor: &TransactionBatchProcessor<MonmouthForkGraph>,
        clock: &Clock,
        rent: &Rent,
        epoch_schedule: &EpochSchedule,
    ) {
        // Create a temporary bridge with sysvar accounts to populate the cache.
        let mut sysvar_bridge = SvmAccountBridge::empty();

        // Clock sysvar
        let clock_data = bincode::serialize(clock).unwrap_or_default();
        sysvar_bridge.set_account(
            solana_sdk_ids::sysvar::clock::id(),
            create_sysvar_account(&clock_data),
        );

        // Rent sysvar
        let rent_data = bincode::serialize(rent).unwrap_or_default();
        sysvar_bridge.set_account(
            solana_sdk_ids::sysvar::rent::id(),
            create_sysvar_account(&rent_data),
        );

        // EpochSchedule sysvar
        let schedule_data = bincode::serialize(epoch_schedule).unwrap_or_default();
        sysvar_bridge.set_account(
            solana_sdk_ids::sysvar::epoch_schedule::id(),
            create_sysvar_account(&schedule_data),
        );

        processor.fill_missing_sysvar_cache_entries(&sysvar_bridge);
    }

    /// Extract execution outcome from the processor output.
    fn extract_outcome<T: SVMTransaction>(
        &self,
        txs: &[T],
        output: solana_svm::transaction_processor::LoadAndExecuteSanitizedTransactionsOutput,
    ) -> Result<SvmExecutionOutcome, SvmError> {
        let mut outcome = SvmExecutionOutcome::default();

        for (i, result) in output.processing_results.into_iter().enumerate() {
            match result {
                Ok(processed) => {
                    use solana_svm::transaction_processing_result::ProcessedTransaction;
                    match processed {
                        ProcessedTransaction::Executed(executed) => {
                            let details = &executed.execution_details;
                            let cu = details.executed_units;
                            outcome.compute_units_used =
                                outcome.compute_units_used.saturating_add(cu);

                            let success = details.status.is_ok();
                            let logs = details.log_messages.clone().unwrap_or_default();

                            let signature = extract_signature(txs, i);

                            let error = if !success {
                                details.status.as_ref().err().map(|e| format!("{e}"))
                            } else {
                                None
                            };

                            outcome.tx_results.push(SvmTxResult {
                                signature,
                                success,
                                compute_units: cu,
                                logs,
                                error,
                            });

                            // Extract account changes from loaded accounts
                            for (pubkey, loaded_account) in
                                &executed.loaded_transaction.accounts
                            {
                                let acct: &AccountSharedData = loaded_account;
                                let update = SvmAccountUpdate {
                                    lamports: acct.lamports(),
                                    data: acct.data().to_vec(),
                                    owner: acct.owner().to_bytes(),
                                    executable: acct.executable(),
                                    rent_epoch: acct.rent_epoch(),
                                };
                                outcome.changes.insert(pubkey.to_bytes(), update);
                            }
                        }
                        ProcessedTransaction::FeesOnly(fees_only) => {
                            #[allow(deprecated)]
                            let error_msg = format!("fees-only: {}", fees_only.load_error);
                            let signature = extract_signature(txs, i);
                            outcome.tx_results.push(SvmTxResult {
                                signature,
                                success: false,
                                compute_units: 0,
                                logs: vec![],
                                error: Some(error_msg),
                            });
                        }
                    }
                }
                Err(tx_error) => {
                    let signature = extract_signature(txs, i);
                    outcome.tx_results.push(SvmTxResult {
                        signature,
                        success: false,
                        compute_units: 0,
                        logs: vec![],
                        error: Some(format!("{tx_error}")),
                    });
                }
            }
        }

        Ok(outcome)
    }
}

impl Default for SvmExecutor {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a sysvar account with the given serialized data.
fn create_sysvar_account(data: &[u8]) -> AccountSharedData {
    use solana_account::Account;
    AccountSharedData::from(Account {
        lamports: 1,
        data: data.to_vec(),
        owner: solana_sdk_ids::sysvar::id(),
        executable: false,
        rent_epoch: 0,
    })
}

/// Extract the 64-byte signature from a transaction at index `i`.
fn extract_signature<T: SVMTransaction>(txs: &[T], i: usize) -> [u8; 64] {
    if i < txs.len() {
        let sig = txs[i].signature();
        let sig_bytes = sig.as_ref();
        let mut buf = [0u8; 64];
        let copy_len = sig_bytes.len().min(64);
        buf[..copy_len].copy_from_slice(&sig_bytes[..copy_len]);
        buf
    } else {
        [0u8; 64]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn executor_default_config() {
        let executor = SvmExecutor::new();
        assert_eq!(executor.config().compute_budget, 200_000);
    }

    #[test]
    fn executor_custom_config() {
        let config = SvmExecutorConfig { compute_budget: 500_000, ..Default::default() };
        let executor = SvmExecutor::with_config(config);
        assert_eq!(executor.config().compute_budget, 500_000);
    }

    #[test]
    fn execute_empty_batch() {
        let executor = SvmExecutor::new();
        let bridge = SvmAccountBridge::empty();
        let empty: Vec<solana_transaction::sanitized::SanitizedTransaction> = vec![];
        let outcome = executor.execute(&bridge, 1, 1_700_000_000, &empty).unwrap();
        assert!(outcome.changes.is_empty());
        assert!(outcome.tx_results.is_empty());
        assert_eq!(outcome.compute_units_used, 0);
    }

    #[test]
    fn executor_is_clone() {
        let executor = SvmExecutor::new();
        let _cloned = executor.clone();
    }

    #[test]
    fn outcome_default() {
        let outcome = SvmExecutionOutcome::default();
        assert!(outcome.changes.is_empty());
        assert_eq!(outcome.compute_units_used, 0);
    }
}
