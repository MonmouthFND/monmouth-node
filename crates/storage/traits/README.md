# `monmouth-traits`

<a href="https://github.com/MonmouthFND/monmouth-node/actions/workflows/ci.yml"><img src="https://github.com/MonmouthFND/monmouth-node/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
<a href="https://github.com/MonmouthFND/monmouth-node/blob/main/LICENSE"><img src="https://img.shields.io/badge/License-MIT-d1d1f6.svg" alt="License"></a>

Core trait abstractions for Monmouth storage and consensus.

This crate defines the high-level interfaces that bridge storage implementations
with consensus requirements. Implementations live in downstream crates.

## Key Traits

- `StateDb` - High-level state database interface for consensus
- `StateDbRead` - Read-only state access
- `StateDbWrite` - State mutation operations

## Architecture

```text
                    monmouth-consensus
                         |
                         | uses trait bounds
                         v
+--------------------monmouth-traits---------------------+
|                                                    |
|  StateDb: StateDbRead + StateDbWrite + ...        |
|                                                    |
+----------------------------------------------------+
                         ^
                         | implements
                         |
                    monmouth-handlers
                         |
                         v
                     monmouth-qmdb
```

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
monmouth-traits = { path = "crates/storage/traits" }
```

Define bounds using the traits:

```rust,ignore
use monmouth_traits::StateDb;

fn execute<S: StateDb>(state: &S) {
    // Use state database through trait
}
```

## License

[MIT License](https://github.com/MonmouthFND/monmouth-node/blob/main/LICENSE)
