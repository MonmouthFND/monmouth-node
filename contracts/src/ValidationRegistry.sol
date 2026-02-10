// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "@openzeppelin/contracts/token/ERC721/IERC721.sol";

/// @title ValidationRegistry - ERC-8004 Agent Validation
/// @author Monmouth Team
/// @notice Independent validation system for agent capabilities on Monmouth.
/// @dev Enables validators to independently verify agent capabilities through
///      a request-response pattern. Validation requests are indexed by their
///      content hash, and responses include tags for categorization.
///      Security: custom errors, events on all state changes,
///      input validation, agent existence verification via IdentityRegistry.
contract ValidationRegistry {
    /// @notice Reference to the IdentityRegistry for agent existence checks.
    IERC721 public immutable identityRegistry;
    /// @notice Response codes for validation results.
    /// @dev 0 = Pending, 1 = Approved, 2 = Rejected, 3 = Inconclusive
    uint8 public constant RESPONSE_PENDING = 0;
    uint8 public constant RESPONSE_APPROVED = 1;
    uint8 public constant RESPONSE_REJECTED = 2;
    uint8 public constant RESPONSE_INCONCLUSIVE = 3;

    /// @notice A validation request record.
    struct ValidationRequest {
        /// @notice The agent being validated.
        uint256 agentId;
        /// @notice The address requesting validation.
        address requester;
        /// @notice The designated validator address.
        address validator;
        /// @notice URI pointing to detailed request data.
        string requestURI;
        /// @notice Hash of the request content for integrity.
        bytes32 requestHash;
        /// @notice Block timestamp of the request.
        uint256 timestamp;
        /// @notice Whether a response has been submitted.
        bool responded;
    }

    /// @notice A validation response record.
    struct ValidationResponse {
        /// @notice The response code (APPROVED/REJECTED/INCONCLUSIVE).
        uint8 response;
        /// @notice URI pointing to detailed response data.
        string responseURI;
        /// @notice Hash of the response content for integrity.
        bytes32 responseHash;
        /// @notice Categorization tag for the validation type.
        string tag;
        /// @notice Block timestamp of the response.
        uint256 timestamp;
    }

    /// @notice Mapping from request hash to validation request.
    mapping(bytes32 => ValidationRequest) private _requests;

    /// @notice Mapping from request hash to validation response.
    mapping(bytes32 => ValidationResponse) private _responses;

    /// @notice Validation request hashes associated with each agent.
    mapping(uint256 => bytes32[]) private _agentRequests;

    /// @notice Minimum seconds between validation requests per sender.
    uint256 public requestCooldown;

    /// @notice Timestamp of last validation request per sender.
    mapping(address => uint256) private _lastRequestTime;

    /// @notice Emitted when a validation request is created.
    event ValidationRequested(
        bytes32 indexed requestHash,
        uint256 indexed agentId,
        address indexed validator,
        address requester,
        string requestURI
    );

    /// @notice Emitted when a validator responds to a request.
    event ValidationResponded(
        bytes32 indexed requestHash,
        uint256 indexed agentId,
        address indexed validator,
        uint8 response,
        string tag
    );

    /// @notice Thrown when a request hash already exists.
    error RequestAlreadyExists(bytes32 requestHash);

    /// @notice Thrown when referencing a request that doesn't exist.
    error RequestNotFound(bytes32 requestHash);

    /// @notice Thrown when caller is not the designated validator.
    error NotDesignatedValidator(bytes32 requestHash, address caller);

    /// @notice Thrown when a request has already been responded to.
    error AlreadyResponded(bytes32 requestHash);

    /// @notice Thrown when the validator address is zero.
    error ZeroValidator();

    /// @notice Thrown when an invalid response code is provided.
    error InvalidResponseCode(uint8 response);

    /// @notice Thrown when a request is submitted too frequently.
    error CooldownNotElapsed(address sender, uint256 remainingSeconds);

    /// @notice Thrown when the referenced agent does not exist.
    error AgentNotFound(uint256 agentId);

    /// @param _identityRegistry Address of the IdentityRegistry contract.
    /// @param _requestCooldown Minimum seconds between validation requests per sender. 0 = no cooldown.
    constructor(address _identityRegistry, uint256 _requestCooldown) {
        identityRegistry = IERC721(_identityRegistry);
        requestCooldown = _requestCooldown;
    }

    /// @notice Submit a validation request for an agent.
    /// @dev The requestHash serves as the unique identifier for the request.
    ///      It must not have been used before.
    ///      IMPORTANT: To prevent front-running, callers SHOULD include msg.sender
    ///      in the requestHash preimage, e.g.:
    ///        requestHash = keccak256(abi.encode(msg.sender, agentId, nonce))
    ///      Without this, an attacker could observe a pending transaction and submit
    ///      the same requestHash first, causing the original transaction to revert.
    /// @param validator The address designated to respond to this request.
    /// @param agentId The agent to be validated.
    /// @param requestURI URI pointing to detailed request data.
    /// @param requestHash Unique hash identifying this validation request.
    function validationRequest(
        address validator,
        uint256 agentId,
        string calldata requestURI,
        bytes32 requestHash
    ) external {
        if (validator == address(0)) revert ZeroValidator();
        if (_requests[requestHash].timestamp != 0) revert RequestAlreadyExists(requestHash);

        // Verify agent exists in IdentityRegistry
        try identityRegistry.ownerOf(agentId) returns (address) {} catch {
            revert AgentNotFound(agentId);
        }

        if (requestCooldown > 0) {
            uint256 elapsed = block.timestamp - _lastRequestTime[msg.sender];
            if (_lastRequestTime[msg.sender] != 0 && elapsed < requestCooldown) {
                revert CooldownNotElapsed(msg.sender, requestCooldown - elapsed);
            }
            _lastRequestTime[msg.sender] = block.timestamp;
        }

        _requests[requestHash] = ValidationRequest({
            agentId: agentId,
            requester: msg.sender,
            validator: validator,
            requestURI: requestURI,
            requestHash: requestHash,
            timestamp: block.timestamp,
            responded: false
        });

        _agentRequests[agentId].push(requestHash);

        emit ValidationRequested(requestHash, agentId, validator, msg.sender, requestURI);
    }

    /// @notice Submit a validation response.
    /// @dev Only the designated validator can respond, and only once per request.
    /// @param requestHash The hash of the request being responded to.
    /// @param response The response code (1=Approved, 2=Rejected, 3=Inconclusive).
    /// @param responseURI URI pointing to detailed response data.
    /// @param responseHash Hash of the response content for integrity.
    /// @param tag Categorization tag for the validation type.
    function validationResponse(
        bytes32 requestHash,
        uint8 response,
        string calldata responseURI,
        bytes32 responseHash,
        string calldata tag
    ) external {
        ValidationRequest storage req = _requests[requestHash];
        if (req.timestamp == 0) revert RequestNotFound(requestHash);
        if (req.validator != msg.sender) revert NotDesignatedValidator(requestHash, msg.sender);
        if (req.responded) revert AlreadyResponded(requestHash);
        if (response < RESPONSE_APPROVED || response > RESPONSE_INCONCLUSIVE) {
            revert InvalidResponseCode(response);
        }

        req.responded = true;

        _responses[requestHash] = ValidationResponse({
            response: response,
            responseURI: responseURI,
            responseHash: responseHash,
            tag: tag,
            timestamp: block.timestamp
        });

        emit ValidationResponded(requestHash, req.agentId, msg.sender, response, tag);
    }

    /// @notice Get a validation request by its hash.
    /// @param requestHash The request hash.
    /// @return agentId The agent being validated.
    /// @return requester The address that created the request.
    /// @return validator The designated validator.
    /// @return responded Whether a response has been submitted.
    function getRequest(bytes32 requestHash)
        external
        view
        returns (uint256 agentId, address requester, address validator, bool responded)
    {
        ValidationRequest storage req = _requests[requestHash];
        if (req.timestamp == 0) revert RequestNotFound(requestHash);
        return (req.agentId, req.requester, req.validator, req.responded);
    }

    /// @notice Get a validation response by request hash.
    /// @param requestHash The request hash.
    /// @return response The response code.
    /// @return responseURI The response data URI.
    /// @return responseHash The response content hash.
    /// @return tag The categorization tag.
    function getResponse(bytes32 requestHash)
        external
        view
        returns (uint8 response, string memory responseURI, bytes32 responseHash, string memory tag)
    {
        ValidationRequest storage req = _requests[requestHash];
        if (req.timestamp == 0) revert RequestNotFound(requestHash);
        if (!req.responded) revert RequestNotFound(requestHash);

        ValidationResponse storage resp = _responses[requestHash];
        return (resp.response, resp.responseURI, resp.responseHash, resp.tag);
    }

    /// @notice Get the number of validation requests for an agent.
    /// @param agentId The agent to query.
    /// @return count The number of validation requests.
    function getAgentRequestCount(uint256 agentId) external view returns (uint256 count) {
        count = _agentRequests[agentId].length;
    }

    /// @notice Get a specific validation request hash for an agent by index.
    /// @param agentId The agent to query.
    /// @param index The index in the agent's request list.
    /// @return requestHash The request hash at that index.
    function getAgentRequestAt(uint256 agentId, uint256 index) external view returns (bytes32 requestHash) {
        requestHash = _agentRequests[agentId][index];
    }
}
