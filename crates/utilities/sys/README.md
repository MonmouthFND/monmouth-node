# `monmouth-sys`

<a href="https://github.com/MonmouthFND/monmouth-node/actions/workflows/ci.yml"><img src="https://github.com/MonmouthFND/monmouth-node/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
<a href="https://github.com/MonmouthFND/monmouth-node/blob/main/LICENSE"><img src="https://img.shields.io/badge/License-MIT-d1d1f6.svg" alt="License"></a>

System utilities for Monmouth.

## Key Types

- `FileLimitHandler` - Best-effort handler for raising process file descriptor limits

## Usage

```rust,ignore
use monmouth_sys::FileLimitHandler;

// Attempt to raise file descriptor limits
FileLimitHandler::raise_limits();
```

## License

[MIT License](https://github.com/MonmouthFND/monmouth-node/blob/main/LICENSE)
