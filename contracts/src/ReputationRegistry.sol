// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "@openzeppelin/contracts/utils/ReentrancyGuard.sol";

/// @title ReputationRegistry - ERC-8004 Agent Reputation
/// @author Monmouth Team
/// @notice On-chain feedback system for agent reputation on the Monmouth network.
/// @dev Implements the ERC-8004 reputation specification with:
///      - Signed feedback with tags for categorization
///      - Aggregated summaries per agent
///      - Revocation support for feedback providers
///      Security: ReentrancyGuard, checks-effects-interactions,
///      events on all state changes, custom errors for gas efficiency.
contract ReputationRegistry is ReentrancyGuard {
    /// @notice Auto-incrementing feedback ID counter.
    uint256 private _nextFeedbackId;

    /// @notice Individual feedback record.
    struct Feedback {
        /// @notice The agent receiving feedback.
        uint256 agentId;
        /// @notice The address providing feedback.
        address client;
        /// @notice Signed feedback value (can be positive or negative).
        int128 value;
        /// @notice Decimal precision of the value.
        uint8 decimals;
        /// @notice Primary categorization tag.
        string tag1;
        /// @notice Secondary categorization tag.
        string tag2;
        /// @notice The endpoint or service being reviewed.
        string endpoint;
        /// @notice URI pointing to detailed feedback data.
        string feedbackURI;
        /// @notice Hash of the feedback content for integrity verification.
        bytes32 feedbackHash;
        /// @notice Block timestamp when feedback was given.
        uint256 timestamp;
        /// @notice Whether this feedback has been revoked.
        bool revoked;
    }

    /// @notice All feedback records by ID.
    mapping(uint256 => Feedback) private _feedbacks;

    /// @notice Feedback IDs associated with each agent.
    mapping(uint256 => uint256[]) private _agentFeedbackIds;

    /// @notice Emitted when new feedback is submitted.
    event FeedbackGiven(
        uint256 indexed feedbackId,
        uint256 indexed agentId,
        address indexed client,
        int128 value,
        uint8 decimals,
        string tag1,
        string tag2
    );

    /// @notice Emitted when feedback is revoked.
    event FeedbackRevoked(uint256 indexed feedbackId, uint256 indexed agentId, address indexed client);

    /// @notice Thrown when a non-owner tries to revoke feedback.
    error NotFeedbackOwner(uint256 feedbackId, address caller);

    /// @notice Thrown when trying to revoke already-revoked feedback.
    error AlreadyRevoked(uint256 feedbackId);

    /// @notice Thrown when referencing a feedback ID that doesn't exist.
    error FeedbackNotFound(uint256 feedbackId);

    constructor() {
        _nextFeedbackId = 1;
    }

    /// @notice Submit feedback for an agent.
    /// @param agentId The agent's identity token ID from IdentityRegistry.
    /// @param value The feedback score. Positive = good, negative = bad.
    /// @param decimals The decimal precision of value (e.g., 2 means value is scaled by 100).
    /// @param tag1 Primary category tag (e.g., "reliability", "speed").
    /// @param tag2 Secondary category tag (e.g., "api", "inference").
    /// @param endpoint The specific service endpoint being reviewed.
    /// @param feedbackURI URI pointing to detailed feedback JSON.
    /// @param feedbackHash Keccak256 hash of the feedback content for integrity.
    function giveFeedback(
        uint256 agentId,
        int128 value,
        uint8 decimals,
        string calldata tag1,
        string calldata tag2,
        string calldata endpoint,
        string calldata feedbackURI,
        bytes32 feedbackHash
    ) external nonReentrant {
        uint256 feedbackId = _nextFeedbackId;
        unchecked {
            ++_nextFeedbackId;
        }

        _feedbacks[feedbackId] = Feedback({
            agentId: agentId,
            client: msg.sender,
            value: value,
            decimals: decimals,
            tag1: tag1,
            tag2: tag2,
            endpoint: endpoint,
            feedbackURI: feedbackURI,
            feedbackHash: feedbackHash,
            timestamp: block.timestamp,
            revoked: false
        });

        _agentFeedbackIds[agentId].push(feedbackId);

        emit FeedbackGiven(feedbackId, agentId, msg.sender, value, decimals, tag1, tag2);
    }

    /// @notice Revoke previously submitted feedback.
    /// @dev Only the original feedback provider can revoke. Revoked feedback
    ///      is excluded from summary calculations.
    /// @param feedbackId The ID of the feedback to revoke.
    function revokeFeedback(uint256 feedbackId) external {
        if (feedbackId == 0 || feedbackId >= _nextFeedbackId) revert FeedbackNotFound(feedbackId);

        Feedback storage fb = _feedbacks[feedbackId];
        if (fb.client != msg.sender) revert NotFeedbackOwner(feedbackId, msg.sender);
        if (fb.revoked) revert AlreadyRevoked(feedbackId);

        fb.revoked = true;

        emit FeedbackRevoked(feedbackId, fb.agentId, msg.sender);
    }

    /// @notice Get an aggregated reputation summary for an agent.
    /// @dev Filters by client addresses and tags. Pass empty arrays/strings
    ///      to skip filtering for that dimension.
    /// @param agentId The agent to summarize.
    /// @param clients Filter to only these client addresses. Empty = all clients.
    /// @param tag1 Filter by primary tag. Empty string = all tags.
    /// @param tag2 Filter by secondary tag. Empty string = all tags.
    /// @return count Number of matching non-revoked feedback entries.
    /// @return summaryValue Sum of all matching feedback values.
    /// @return decimals The decimal precision (uses the first matching entry's decimals).
    function getSummary(
        uint256 agentId,
        address[] calldata clients,
        string calldata tag1,
        string calldata tag2
    ) external view returns (uint256 count, int256 summaryValue, uint8 decimals) {
        uint256[] storage ids = _agentFeedbackIds[agentId];
        bool decimalsSet = false;

        for (uint256 i = 0; i < ids.length; i++) {
            Feedback storage fb = _feedbacks[ids[i]];

            if (fb.revoked) continue;
            if (!_matchesFilters(fb, clients, tag1, tag2)) continue;

            if (!decimalsSet) {
                decimals = fb.decimals;
                decimalsSet = true;
            }

            summaryValue += int256(fb.value);
            unchecked {
                ++count;
            }
        }
    }

    /// @notice Get a specific feedback record.
    /// @param feedbackId The feedback ID to look up.
    /// @return agentId The agent that received the feedback.
    /// @return client The address that submitted the feedback.
    /// @return value The feedback score.
    /// @return decimals The decimal precision.
    /// @return revoked Whether the feedback has been revoked.
    function getFeedback(uint256 feedbackId)
        external
        view
        returns (uint256 agentId, address client, int128 value, uint8 decimals, bool revoked)
    {
        if (feedbackId == 0 || feedbackId >= _nextFeedbackId) revert FeedbackNotFound(feedbackId);
        Feedback storage fb = _feedbacks[feedbackId];
        return (fb.agentId, fb.client, fb.value, fb.decimals, fb.revoked);
    }

    /// @notice Get the total number of feedback entries submitted.
    /// @return count The total feedback count (including revoked).
    function totalFeedback() external view returns (uint256 count) {
        count = _nextFeedbackId - 1;
    }

    /// @dev Check if a feedback entry matches the given filters.
    function _matchesFilters(
        Feedback storage fb,
        address[] calldata clients,
        string calldata tag1,
        string calldata tag2
    ) private view returns (bool) {
        if (clients.length > 0 && !_containsAddress(clients, fb.client)) return false;
        if (bytes(tag1).length > 0 && keccak256(bytes(fb.tag1)) != keccak256(bytes(tag1))) return false;
        if (bytes(tag2).length > 0 && keccak256(bytes(fb.tag2)) != keccak256(bytes(tag2))) return false;
        return true;
    }

    /// @dev Check if an address is in a list.
    function _containsAddress(address[] calldata list, address target) private pure returns (bool) {
        for (uint256 i = 0; i < list.length; i++) {
            if (list[i] == target) return true;
        }
        return false;
    }
}
