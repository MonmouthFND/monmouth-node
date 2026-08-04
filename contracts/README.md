# Monmouth Contracts

ERC-8004 aligned agent registries for the Monmouth network — on-chain identity,
reputation, and validation for autonomous agents.

These contracts form the on-chain trust layer that agents (driven by off-chain LLM
runtimes) read and write to. They are deliberately deterministic and dumb: no
inference, no scoring heuristics, no ML. Intelligence lives off-chain; the chain
records claims, feedback, and validations.

## Contracts

### `IdentityRegistry` ([src](src/IdentityRegistry.sol))

ERC-721 (`Monmouth Agent Identity` / `MAID`) where each token is an agent's
on-chain identity. Token URIs point to an ERC-8004 registration JSON describing
the agent's name, services, `x402Support`, and `supportedTrust`.

| Function | Notes |
| --- | --- |
| `register(agentURI) → agentId` | Mints the next agent ID (starts at 1). Reverts `EmptyURI` on empty input. |
| `setAgentWallet(agentId, wallet)` | Owner-only. Designates the address authorized to transact for the agent. |
| `getAgentWallet(agentId)` | Zero address if unset. |
| `setMetadata(agentId, key, value)` | Owner-only arbitrary `string → bytes` store. |
| `getMetadata(agentId, key)` | Empty bytes if unset. |
| `totalAgents()` | Count of agents registered so far. |

### `ReputationRegistry` ([src](src/ReputationRegistry.sol))

Signed, tagged feedback per agent, with revocation and paginated aggregation.
Constructed with the `IdentityRegistry` address and a per-sender cooldown
(`0` disables it).

| Function | Notes |
| --- | --- |
| `giveFeedback(agentId, value, decimals, tag1, tag2, endpoint, feedbackURI, feedbackHash)` | Verifies the agent exists, enforces the cooldown, and locks `decimals` per agent on first submission. |
| `revokeFeedback(feedbackId)` | Original submitter only. Revoked entries are skipped in summaries. |
| `getSummary(agentId, clients, tag1, tag2, offset, limit)` | Returns `(count, summaryValue, decimals, nextOffset)`. Empty `clients`/tags skip that filter; `limit = 0` means no limit. |
| `getFeedback(feedbackId)` | Reverts `FeedbackNotFound` for out-of-range IDs. |
| `totalFeedback()` | Includes revoked entries. |

`getSummary` is paginated on purpose — agents with large feedback histories would
otherwise run out of gas. Callers should loop until `nextOffset == 0`.

### `ValidationRegistry` ([src](src/ValidationRegistry.sol))

Request/response validation: a requester nominates a validator for an agent, and
only that validator may respond, exactly once. Requests are keyed by a caller-supplied
`requestHash`.

| Function | Notes |
| --- | --- |
| `validationRequest(validator, agentId, requestURI, requestHash)` | Rejects zero validators, duplicate hashes, unknown agents, and cooldown violations. |
| `validationResponse(requestHash, response, responseURI, responseHash, tag)` | Designated validator only. Response must be `1`–`3`. |
| `getRequest(requestHash)` | Returns `(agentId, requester, validator, responded)`. |
| `getResponse(requestHash)` | Reverts if the request is unknown or unanswered. |
| `getAgentRequestCount` / `getAgentRequestAt` | Enumerate an agent's requests. |

Response codes: `0` pending, `1` approved, `2` rejected, `3` inconclusive.

> **Front-running:** `requestHash` is the request's unique key, so an observer can
> claim a pending hash first and make the original transaction revert. Bind the
> hash to the caller: `keccak256(abi.encode(msg.sender, agentId, nonce))`.

## Layout

```
contracts/
├── src/            IdentityRegistry, ReputationRegistry, ValidationRegistry
├── test/           Forge tests (71 total)
├── script/         Deployment scripts
├── lib/            openzeppelin-contracts, forge-std (git submodules)
└── foundry.toml    solc 0.8.24, cancun, via-ir, optimizer 200 runs
```

`via_ir = true` is required — `ReputationRegistry.getSummary` hits "stack too deep"
without it.

## Development

Requires [Foundry](https://book.getfoundry.sh/). Submodules must be present:

```bash
forge install
```

Build:

```bash
forge build
```

Test — 71 tests across the three registries (23 identity, 27 reputation, 21 validation):

```bash
forge test
```

Verbose failures and gas:

```bash
forge test -vvv --gas-report
```

Format:

```bash
forge fmt
```

## Security

Contracts follow Trail of Bits guidance: `ReentrancyGuard` on state-changing entry
points, checks-effects-interactions ordering, custom errors instead of revert
strings, events on every state change, and NatSpec on all public/external functions.
Both `ReputationRegistry` and `ValidationRegistry` verify agent existence through
`IdentityRegistry.ownerOf` before writing.

Static analysis with [Slither](https://github.com/crytic/slither):

```bash
slither . --json slither-report.json
```

The `/solidity-auditor` and `/secure-workflow-guide` skills cover the fuller audit
workflow.

## Deployment

`ReputationRegistry` and `ValidationRegistry` both take the identity registry
address in their constructor, so ordering matters.
[`script/Deploy.s.sol`](script/Deploy.s.sol) handles it:

```bash
forge script script/Deploy.s.sol:Deploy --rpc-url <rpc> --private-key <key> --broadcast
```

Drop `--broadcast` for a simulation — the script prints all three addresses and the
configured cooldowns either way.

Cooldowns (seconds between submissions per sender, `0` disables) come from the
environment, defaulting to 60:

```bash
FEEDBACK_COOLDOWN=300 REQUEST_COOLDOWN=0 forge script script/Deploy.s.sol:Deploy --rpc-url <rpc> --private-key <key> --broadcast
```

Monmouth's own chain ID is `7750` with 2s blocks.
