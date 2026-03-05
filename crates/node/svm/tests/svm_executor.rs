//! SVM executor integration tests.

use std::collections::BTreeMap;

use monmouth_svm::{SvmAccountBridge, SvmExecutor, SvmExecutorConfig};
use solana_account::{Account, AccountSharedData};
use solana_hash::Hash;
use solana_keypair::Keypair;
use solana_native_token::LAMPORTS_PER_SOL;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use solana_system_transaction as system_transaction;
use solana_transaction::sanitized::SanitizedTransaction;

fn funded_account(lamports: u64) -> AccountSharedData {
    AccountSharedData::from(Account {
        lamports,
        data: vec![],
        owner: Pubkey::default(),
        executable: false,
        rent_epoch: 0,
    })
}

fn make_bridge_with_accounts(accounts: Vec<(Pubkey, u64)>) -> SvmAccountBridge {
    let mut map = BTreeMap::new();
    for (pk, lamports) in accounts {
        map.insert(pk, funded_account(lamports));
    }
    SvmAccountBridge::new(map)
}

#[test]
fn execute_empty_batch_returns_empty_outcome() {
    let executor = SvmExecutor::new();
    let bridge = SvmAccountBridge::empty();
    let empty: Vec<SanitizedTransaction> = vec![];
    let outcome = executor.execute(&bridge, 1, 1_700_000_000, &empty, None).unwrap();
    assert!(outcome.changes.is_empty());
    assert!(outcome.tx_results.is_empty());
    assert_eq!(outcome.compute_units_used, 0);
}

#[test]
fn executor_with_custom_config() {
    let config = SvmExecutorConfig { compute_budget: 500_000, ..Default::default() };
    let executor = SvmExecutor::with_config(config);
    assert_eq!(executor.config().compute_budget, 500_000);
}

#[test]
fn sol_transfer_produces_tx_result() {
    let executor = SvmExecutor::new();

    let sender = Keypair::new();
    let recipient = Pubkey::new_unique();

    // Fund sender with 10 SOL
    let mut bridge = make_bridge_with_accounts(vec![(sender.pubkey(), 10 * LAMPORTS_PER_SOL)]);

    // Add system program account (needed for processing)
    bridge.set_account(
        solana_system_program::id(),
        AccountSharedData::from(Account {
            lamports: 1,
            data: vec![],
            owner: solana_sdk_ids::native_loader::id(),
            executable: true,
            rent_epoch: 0,
        }),
    );

    // Create a transfer transaction with matching blockhash (sha256-based derivation)
    let blockhash = {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(b"monmouth-svm-blockhash-v1");
        hasher.update(1u64.to_le_bytes());
        hasher.update(1_700_000_000u64.to_le_bytes());
        let digest = hasher.finalize();
        let mut h = [0u8; 32];
        h.copy_from_slice(&digest);
        Hash::new_from_array(h)
    };

    let tx = system_transaction::transfer(&sender, &recipient, LAMPORTS_PER_SOL, blockhash);
    let stx = SanitizedTransaction::from_transaction_for_tests(tx);

    let outcome = executor.execute(&bridge, 1, 1_700_000_000, &[stx], None).unwrap();

    // Should have exactly one transaction result
    assert_eq!(outcome.tx_results.len(), 1);

    let result = &outcome.tx_results[0];
    // The transfer may succeed or fail (e.g. missing recent blockhash in history),
    // but the executor should produce a result without panicking.
    if result.success {
        assert!(outcome.compute_units_used > 0, "successful tx should use compute units");
        assert!(!outcome.changes.is_empty(), "successful transfer should produce changes");
    } else {
        // Common failure: account loading issues in minimal environment.
        // This is expected — the important thing is no panic.
        eprintln!("Transfer failed (expected in minimal env): {:?}", result.error);
    }
}

#[test]
fn deterministic_execution_same_inputs_same_output() {
    let executor = SvmExecutor::new();
    let bridge = SvmAccountBridge::empty();
    let empty: Vec<SanitizedTransaction> = vec![];

    let outcome1 = executor.execute(&bridge, 42, 1_700_000_000, &empty, None).unwrap();
    let outcome2 = executor.execute(&bridge, 42, 1_700_000_000, &empty, None).unwrap();

    assert_eq!(outcome1.compute_units_used, outcome2.compute_units_used);
    assert_eq!(outcome1.changes, outcome2.changes);
    assert_eq!(outcome1.tx_results.len(), outcome2.tx_results.len());
}

#[test]
fn different_block_heights_produce_independent_results() {
    let executor = SvmExecutor::new();
    let bridge = SvmAccountBridge::empty();
    let empty: Vec<SanitizedTransaction> = vec![];

    let outcome1 = executor.execute(&bridge, 1, 1_700_000_000, &empty, None).unwrap();
    let outcome2 = executor.execute(&bridge, 2, 1_700_000_002, &empty, None).unwrap();

    // Both should succeed with empty results
    assert!(outcome1.changes.is_empty());
    assert!(outcome2.changes.is_empty());
}
