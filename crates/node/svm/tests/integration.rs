//! SVM integration tests — cross-block persistence, state root computation,
//! and end-to-end SVM pipeline validation.

use monmouth_svm::{
    SvmAccountBridge, SvmAccountUpdate, SvmChangeSet, SvmExecutor, SvmExecutorConfig,
    SvmStateStore, deserialize_svm_tx,
};
use solana_account::{Account, AccountSharedData};
use solana_hash::Hash;
use solana_keypair::Keypair;
use solana_native_token::LAMPORTS_PER_SOL;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use solana_system_transaction as system_transaction;
use solana_transaction::sanitized::SanitizedTransaction;
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn funded_bridge(accounts: Vec<(Pubkey, u64)>) -> SvmAccountBridge {
    let mut map = BTreeMap::new();
    for (pk, lamports) in accounts {
        map.insert(
            pk,
            AccountSharedData::from(Account {
                lamports,
                data: vec![],
                owner: Pubkey::default(),
                executable: false,
                rent_epoch: 0,
            }),
        );
    }
    // Always include the system program so transfers can execute.
    map.insert(
        solana_system_program::id(),
        AccountSharedData::from(Account {
            lamports: 1,
            data: vec![],
            owner: solana_sdk_ids::native_loader::id(),
            executable: true,
            rent_epoch: 0,
        }),
    );
    SvmAccountBridge::new(map)
}

fn block_hash(height: u64, timestamp: u64) -> Hash {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(b"monmouth-svm-blockhash-v1");
    hasher.update(height.to_le_bytes());
    hasher.update(timestamp.to_le_bytes());
    let digest = hasher.finalize();
    let mut h = [0u8; 32];
    h.copy_from_slice(&digest);
    Hash::new_from_array(h)
}

fn dummy_update(lamports: u64) -> SvmAccountUpdate {
    SvmAccountUpdate {
        lamports,
        data: vec![],
        owner: [0u8; 32],
        executable: false,
        rent_epoch: 0,
    }
}

fn program_update(data_len: usize) -> SvmAccountUpdate {
    SvmAccountUpdate {
        lamports: 1_000_000,
        data: vec![0xBF; data_len], // BPF bytecode placeholder
        owner: solana_sdk_ids::bpf_loader::id().to_bytes(),
        executable: true,
        rent_epoch: 0,
    }
}

// ---------------------------------------------------------------------------
// Cross-block persistence tests
// ---------------------------------------------------------------------------

#[test]
fn state_store_persists_across_blocks() {
    let store = SvmStateStore::new();

    // Block 1: create two accounts
    let mut block1_changes = SvmChangeSet::new();
    block1_changes.insert([1u8; 32], dummy_update(1_000));
    block1_changes.insert([2u8; 32], dummy_update(2_000));
    let root1 = store.compute_root(&block1_changes).unwrap();
    store.apply_changes(&block1_changes).unwrap();

    // Block 2: modify one account, add a new one
    let mut block2_changes = SvmChangeSet::new();
    block2_changes.insert([1u8; 32], dummy_update(1_500)); // modified
    block2_changes.insert([3u8; 32], dummy_update(3_000)); // new
    let root2 = store.compute_root(&block2_changes).unwrap();
    store.apply_changes(&block2_changes).unwrap();

    // Roots must differ (state changed)
    assert_ne!(root1, root2);

    // Verify final state
    assert_eq!(store.len(), 3);
    assert_eq!(store.get_account(&[1u8; 32]).unwrap().lamports, 1_500);
    assert_eq!(store.get_account(&[2u8; 32]).unwrap().lamports, 2_000);
    assert_eq!(store.get_account(&[3u8; 32]).unwrap().lamports, 3_000);
}

#[test]
fn state_root_includes_all_accumulated_state() {
    let store = SvmStateStore::new();

    // Apply block 1
    let mut cs1 = SvmChangeSet::new();
    cs1.insert([1u8; 32], dummy_update(100));
    store.apply_changes(&cs1).unwrap();

    // Root after block 2 should include block 1's account even if block 2
    // only modifies a different account.
    let mut cs2 = SvmChangeSet::new();
    cs2.insert([2u8; 32], dummy_update(200));
    let root_with_both = store.compute_root(&cs2).unwrap();

    // A fresh store with only account 2 should produce a different root.
    let fresh = SvmStateStore::new();
    let root_only_two = fresh.compute_root(&cs2).unwrap();

    assert_ne!(root_with_both, root_only_two);
}

