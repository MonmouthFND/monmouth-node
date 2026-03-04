//! Builtin program registration for SVM.
//!
//! Registers the minimal set of Solana built-in programs needed for
//! basic transaction processing: System Program, BPF Loaders, and
//! Compute Budget Program.

use solana_pubkey::Pubkey;
use solana_svm::transaction_processor::TransactionBatchProcessor;

use crate::fork_graph::MonmouthForkGraph;

/// System Program — handles SOL transfers, account creation.
pub fn system_program_id() -> Pubkey {
    solana_system_program::id()
}

/// BPF Loader v2 (upgradeable) — loads deployed SVM programs.
pub fn bpf_loader_upgradeable_id() -> Pubkey {
    solana_sdk_ids::bpf_loader_upgradeable::id()
}

/// Compute Budget Program — sets compute unit limits/prices.
pub fn compute_budget_program_id() -> Pubkey {
    solana_sdk_ids::compute_budget::id()
}

/// Register all built-in programs on a `TransactionBatchProcessor`.
///
/// This is the minimal set required for basic SVM operation:
/// - System Program: transfers, account creation
/// - BPF Loader Upgradeable: program deployment and execution
/// - Compute Budget: CU limits and pricing
pub fn register_builtins(processor: &TransactionBatchProcessor<MonmouthForkGraph>) {
    use solana_program_runtime::loaded_programs::ProgramCacheEntry;

    // System Program — uses `Entrypoint::vm` from declare_process_instruction! macro
    processor.add_builtin(
        system_program_id(),
        ProgramCacheEntry::new_builtin(
            0,
            0,
            solana_system_program::system_processor::Entrypoint::vm,
        ),
    );

    // BPF Loader (Upgradeable) — uses declare_builtin_function! macro
    processor.add_builtin(
        bpf_loader_upgradeable_id(),
        ProgramCacheEntry::new_builtin(
            0,
            0,
            solana_bpf_loader_program::Entrypoint::vm,
        ),
    );

    // Compute Budget Program
    processor.add_builtin(
        compute_budget_program_id(),
        ProgramCacheEntry::new_builtin(
            0,
            0,
            solana_compute_budget_program::Entrypoint::vm,
        ),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn program_ids_are_distinct() {
        let ids = [system_program_id(), bpf_loader_upgradeable_id(), compute_budget_program_id()];
        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                assert_ne!(ids[i], ids[j], "program IDs must be unique");
            }
        }
    }

    #[test]
    fn system_program_is_well_known() {
        let id = system_program_id();
        // System program ID is base58 "11111111111111111111111111111111"
        // which is NOT all-zero bytes — it's [0, 0, 0, ..., 0, 0, 1]
        assert_eq!(id.to_string(), "11111111111111111111111111111111");
    }
}
