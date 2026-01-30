# `kora-rpc`

[![CI](https://github.com/refcell/kora/actions/workflows/ci.yml/badge.svg)](https://github.com/refcell/kora/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)

RPC server for Kora nodes. Provides HTTP endpoints for querying node status and chain state, as well as a full Ethereum JSON-RPC 2.0 API implementation.

## Key Types

- `RpcServer` - Main RPC server implementation
- `NodeState` - Current state of the node
- `EthApiServer` - Ethereum JSON-RPC API server trait
- `StateProvider` - Provider for accessing chain state

## License

[MIT License](https://opensource.org/licenses/MIT)
