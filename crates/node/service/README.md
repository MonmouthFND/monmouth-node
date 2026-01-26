# `kora-service`

[![CI](https://github.com/refcell/kora/actions/workflows/ci.yml/badge.svg)](https://github.com/refcell/kora/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)

Kora node service orchestration.

## Key Types

- `KoraNodeService` - Main service type that orchestrates node components

## Usage

```rust
use kora_config::NodeConfig;
use kora_service::KoraNodeService;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let config = NodeConfig::default();
    let service = KoraNodeService::new(config);
    service.run().await
}
```

## License

[MIT License](https://opensource.org/licenses/MIT)
