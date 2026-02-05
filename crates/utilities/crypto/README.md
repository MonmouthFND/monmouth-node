# `monmouth-crypto`

<a href="https://github.com/monmouth-ai/monmouth/actions/workflows/ci.yml"><img src="https://github.com/monmouth-ai/monmouth/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
<a href="https://github.com/monmouth-ai/monmouth/blob/main/LICENSE"><img src="https://img.shields.io/badge/License-MIT-d1d1f6.svg" alt="License"></a>

Cryptographic utilities for Monmouth.

## Features

- `test-utils` - Enables test utilities including `threshold_schemes` for generating deterministic threshold BLS signing schemes.

## Usage

```toml
[dependencies]
monmouth-crypto = { path = "crates/utilities/crypto" }

# For testing
[dev-dependencies]
monmouth-crypto = { path = "crates/utilities/crypto", features = ["test-utils"] }
```

## License

[MIT License](https://github.com/monmouth-ai/monmouth/blob/main/LICENSE)