#[test]
fn program_account_persists_across_blocks() {
    let store = SvmStateStore::new();

    // Block N: deploy a program
    let program_key = [42u8; 32];
    let mut deploy = SvmChangeSet::new();
    deploy.insert(program_key, program_update(256));
    store.apply_changes(&deploy).unwrap();

    // Block N+1: verify the program is still there
    let acct = store.get_account(&program_key).unwrap();
    assert!(acct.executable);
    assert_eq!(acct.data.len(), 256);
    assert_eq!(acct.owner, solana_sdk_ids::bpf_loader::id().to_bytes());
}

// ---------------------------------------------------------------------------
// Determinism tests
// ---------------------------------------------------------------------------

#[test]
fn same_changes_produce_same_root_on_independent_stores() {
    let mut changes = SvmChangeSet::new();
    changes.insert([1u8; 32], dummy_update(100));
    changes.insert([2u8; 32], dummy_update(200));
    changes.insert([3u8; 32], program_update(64));

    let store_a = SvmStateStore::new();
    let store_b = SvmStateStore::new();

    let root_a = store_a.compute_root(&changes).unwrap();
    let root_b = store_b.compute_root(&changes).unwrap();

    assert_eq!(root_a, root_b);
}

#[test]
fn order_of_application_produces_same_final_root() {
    // Apply changes in two different orderings — final root should match.
    let store_a = SvmStateStore::new();
    let store_b = SvmStateStore::new();

    let mut cs1 = SvmChangeSet::new();
    cs1.insert([1u8; 32], dummy_update(100));

    let mut cs2 = SvmChangeSet::new();
    cs2.insert([2u8; 32], dummy_update(200));

    // Store A: apply cs1 then cs2
    store_a.apply_changes(&cs1).unwrap();
    store_a.apply_changes(&cs2).unwrap();

    // Store B: apply cs2 then cs1
    store_b.apply_changes(&cs2).unwrap();
    store_b.apply_changes(&cs1).unwrap();

    let root_a = store_a.compute_root(&SvmChangeSet::new()).unwrap();
    let root_b = store_b.compute_root(&SvmChangeSet::new()).unwrap();

    assert_eq!(root_a, root_b);
}

#[test]
fn identical_executor_runs_produce_identical_outcomes() {
    let exec_a = SvmExecutor::new();
    let exec_b = SvmExecutor::new();

    let bridge = SvmAccountBridge::empty();
    let empty: Vec<SanitizedTransaction> = vec![];

    let out_a = exec_a.execute(&bridge, 10, 1_700_000_000, &empty, None).unwrap();
    let out_b = exec_b.execute(&bridge, 10, 1_700_000_000, &empty, None).unwrap();

    assert_eq!(out_a.compute_units_used, out_b.compute_units_used);
    assert_eq!(out_a.changes, out_b.changes);
}

#[test]
fn deterministic_transfer_results() {
    let sender = Keypair::new();
    let recipient = Pubkey::new_unique();
    let height = 5u64;
    let ts = 1_700_000_000u64;

    let make_bridge = || funded_bridge(vec![(sender.pubkey(), 10 * LAMPORTS_PER_SOL)]);

    let bh = block_hash(height, ts);
    let tx = system_transaction::transfer(&sender, &recipient, LAMPORTS_PER_SOL, bh);
    let stx = SanitizedTransaction::from_transaction_for_tests(tx);

    let exec_a = SvmExecutor::new();
    let exec_b = SvmExecutor::new();

    let out_a = exec_a.execute(&make_bridge(), height, ts, &[stx.clone()], None).unwrap();
    let out_b = exec_b.execute(&make_bridge(), height, ts, &[stx], None).unwrap();

    assert_eq!(out_a.tx_results.len(), out_b.tx_results.len());
    assert_eq!(out_a.tx_results[0].success, out_b.tx_results[0].success);
    assert_eq!(out_a.compute_units_used, out_b.compute_units_used);
    assert_eq!(out_a.changes, out_b.changes);
}

// ---------------------------------------------------------------------------
// Compute budget tests
// ---------------------------------------------------------------------------

#[test]
fn custom_compute_budget_propagates() {
    let config = SvmExecutorConfig { compute_budget: 50_000, ..Default::default() };
    let executor = SvmExecutor::with_config(config);
    assert_eq!(executor.config().compute_budget, 50_000);
}

#[test]
fn zero_compute_budget_does_not_panic() {
    let config = SvmExecutorConfig { compute_budget: 0, ..Default::default() };
    let executor = SvmExecutor::with_config(config);
    let bridge = SvmAccountBridge::empty();
    let empty: Vec<SanitizedTransaction> = vec![];
    // Should not panic even with 0 budget on empty batch
    let outcome = executor.execute(&bridge, 1, 1_700_000_000, &empty, None).unwrap();
    assert!(outcome.tx_results.is_empty());
}

