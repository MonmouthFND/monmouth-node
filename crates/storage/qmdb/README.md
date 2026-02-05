# `monmouth-qmdb`

<a href="https://github.com/MonmouthFND/monmouth-node/actions/workflows/ci.yml"><img src="https://github.com/MonmouthFND/monmouth-node/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
<a href="https://github.com/MonmouthFND/monmouth-node/blob/main/LICENSE"><img src="https://img.shields.io/badge/License-MIT-d1d1f6.svg" alt="License"></a>

Core QMDB abstractions and traits for Monmouth.

This crate provides the foundational types and traits for QMDB storage without concrete implementations. For storage backends, see [`monmouth-backend`](../backend). For thread-safe access, see [`monmouth-handlers`](../handlers).

## Key Types

- `QmdbStore` - Owns three QMDB partitions (accounts, storage, code)
- `ChangeSet` - Accumulated state changes with merge capability
- `StoreBatches` - Batch operations for atomic writes
- `QmdbGettable` / `QmdbBatchable` - Traits for store backends

## Usage

```rust,ignore
use monmouth_qmdb::{QmdbStore, ChangeSet, AccountUpdate};

// Create store from backends (that implement QmdbGettable/QmdbBatchable)
let mut store = QmdbStore::new(accounts, storage, code);

// Build and apply changes
let changes = ChangeSet::new();
store.commit_changes(changes)?;
```

## License

[MIT License](https://github.com/MonmouthFND/monmouth-node/blob/main/LICENSE)
