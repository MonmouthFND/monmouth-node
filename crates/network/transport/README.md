# `kora-transport`

P2P transport layer for Kora nodes using commonware's authenticated discovery network.

## Overview

This crate provides a simple interface for building production-ready P2P transport
infrastructure. It wraps commonware's `authenticated::discovery` network and exposes
a clean API for registering channels and managing peer connections.

## Usage

```rust,ignore
use kora_transport::{NetworkTransport, TransportConfig};
use commonware_runtime::tokio;

// Create transport config
let config = TransportConfig::recommended(
    signer,
    b"_KORA_NETWORK",
    listen_addr,
    dialable_addr,
    bootstrappers,
    max_message_size,
);

// Build the transport
let transport = config.build(context).await?;

// Access channels for consensus engine
let (vote_sender, vote_receiver) = transport.simplex.votes;
let (cert_sender, cert_receiver) = transport.simplex.certs;

// Access channels for marshal
let (block_sender, block_receiver) = transport.marshal.blocks;

// Register validator set
transport.oracle.update(0, validators).await;
```

## License

[MIT License](https://github.com/refcell/kora/blob/main/LICENSE)
