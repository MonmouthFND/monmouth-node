# Kora REVM Simulation Example

[![CI](https://github.com/refcell/kora/actions/workflows/ci.yml/badge.svg)](https://github.com/refcell/kora/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](../../LICENSE)

Demonstrates running kora as a simulation.

REVM-based example chain driven by threshold-simplex.

This example uses `alloy-evm` as the integration layer
above `revm` and keeps the execution backend generic over
the database trait boundary (`Database` + `DatabaseCommit`).

## Running

```sh
cargo run -p kora-revm-example
```

## What it demonstrates

- **Threshold BLS consensus** via commonware simplex
- **REVM execution** with alloy-evm integration
- **Generic database backend** over the `Database` + `DatabaseCommit` trait boundary

## License

This project is licensed under the [MIT License](../../LICENSE).
