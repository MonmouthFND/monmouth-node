# `kora-qmdb`

<a href="https://github.com/refcell/kora/actions/workflows/ci.yml"><img src="https://github.com/refcell/kora/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
<a href="https://github.com/refcell/kora/blob/main/LICENSE"><img src="https://img.shields.io/badge/License-MIT-d1d1f6.svg" alt="License"></a>

Pure QMDB store logic for Kora.

This crate provides low-level store management without synchronization.
For thread-safe access, use `kora-handlers`.

## Key Types

- `QmdbStore` - Owns three QMDB stores (accounts, storage, code)
- `ChangeSet` - Accumulated state changes with merge capability
- `StoreBatches` - Batch operations for atomic writes
- `QmdbGettable` / `QmdbBatchable` - Traits for store backends

## Architecture

```text
┌─────────────────────────────────┐
│       kora-handlers             │  (Layer 2: Sync + Public API)
│       QmdbHandle                │
└───────────────┬─────────────────┘
                │ Arc<RwLock<QmdbStore>>
                ▼
┌─────────────────────────────────┐
│         kora-qmdb               │  (Layer 1: Pure store logic)
│         QmdbStore               │
│  (accounts, storage, code)      │
└───────────────┬─────────────────┘
                │ QmdbGettable / QmdbBatchable
                ▼
┌─────────────────────────────────┐
│       Backend Stores            │
└─────────────────────────────────┘
```

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
kora-qmdb = { path = "crates/storage/qmdb" }
```

Use the store:

```rust,ignore
use kora_qmdb::{QmdbStore, ChangeSet, AccountUpdate};

// Create store from backends
let mut store = QmdbStore::new(accounts, storage, code);

// Build and apply changes
let changes = ChangeSet::new();
store.commit_changes(changes)?;
```

## License

[MIT License](https://github.com/refcell/kora/blob/main/LICENSE)