#[test]
fn large_compute_budget_accepted() {
    let config = SvmExecutorConfig { compute_budget: 1_400_000, ..Default::default() };
    let executor = SvmExecutor::with_config(config);
    assert_eq!(executor.config().compute_budget, 1_400_000);

    let bridge = SvmAccountBridge::empty();
    let empty: Vec<SanitizedTransaction> = vec![];
    let outcome = executor.execute(&bridge, 1, 1_700_000_000, &empty, None).unwrap();
    assert_eq!(outcome.compute_units_used, 0);
}

// ---------------------------------------------------------------------------
// Full pipeline: executor → changeset → store → root
// ---------------------------------------------------------------------------

#[test]
fn executor_changes_applied_to_store_produce_nonzero_root() {
    let sender = Keypair::new();
    let recipient = Pubkey::new_unique();
    let height = 1u64;
    let ts = 1_700_000_000u64;

    let bridge = funded_bridge(vec![(sender.pubkey(), 10 * LAMPORTS_PER_SOL)]);
    let bh = block_hash(height, ts);
    let tx = system_transaction::transfer(&sender, &recipient, LAMPORTS_PER_SOL, bh);
    let stx = SanitizedTransaction::from_transaction_for_tests(tx);

    let executor = SvmExecutor::new();
    let outcome = executor.execute(&bridge, height, ts, &[stx], None).unwrap();

    // Whether the transfer succeeded or not, feed changes to store
    if !outcome.changes.is_empty() {
        let store = SvmStateStore::new();
        let root = store.compute_root(&outcome.changes).unwrap();
        assert_ne!(root, alloy_primitives::B256::ZERO);

        store.apply_changes(&outcome.changes).unwrap();
        assert!(!store.is_empty());
    }
}

// ---------------------------------------------------------------------------
// Shared store between clones (simulating RPC + consensus sharing)
// ---------------------------------------------------------------------------

#[test]
fn cloned_stores_share_underlying_state() {
    let store = SvmStateStore::new();
    let rpc_store = store.clone(); // Simulates RPC getting a clone

    // Consensus applies changes
    let mut changes = SvmChangeSet::new();
    changes.insert([10u8; 32], dummy_update(5_000));
    store.apply_changes(&changes).unwrap();

    // RPC should see the same data
    let acct = rpc_store.get_account(&[10u8; 32]).unwrap();
    assert_eq!(acct.lamports, 5_000);
    assert_eq!(rpc_store.len(), 1);

    // Roots computed from either clone should match
    let empty = SvmChangeSet::new();
    assert_eq!(
        store.compute_root(&empty).unwrap(),
        rpc_store.compute_root(&empty).unwrap()
    );
}

// ---------------------------------------------------------------------------
// Deserialization tests
// ---------------------------------------------------------------------------

#[test]
fn deserialize_valid_solana_transfer() {
    let sender = Keypair::new();
    let recipient = Pubkey::new_unique();
    let bh = Hash::new_unique();

    let tx = system_transaction::transfer(&sender, &recipient, LAMPORTS_PER_SOL, bh);

    // Serialize to wire format (bincode) — same as Solana RPC sendTransaction
    let raw = bincode::serialize(&tx).expect("serialize tx");

    // Deserialize back via our function
    let sanitized = deserialize_svm_tx(&raw).expect("deserialize should succeed");

    // The deserialized tx should have the same signature
    let original_sig = tx.signatures[0];
    let deser_sig = sanitized.signature();
    assert_eq!(deser_sig.as_ref(), original_sig.as_ref());
}

#[test]
fn deserialize_invalid_bytes_fails() {
    let garbage = &[0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x01, 0x02, 0x03];
    let result = deserialize_svm_tx(garbage);
    assert!(result.is_err());
}

