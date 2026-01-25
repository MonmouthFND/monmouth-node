# kora-handlers

Thread-safe database handles for Kora.

This crate provides synchronized wrappers around storage backends,
implementing REVM database traits for EVM execution.

## Key Types

- `QmdbHandle` - Thread-safe handle to QMDB stores with `Arc<RwLock>` synchronization
- `HandleError` - Error type implementing REVM's `DBErrorMarker`

## Usage

```rust,ignore
use kora_handlers::QmdbHandle;
use revm::database_interface::DatabaseRef;

// Create handle from stores
let handle = QmdbHandle::new(accounts, storage, code);

// Use as REVM database
let account = handle.basic_ref(address)?;
```

## Design

This crate implements Layer 2 of a 2-layer architecture:

1. **Layer 1 (kora-qmdb)**: Pure store logic, state transitions, no synchronization
2. **Layer 2 (this crate)**: Thread-safe handles, REVM trait implementations
