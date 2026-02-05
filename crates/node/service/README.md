# `monmouth-service`

[![CI](https://github.com/monmouth-ai/monmouth/actions/workflows/ci.yml/badge.svg)](https://github.com/monmouth-ai/monmouth/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)

Monmouth node service orchestration.

## Key Types

- `MonmouthNodeService` - Main service type that orchestrates node components

## Usage

```rust,ignore
use monmouth_config::NodeConfig;
use monmouth_service::MonmouthNodeService;

fn main() -> eyre::Result<()> {
    let config = NodeConfig::default();
    let service = MonmouthNodeService::new(config);
    service.run()
}
```

## License

[MIT License](https://opensource.org/licenses/MIT)