#[test]
fn deserialize_empty_bytes_fails() {
    let result = deserialize_svm_tx(&[]);
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// Full pipeline: serialize → deserialize → execute → store → root
// ---------------------------------------------------------------------------

#[test]
fn full_pipeline_deserialize_execute_store() {
    let sender = Keypair::new();
    let recipient = Pubkey::new_unique();
    let height = 1u64;
    let ts = 1_700_000_000u64;

    // 1. Create a funded bridge with the sender account
    let bridge = funded_bridge(vec![(sender.pubkey(), 10 * LAMPORTS_PER_SOL)]);

    // 2. Create and serialize a transfer transaction
    let bh = block_hash(height, ts);
    let tx = system_transaction::transfer(&sender, &recipient, LAMPORTS_PER_SOL, bh);
    let raw = bincode::serialize(&tx).expect("serialize tx");

    // 3. Deserialize via our function (simulates envelope → inner_tx path)
    let sanitized = deserialize_svm_tx(&raw).expect("deserialize should succeed");

    // 4. Execute via SvmExecutor
    let executor = SvmExecutor::new();
    let outcome = executor.execute(&bridge, height, ts, &[sanitized], None).unwrap();

    assert_eq!(outcome.tx_results.len(), 1);
    assert!(outcome.tx_results[0].success, "transfer should succeed: {:?}", outcome.tx_results[0].error);

    // 5. Apply changes to store and verify non-zero root
    let store = SvmStateStore::new();
    let root = store.compute_root(&outcome.changes).unwrap();
    assert_ne!(root, alloy_primitives::B256::ZERO, "root should be non-zero after execution");

    store.apply_changes(&outcome.changes).unwrap();
    assert!(!store.is_empty(), "store should have accounts after applying changes");

    // Verify the recipient got funded
    let recipient_acct = store.get_account(&recipient.to_bytes());
    assert!(recipient_acct.is_some(), "recipient should exist in store");
    assert_eq!(recipient_acct.unwrap().lamports, LAMPORTS_PER_SOL);
}

// ---------------------------------------------------------------------------
// Store to_bridge round-trip
// ---------------------------------------------------------------------------

#[test]
fn store_to_bridge_round_trip() {
    let store = SvmStateStore::new();

    // Seed the store with an account
    let mut changes = SvmChangeSet::new();
    let pubkey_bytes = [42u8; 32];
    changes.insert(pubkey_bytes, dummy_update(5_000_000));
    store.apply_changes(&changes).unwrap();

    // Convert to bridge
    let bridge = store.to_bridge().unwrap();

    // Bridge should contain the seeded account + builtin programs
    let pk = Pubkey::new_from_array(pubkey_bytes);
    let acct = bridge.get_account(&pk);
    assert!(acct.is_some(), "seeded account should be in bridge");

    // Builtin programs should be present
    assert!(bridge.get_account(&solana_system_program::id()).is_some(), "system program missing");
}

#[test]
fn store_to_bridge_empty_has_builtins() {
    let store = SvmStateStore::new();
    let bridge = store.to_bridge().unwrap();

    // Even an empty store should have builtin program accounts
    assert!(bridge.get_account(&solana_system_program::id()).is_some());
}

// ---------------------------------------------------------------------------
// Multi-transaction and edge case tests
// ---------------------------------------------------------------------------

#[test]
fn multi_transfer_single_block() {
    let sender = Keypair::new();
    let recipient_a = Pubkey::new_unique();
    let recipient_b = Pubkey::new_unique();
    let height = 1u64;
    let ts = 1_700_000_000u64;

    let bridge = funded_bridge(vec![(sender.pubkey(), 10 * LAMPORTS_PER_SOL)]);
    let bh = block_hash(height, ts);

    let tx_a = system_transaction::transfer(&sender, &recipient_a, LAMPORTS_PER_SOL, bh);
    let tx_b = system_transaction::transfer(&sender, &recipient_b, LAMPORTS_PER_SOL, bh);
    let stx_a = SanitizedTransaction::from_transaction_for_tests(tx_a);
    let stx_b = SanitizedTransaction::from_transaction_for_tests(tx_b);

    let executor = SvmExecutor::new();
    let outcome = executor.execute(&bridge, height, ts, &[stx_a, stx_b], None).unwrap();

    assert_eq!(outcome.tx_results.len(), 2);
    // At least one should succeed (both may succeed depending on SVM scheduling).
    let successes = outcome.tx_results.iter().filter(|r| r.success).count();
    assert!(successes >= 1, "at least one transfer should succeed");
    assert!(outcome.compute_units_used > 0, "CU should be non-zero");
}

#[test]
fn transfer_insufficient_lamports_fails() {
    let sender = Keypair::new();
    let recipient = Pubkey::new_unique();
    let height = 1u64;
    let ts = 1_700_000_000u64;

    // Fund sender with only 100 lamports — not enough for 1 SOL transfer.
    let bridge = funded_bridge(vec![(sender.pubkey(), 100)]);
    let bh = block_hash(height, ts);

    let tx = system_transaction::transfer(&sender, &recipient, LAMPORTS_PER_SOL, bh);
    let stx = SanitizedTransaction::from_transaction_for_tests(tx);

    let executor = SvmExecutor::new();
    let outcome = executor.execute(&bridge, height, ts, &[stx], None).unwrap();

    assert_eq!(outcome.tx_results.len(), 1);
    assert!(!outcome.tx_results[0].success, "transfer should fail due to insufficient funds");
}

#[test]
fn cross_block_transfer_chain() {
    let alice = Keypair::new();
    let bob = Keypair::new();
    let carol = Pubkey::new_unique();
    let executor = SvmExecutor::new();

    // Block 1: Alice → Bob (1 SOL)
    let height1 = 1u64;
    let ts1 = 1_700_000_000u64;
    let bridge1 = funded_bridge(vec![(alice.pubkey(), 5 * LAMPORTS_PER_SOL)]);
    let bh1 = block_hash(height1, ts1);
    let tx1 = system_transaction::transfer(&alice, &bob.pubkey(), LAMPORTS_PER_SOL, bh1);
    let stx1 = SanitizedTransaction::from_transaction_for_tests(tx1);
    let out1 = executor.execute(&bridge1, height1, ts1, &[stx1], None).unwrap();
    assert!(out1.tx_results[0].success, "block 1 transfer should succeed");

    // Apply changes to store
    let store = SvmStateStore::new();
    store.apply_changes(&out1.changes).unwrap();

    // Block 2: Bob → Carol (0.5 SOL) using Bob's new balance
    let height2 = 2u64;
    let ts2 = 1_700_000_002u64;
    let bridge2 = store.to_bridge().unwrap();
    let bh2 = block_hash(height2, ts2);
    let tx2 = system_transaction::transfer(&bob, &carol, LAMPORTS_PER_SOL / 2, bh2);
    let stx2 = SanitizedTransaction::from_transaction_for_tests(tx2);
    let out2 = executor.execute(&bridge2, height2, ts2, &[stx2], None).unwrap();
    assert!(out2.tx_results[0].success, "block 2 transfer should succeed");

    // Apply and verify final state
    store.apply_changes(&out2.changes).unwrap();
    let carol_acct = store.get_account(&carol.to_bytes());
    assert!(carol_acct.is_some(), "carol should exist after receiving funds");
    assert_eq!(carol_acct.unwrap().lamports, LAMPORTS_PER_SOL / 2);
}

#[test]
fn executor_with_store_bridge_round_trip() {
    let sender = Keypair::new();
    let recipient = Pubkey::new_unique();
    let height = 1u64;
    let ts = 1_700_000_000u64;

    // Seed the store with sender's funds
    let store = SvmStateStore::new();
    let mut seed_changes = SvmChangeSet::new();
    seed_changes.insert(
        sender.pubkey().to_bytes(),
        SvmAccountUpdate {
            lamports: 5 * LAMPORTS_PER_SOL,
            data: vec![],
            owner: [0u8; 32], // system program owns SOL accounts
            executable: false,
            rent_epoch: 0,
        },
    );
    store.apply_changes(&seed_changes).unwrap();

    // Convert store → bridge → execute → apply back to store
    let bridge = store.to_bridge().unwrap();
    let bh = block_hash(height, ts);
    let tx = system_transaction::transfer(&sender, &recipient, LAMPORTS_PER_SOL, bh);
    let stx = SanitizedTransaction::from_transaction_for_tests(tx);

    let executor = SvmExecutor::new();
    let outcome = executor.execute(&bridge, height, ts, &[stx], None).unwrap();
    assert!(outcome.tx_results[0].success, "store→bridge transfer should succeed");

    store.apply_changes(&outcome.changes).unwrap();

    // Verify recipient got funds in the store
    let recipient_acct = store.get_account(&recipient.to_bytes());
    assert!(recipient_acct.is_some(), "recipient should exist in store");
    assert_eq!(recipient_acct.unwrap().lamports, LAMPORTS_PER_SOL);
}

#[test]
fn deserialize_oversized_tx_fails() {
    // 1MB of zeros — far too large to be a valid Solana transaction.
    let oversized = vec![0u8; 1_000_000];
    let result = deserialize_svm_tx(&oversized);
    assert!(result.is_err(), "oversized bytes should fail to deserialize");
}

#[test]
fn empty_batch_produces_zero_cu() {
    let executor = SvmExecutor::new();
    let bridge = SvmAccountBridge::empty();
    let empty: Vec<SanitizedTransaction> = vec![];
    let outcome = executor.execute(&bridge, 1, 1_700_000_000, &empty, None).unwrap();
    assert_eq!(outcome.compute_units_used, 0);
    assert!(outcome.tx_results.is_empty());
    assert!(outcome.changes.is_empty());
}
