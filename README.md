<p align="center">
    <img src="./assets/monmouth-github.png" alt="Monmouth" width="200">
</p>

<h1 align="center">Monmouth</h1>

<h4 align="center">
    The settlement layer for autonomous AI agents. Built in Rust.
</h4>

<p align="center">
  <a href="https://github.com/MonmouthFND/monmouth-node/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/MonmouthFND/monmouth-node/ci.yml?style=flat&labelColor=1C2C2E&label=ci&logo=GitHub%20Actions&logoColor=white" alt="CI"></a>
  <a href="https://github.com/MonmouthFND/monmouth-node/blob/main/LICENSE"><img src="https://img.shields.io/badge/License-MIT-d1d1f6.svg?style=flat&labelColor=1C2C2E&color=a78bfa&logo=googledocs&logoColor=white" alt="License"></a>
</p>

<p align="center">
  <a href="#whats-monmouth">What's Monmouth?</a> •
  <a href="#agent-primitives">Agent Primitives</a> •
  <a href="#architecture">Architecture</a> •
  <a href="#usage">Usage</a> •
  <a href="#contributing">Contributing</a>
</p>

> [!CAUTION]
> Monmouth is pre-alpha software.

## What's Monmouth?

Monmouth is consensus infrastructure designed for AI agents to transact safely across blockchain ecosystems. General-purpose chains treat agent transactions like any other — Monmouth doesn't. It provides agent-native primitives that let autonomous agents operate with identity, reputation, and coordination built into the protocol.

LLMs handle reasoning and planning off-chain. Monmouth handles verification and settlement on-chain. Agents operate autonomously within safe boundaries.

## Why?

AI agents need more than gas. They need native intent resolution, multi-step autonomous workflows, cross-chain identity, and agent-to-agent payment protocols. None of this exists on general-purpose chains. Monmouth is purpose-built to fill that gap with agent-native primitives at the protocol level.

## Agent Primitives

- **Agent Identity Registry** — Canonical on-chain identities ([ERC-8004](https://github.com/ethereum/ERCs/pull/655)) with cross-chain verification. Each agent receives an NFT-based identity with metadata, wallet delegation, and a registration URI.
- **Reputation System** — On-chain feedback with signed scores, tags, revocation support, and aggregated summaries. Agents build verifiable track records.
- **Validation Framework** — Independent capability verification through a request-response pattern. Validators attest to agent capabilities with categorized responses.
- **Native Intent Resolution** — Agent intents as first-class transaction types, classified and routed before execution.
- **Custom Precompiles** — Protocol-level operations for AI inference, vector similarity, intent parsing, cross-chain messaging, and SVM routing.
- **Transaction Classification** — Pre-execution classification of agent transactions (pure EVM, SVM-routed, hybrid cross-chain, RAG-enhanced, agent-to-agent) with configurable confidence thresholds.

## Architecture

Monmouth is built on [Commonware](https://github.com/commonwarexyz/monorepo) with a modular architecture:

| Layer | Implementation |
|---|---|
| **Consensus** | BLS12-381 threshold signatures via Commonware Simplex |
| **Execution** | REVM with custom agent precompiles and transaction classifier |
| **Storage** | QMDB for high-performance state management |
| **Networking** | P2P transport with message marshaling |
| **Contracts** | ERC-8004 identity, reputation, and validation registries |

The devnet runs in three phases: key generation (ed25519 identity keys), DKG ceremony (collaborative BLS12-381 threshold key generation), and validator launch (full consensus + execution + storage).

### Configuration

| Parameter | Default |
|---|---|
| Chain ID | `7750` |
| Hardfork Spec | Prague |
| Gas Limit | 30,000,000 |
| Block Time | 2s |

Agent features are opt-in via CLI flags:

```sh
monmouth validator --enable-agent-pool --confidence-threshold 0.8
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
