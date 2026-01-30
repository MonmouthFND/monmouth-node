# `kora-e2e`

[![CI](https://github.com/refcell/kora/actions/workflows/ci.yml/badge.svg)](https://github.com/refcell/kora/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)

End-to-end testing framework for Kora consensus network.

This crate provides a simulation-based testing infrastructure for running
multi-validator consensus tests without real networking. It enables testing
of:

- Consensus finalization across multiple validators
- Transaction execution and state convergence
- Block production with proper ordering
- Network partition recovery
- Contract deployment and interaction

## Key Types

- `TestHarness` - Main test orchestration harness
- `TestNode` - Individual validator node in tests
- `TestSetup` - Test configuration and initialization
- `TestConfig` - Configuration for test scenarios

## Running Tests

Tests in this crate use file-based storage and are resource-intensive.
For reliable results, run with a single test thread:

```bash
cargo test -p kora-e2e -- --test-threads=1
```

## License

[MIT License](https://opensource.org/licenses/MIT)
