# `monmouth-consensus`

<a href="https://github.com/monmouth-ai/monmouth/actions/workflows/ci.yml"><img src="https://github.com/monmouth-ai/monmouth/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
<a href="https://github.com/monmouth-ai/monmouth/blob/main/LICENSE"><img src="https://img.shields.io/badge/License-MIT-d1d1f6.svg" alt="License"></a>

Consensus application layer for Monmouth.

This crate provides the bridge between Commonware consensus and REVM execution,
using trait-abstracted components for modularity.

## Key Types

- `ConsensusApplication` - Implements Commonware's Application trait
- `Block` - Commonware-compatible block type from `monmouth-domain`
- `ExecutionOutcome` - Result of block execution

## Traits

All components are trait-abstracted for swappability:

- `Mempool` - Pending transaction pool
- `SnapshotStore` - Execution state caching
- `SeedTracker` - VRF seed management
- `BlockExecutor` - Transaction execution

## Architecture

```text
+--------------------------------------------------+
|              monmouth-consensus                       |
|                                                   |
|  ConsensusApplication<M, S, SS, ST, E>           |
|       |         |        |       |       |       |
|       v         v        v       v       v       |
|   Mempool   StateDb  Snapshot  Seed   Block     |
|   trait     trait    Store    Tracker Executor  |
+--------------------------------------------------+
        |         |
        v         v
   monmouth-traits  monmouth-handlers
```

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
monmouth-consensus = { path = "crates/node/consensus" }
```

## License

[MIT License](https://github.com/monmouth-ai/monmouth/blob/main/LICENSE)
