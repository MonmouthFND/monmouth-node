<p align="center">
    <img src="./assets/monmouth-github.png" alt="Monmouth" width="200">
</p>

<h1 align="center">Monmouth</h1>

<h4 align="center">
    Agent-native L1 blockchain. Dual-VM execution for autonomous AI agents. Built in Rust.
</h4>

<p align="center">
  <a href="https://github.com/MonmouthFND/monmouth-node/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/MonmouthFND/monmouth-node/ci.yml?style=flat&labelColor=1C2C2E&label=ci&logo=GitHub%20Actions&logoColor=white" alt="CI"></a>
  <a href="https://github.com/MonmouthFND/monmouth-node/blob/main/LICENSE"><img src="https://img.shields.io/badge/License-MIT-d1d1f6.svg?style=flat&labelColor=1C2C2E&color=a78bfa&logo=googledocs&logoColor=white" alt="License"></a>
</p>

<p align="center">
  <a href="#whats-monmouth">What's Monmouth?</a> •
  <a href="#native-modules">Native Modules</a> •
  <a href="#architecture">Architecture</a> •
  <a href="#usage">Usage</a> •
  <a href="#contributing">Contributing</a>
</p>

> [!CAUTION]
> Monmouth is pre-alpha software.

## What's Monmouth?

Monmouth is an agent-native L1 blockchain — a deterministic execution substrate that LLMs call as a tool. Intelligence lives off-chain in agent runtimes. The chain provides dual-VM execution (EVM + SVM), policy guardrails, intent receipts, simulation, and cryptographic attestation.

LLMs read state, simulate outcomes, submit transactions, verify results, and repeat. Monmouth provides the on-chain primitives that make this loop safe, auditable, and composable across agents.

## Why?

General-purpose chains treat agent transactions like any other. Agents need more: scoped permissions with session keys, spending caps enforced at the protocol level, intent declarations paired with execution outcomes, preflight simulation before committing state, and native coordination primitives for multi-agent workflows. Monmouth is purpose-built with these as first-class protocol features.

## Three-Layer Architecture

| Layer | Where | What |
|---|---|---|
| **A — Agent Runtime** | Off-chain | LLM planning loop, tool calling, memory, signing |
| **B — Execution** | On-chain | Deterministic EVM + SVM, state, typed endpoints |
| **C — Bridge** | On-chain modules | Policy engine, simulation, attestation, intent receipts |

## Native Modules

Twelve on-chain modules replace traditional precompiles with typed, agent-aware primitives:

### Core Agent Loop

| Module | Crate | Purpose |
|---|---|---|
| **Capability Registry** | `monmouth-capabilities` | Tool catalog with typed schemas, permissions, rate limits |
| **Transaction Envelope** | `monmouth-envelope` | Extended tx format with VM routing (`0x4d` magic byte), session hints, intent declarations |
| **Simulation / Preview** | `monmouth-simulation` | Deterministic preflight with pluggable backends |
| **Policy Engine** | `monmouth-policy` | Spending caps, rate limits, allowlists, human-confirmation flags |
| **Intent Receipts** | `monmouth-intent-receipts` | Declared intent vs. actual outcome audit trail |

### Agent Economy Layer

| Module | Crate | Purpose |
|---|---|---|
| **Delegation** | `monmouth-delegation` | Scoped session keys with secp256k1 crypto, time/spend limits |
| **State Observation** | `monmouth-state-observation` | Structured reads: balance, nonce, storage, ERC-20, contract state |
| **Memory Anchoring** | `monmouth-memory-anchoring` | On-chain commitments to off-chain agent context |
| **Coordination** | `monmouth-coordination` | ACP-compatible job lifecycle, escrow, multi-agent settlement |
| **Conditional Subscriptions** | `monmouth-conditional-subs` | Trigger-based notifications: balance thresholds, storage changes, events |
| **Attestation** | `monmouth-attestation` | Cryptographic proofs (secp256k1 native, Ed25519/TEE/ZK extensible) |
| **SVM Module** | `monmouth-svm` | Native Solana VM alongside EVM, routed via envelope |

### Smart Contracts (ERC-8004)

