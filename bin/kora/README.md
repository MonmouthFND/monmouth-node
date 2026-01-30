# `kora`

[![CI](https://github.com/refcell/kora/actions/workflows/ci.yml/badge.svg)](https://github.com/refcell/kora/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)

The main Kora node binary. Implements BLS12-381 threshold consensus via [commonware], EVM execution with [revm], and state storage using [QMDB].

[commonware]: https://github.com/commonwarexyz/monorepo
[revm]: https://github.com/bluealloy/revm
[QMDB]: https://github.com/LayerZero-Labs/qmdb

## Usage

### Running the Devnet

```bash
just devnet
```

### CLI Options

```bash
# Run with default configuration (legacy mode)
kora

# Run with a custom configuration file
kora --config /path/to/config.toml

# Run with CLI overrides
kora --chain-id 1 --data-dir /path/to/data

# Run DKG ceremony
kora dkg --peers peers.json

# Run as validator (requires completed DKG)
kora validator --peers peers.json
```

| Flag | Description |
|------|-------------|
| `-c, --config <FILE>` | Path to TOML configuration file |
| `--chain-id <ID>` | Override the chain ID |
| `--data-dir <PATH>` | Override the data directory |
| `-v, --verbose` | Enable verbose logging |

### Environment Variables

| Variable | Description |
|----------|-------------|
| `RUST_LOG` | Controls log level (e.g., `info`, `debug`, `kora=trace`) |

## Configuration

Kora uses TOML configuration files. See [`kora-config`](../../crates/node/config) for the full schema.

```toml
chain_id = 1337
data_dir = "/var/lib/kora"

[network]
listen_addr = "0.0.0.0:9000"
bootstrap_peers = []
```

## Related Crates

- [`kora-service`](../../crates/node/service) - Node service orchestration
- [`kora-config`](../../crates/node/config) - Configuration types and loading

## License

[MIT License](https://opensource.org/licenses/MIT)
