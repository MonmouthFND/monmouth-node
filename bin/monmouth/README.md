# `monmouth`

[![CI](https://github.com/MonmouthFND/monmouth-node/actions/workflows/ci.yml/badge.svg)](https://github.com/MonmouthFND/monmouth-node/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)

The main Monmouth node binary. Implements BLS12-381 threshold consensus via [commonware], EVM execution with [revm], and state storage using [QMDB].

[commonware]: https://github.com/commonwarexyz/monorepo
[revm]: https://github.com/bluealloy/revm
[QMDB]: https://github.com/commonwarexyz/monorepo/tree/main/storage

## Usage

Start the devnet with interactive DKG (Distributed Key Generation):

```bash
just devnet
```

Run with a custom configuration file:

```bash
monmouth --config /path/to/config.toml
```

Run the DKG ceremony:

```bash
monmouth dkg --peers peers.json
```

Run as a validator (requires completed DKG):

```bash
monmouth validator --peers peers.json
```

The `--chain-id` and `--data-dir` flags can override configuration values. Set `RUST_LOG` to control log level (e.g., `info`, `debug`, `monmouth=trace`).

## Configuration

Monmouth uses TOML configuration files. See [`monmouth-config`](../../crates/node/config) for the full schema.

```toml
chain_id = 1337
data_dir = "/var/lib/monmouth"

[network]
listen_addr = "0.0.0.0:9000"
bootstrap_peers = []
```

## License

This project is licensed under the MIT License - see the [LICENSE](../../LICENSE) file for details.