| Contract | Purpose |
|---|---|
| `IdentityRegistry.sol` | ERC-721 agent identity NFTs with metadata |
| `ReputationRegistry.sol` | Signed feedback scores, revocation, tag-based filtering |
| `ValidationRegistry.sol` | Independent capability verification requests/responses |

## Architecture

Built on [Commonware](https://github.com/commonwarexyz/monorepo) with dual-VM execution:

| Layer | Implementation |
|---|---|
| **Consensus** | BFT Simplex with BLS12-381 threshold signatures |
| **Execution** | REVM v34 (EVM) + Solana SVM 3.1.9, dual-VM block building via transaction envelope routing |
| **Storage** | QMDB with separate state roots per VM (EVM accounts/storage/code + SVM account tree) |
| **Networking** | Commonware P2P transport with message marshaling |
| **RPC** | 52 endpoints — full `eth_*` JSON-RPC, `monmouth_*` agent APIs, WebSocket pub/sub, filter polling |
| **Contracts** | ERC-8004 identity, reputation, and validation registries (Foundry + OpenZeppelin) |

### Dual-VM Execution

Transactions are partitioned at the consensus layer by VM target:

- **EVM path**: Standard Ethereum transactions and EVM-targeted envelopes execute via REVM
- **SVM path**: Solana-targeted envelopes (`VmTarget::Svm`) unwrap inner bytes, deserialize via bincode, and execute via `TransactionBatchProcessor`
- Each VM computes an independent state root; both are committed in the block header
- Routing is deterministic — the `0x4d` magic byte and `VmTarget` field in the envelope control dispatch

### RPC Endpoints

- **`eth_*`** (26 methods) — Full Ethereum JSON-RPC compatibility
- **`monmouth_*`** (7 methods) — `nodeStatus`, `listCapabilities`, `getCapability`, `getCapabilitySchema`, `svmStatus`, `svmGetAccount`, `svmGetProgramInfo`
- **Filters** (6 methods) — `newFilter`, `newBlockFilter`, `getFilterChanges`, `getFilterLogs`, etc.
- **WebSocket** — `eth_subscribe`/`eth_unsubscribe` for `newHeads`, `logs`, `pendingTransactions`

### Configuration

| Parameter | Default |
|---|---|
| Chain ID | `7750` |
| Hardfork Spec | Prague |
| Gas Limit | 30,000,000 |
| Block Time | 2s |
| SVM Compute Budget | 200,000 CU |

All native modules are opt-in via TOML configuration. CLI flags override config values.

### Crate Layout

```
bin/monmouth/                  CLI binary (clap derive)
crates/
  node/
    agent-types/               Shared agent type definitions
    attestation/               Cryptographic proof registry
    capabilities/              Tool catalog and schemas
    conditional-subs/          Trigger-based subscriptions
    config/                    TOML/JSON config for all modules
    consensus/                 BFT Simplex application layer
    coordination/              Multi-agent job lifecycle + escrow
    delegation/                Session keys + scoped permissions
    domain/                    Block, Tx, StateRoot, identifiers
    envelope/                  Extended tx format with VM routing
    executor/                  REVM v34 block executor
    intent-receipts/           Intent vs. outcome audit trail
    ledger/                    Mempool, snapshots, QMDB ledger
    memory-anchoring/          Off-chain context commitments
    policy/                    Spending caps + rate limits
    rpc/                       JSON-RPC + WebSocket server
    runner/                    Production validator runner
    simulation/                Preflight simulation provider
    state-observation/         Structured state queries
    svm/                       Native Solana VM executor
  storage/
    qmdb/                      Triple-partitioned state storage
  utilities/
    verification/              Q8.24 fixed-point math
contracts/                     Foundry project (ERC-8004)
```

## Tests

980+ tests across Rust and Solidity:

| Category | Count |
|---|---|
| Rust unit + integration tests | 910+ |
| Solidity tests | 71 |

```sh
cargo test --all          # Run all Rust tests
cd contracts && forge test  # Run Solidity tests
```

## Usage

Start the devnet with interactive DKG:

```sh
just devnet
```

> [!TIP]
> See the [Justfile](./Justfile) for other useful commands.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
