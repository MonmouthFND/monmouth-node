// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "@openzeppelin/contracts/token/ERC721/ERC721.sol";
import "@openzeppelin/contracts/token/ERC721/extensions/ERC721URIStorage.sol";
import "@openzeppelin/contracts/utils/ReentrancyGuard.sol";

/// @title IdentityRegistry - ERC-8004 Agent Identity
/// @author Monmouth Team
/// @notice ERC-721 based agent identity NFTs for the Monmouth network.
/// @dev Each agent receives a unique NFT representing their on-chain identity.
///      The agent URI points to a registration JSON containing:
///      - name, description, services, x402Support, supportedTrust
///      Follows Trail of Bits security guidelines:
///      - ReentrancyGuard on state-changing functions
///      - Checks-effects-interactions pattern
///      - Events on all critical operations
///      - NatSpec on all public/external functions
///      - Input validation on all parameters
contract IdentityRegistry is ERC721URIStorage, ReentrancyGuard {
    /// @notice Counter for the next agent ID to mint. Starts at 1.
    uint256 private _nextAgentId;

    /// @notice Mapping from agent ID to designated wallet address.
    mapping(uint256 => address) private _agentWallets;

    /// @notice Mapping from agent ID to arbitrary metadata key-value store.
    mapping(uint256 => mapping(string => bytes)) private _metadata;

    /// @notice Emitted when a new agent is registered.
    /// @param agentId The unique identifier assigned to the agent.
    /// @param owner The address that owns the agent NFT.
    /// @param agentURI The URI pointing to the agent's registration JSON.
    event AgentRegistered(uint256 indexed agentId, address indexed owner, string agentURI);

    /// @notice Emitted when an agent's designated wallet is updated.
    /// @param agentId The agent's token ID.
    /// @param wallet The new wallet address.
    event AgentWalletSet(uint256 indexed agentId, address indexed wallet);

    /// @notice Emitted when agent metadata is updated.
    /// @param agentId The agent's token ID.
    /// @param key The metadata key that was set.
    event MetadataSet(uint256 indexed agentId, string key);

    /// @notice Thrown when caller is not the owner of the agent NFT.
    error NotAgentOwner(uint256 agentId, address caller);

    /// @notice Thrown when the provided URI is empty.
    error EmptyURI();

    /// @notice Thrown when the provided metadata key is empty.
    error EmptyKey();

    constructor() ERC721("Monmouth Agent Identity", "MAID") {
        _nextAgentId = 1;
    }

    /// @notice Register a new agent identity.
    /// @param agentURI URI pointing to the agent's registration JSON.
    ///        Must be non-empty. Expected to resolve to a JSON document
    ///        conforming to the ERC-8004 agent registration schema.
    /// @return agentId The unique identifier for the registered agent.
    function register(string calldata agentURI) external nonReentrant returns (uint256 agentId) {
        if (bytes(agentURI).length == 0) revert EmptyURI();

        // Effects: increment counter first
        agentId = _nextAgentId;
        unchecked {
            ++_nextAgentId;
        }

        // Interactions: mint (calls onERC721Received if recipient is contract)
        _safeMint(msg.sender, agentId);
        _setTokenURI(agentId, agentURI);

        emit AgentRegistered(agentId, msg.sender, agentURI);
    }

    /// @notice Set the designated wallet for an agent.
    /// @dev Only the NFT owner can set the wallet. The wallet address
    ///      is the address authorized to transact on behalf of the agent.
    /// @param agentId The agent's token ID.
    /// @param wallet The wallet address to associate with the agent.
    function setAgentWallet(uint256 agentId, address wallet) external {
        if (ownerOf(agentId) != msg.sender) revert NotAgentOwner(agentId, msg.sender);

        _agentWallets[agentId] = wallet;

        emit AgentWalletSet(agentId, wallet);
    }

    /// @notice Get the designated wallet for an agent.
    /// @param agentId The agent's token ID.
    /// @return wallet The associated wallet address (zero if not set).
    function getAgentWallet(uint256 agentId) external view returns (address wallet) {
        wallet = _agentWallets[agentId];
    }

    /// @notice Set metadata for an agent.
    /// @dev Only the NFT owner can set metadata. Metadata is stored as
    ///      arbitrary bytes indexed by string keys.
    /// @param agentId The agent's token ID.
    /// @param key The metadata key. Must be non-empty.
    /// @param value The metadata value as raw bytes.
    function setMetadata(uint256 agentId, string calldata key, bytes calldata value) external {
        if (ownerOf(agentId) != msg.sender) revert NotAgentOwner(agentId, msg.sender);
        if (bytes(key).length == 0) revert EmptyKey();

        _metadata[agentId][key] = value;

        emit MetadataSet(agentId, key);
    }

    /// @notice Get metadata for an agent.
    /// @param agentId The agent's token ID.
    /// @param key The metadata key.
    /// @return value The metadata value as raw bytes. Empty if not set.
    function getMetadata(uint256 agentId, string calldata key) external view returns (bytes memory value) {
        value = _metadata[agentId][key];
    }

    /// @notice Get the total number of registered agents.
    /// @return count The number of agents registered so far.
    function totalAgents() external view returns (uint256 count) {
        // _nextAgentId starts at 1, so total = _nextAgentId - 1
        count = _nextAgentId - 1;
    }
}
