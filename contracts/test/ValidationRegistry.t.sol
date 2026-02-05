// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "forge-std/Test.sol";
import "../src/ValidationRegistry.sol";

contract ValidationRegistryTest is Test {
    ValidationRegistry public registry;
    address public requester = address(0xA11CE);
    address public validator = address(0xB0B);
    address public stranger = address(0xC0C);

    bytes32 public constant REQ_HASH = keccak256("request-1");
    bytes32 public constant RESP_HASH = keccak256("response-1");

    event ValidationRequested(
        bytes32 indexed requestHash,
        uint256 indexed agentId,
        address indexed validator,
        address requester,
        string requestURI
    );
    event ValidationResponded(
        bytes32 indexed requestHash,
        uint256 indexed agentId,
        address indexed validator,
        uint8 response,
        string tag
    );

    function setUp() public {
        registry = new ValidationRegistry();
    }

    // --- validationRequest ---

    function test_validationRequest_success() public {
        vm.prank(requester);
        registry.validationRequest(validator, 1, "ipfs://req1", REQ_HASH);

        (uint256 agentId, address req, address val, bool responded) = registry.getRequest(REQ_HASH);
        assertEq(agentId, 1);
        assertEq(req, requester);
        assertEq(val, validator);
        assertFalse(responded);
    }

    function test_validationRequest_emits_event() public {
        vm.prank(requester);
        vm.expectEmit(true, true, true, true);
        emit ValidationRequested(REQ_HASH, 1, validator, requester, "ipfs://req1");
        registry.validationRequest(validator, 1, "ipfs://req1", REQ_HASH);
    }

    function test_validationRequest_reverts_zero_validator() public {
        vm.prank(requester);
        vm.expectRevert(ValidationRegistry.ZeroValidator.selector);
        registry.validationRequest(address(0), 1, "ipfs://req1", REQ_HASH);
    }

    function test_validationRequest_reverts_duplicate_hash() public {
        vm.startPrank(requester);
        registry.validationRequest(validator, 1, "ipfs://req1", REQ_HASH);

        vm.expectRevert(abi.encodeWithSelector(ValidationRegistry.RequestAlreadyExists.selector, REQ_HASH));
        registry.validationRequest(validator, 2, "ipfs://req2", REQ_HASH);
        vm.stopPrank();
    }

    function test_validationRequest_tracks_agent_requests() public {
        bytes32 hash1 = keccak256("req-1");
        bytes32 hash2 = keccak256("req-2");

        vm.startPrank(requester);
        registry.validationRequest(validator, 42, "uri1", hash1);
        registry.validationRequest(validator, 42, "uri2", hash2);
        vm.stopPrank();

        assertEq(registry.getAgentRequestCount(42), 2);
        assertEq(registry.getAgentRequestAt(42, 0), hash1);
        assertEq(registry.getAgentRequestAt(42, 1), hash2);
    }

    // --- validationResponse ---

    function test_validationResponse_approved() public {
        vm.prank(requester);
        registry.validationRequest(validator, 1, "ipfs://req", REQ_HASH);

        vm.prank(validator);
        registry.validationResponse(REQ_HASH, 1, "ipfs://resp", RESP_HASH, "capability");

        (uint8 response, string memory responseURI, bytes32 responseHash, string memory tag) =
            registry.getResponse(REQ_HASH);
        assertEq(response, 1); // APPROVED
        assertEq(responseURI, "ipfs://resp");
        assertEq(responseHash, RESP_HASH);
        assertEq(tag, "capability");
    }

    function test_validationResponse_rejected() public {
        vm.prank(requester);
        registry.validationRequest(validator, 1, "ipfs://req", REQ_HASH);

        vm.prank(validator);
        registry.validationResponse(REQ_HASH, 2, "ipfs://resp", RESP_HASH, "capability");

        (uint8 response,,,) = registry.getResponse(REQ_HASH);
        assertEq(response, 2); // REJECTED
    }

    function test_validationResponse_inconclusive() public {
        vm.prank(requester);
        registry.validationRequest(validator, 1, "ipfs://req", REQ_HASH);

        vm.prank(validator);
        registry.validationResponse(REQ_HASH, 3, "ipfs://resp", RESP_HASH, "capability");

        (uint8 response,,,) = registry.getResponse(REQ_HASH);
        assertEq(response, 3); // INCONCLUSIVE
    }

    function test_validationResponse_emits_event() public {
        vm.prank(requester);
        registry.validationRequest(validator, 1, "ipfs://req", REQ_HASH);

        vm.prank(validator);
        vm.expectEmit(true, true, true, true);
        emit ValidationResponded(REQ_HASH, 1, validator, 1, "capability");
        registry.validationResponse(REQ_HASH, 1, "ipfs://resp", RESP_HASH, "capability");
    }

    function test_validationResponse_marks_responded() public {
        vm.prank(requester);
        registry.validationRequest(validator, 1, "ipfs://req", REQ_HASH);

        vm.prank(validator);
        registry.validationResponse(REQ_HASH, 1, "ipfs://resp", RESP_HASH, "tag");

        (,,, bool responded) = registry.getRequest(REQ_HASH);
        assertTrue(responded);
    }

    function test_validationResponse_reverts_request_not_found() public {
        bytes32 fakeHash = keccak256("fake");
        vm.prank(validator);
        vm.expectRevert(abi.encodeWithSelector(ValidationRegistry.RequestNotFound.selector, fakeHash));
        registry.validationResponse(fakeHash, 1, "uri", RESP_HASH, "tag");
    }

    function test_validationResponse_reverts_not_designated_validator() public {
        vm.prank(requester);
        registry.validationRequest(validator, 1, "ipfs://req", REQ_HASH);

        vm.prank(stranger);
        vm.expectRevert(
            abi.encodeWithSelector(ValidationRegistry.NotDesignatedValidator.selector, REQ_HASH, stranger)
        );
        registry.validationResponse(REQ_HASH, 1, "uri", RESP_HASH, "tag");
    }

    function test_validationResponse_reverts_already_responded() public {
        vm.prank(requester);
        registry.validationRequest(validator, 1, "ipfs://req", REQ_HASH);

        vm.startPrank(validator);
        registry.validationResponse(REQ_HASH, 1, "uri", RESP_HASH, "tag");

        vm.expectRevert(abi.encodeWithSelector(ValidationRegistry.AlreadyResponded.selector, REQ_HASH));
        registry.validationResponse(REQ_HASH, 2, "uri2", keccak256("resp2"), "tag2");
        vm.stopPrank();
    }

    function test_validationResponse_reverts_invalid_response_zero() public {
        vm.prank(requester);
        registry.validationRequest(validator, 1, "ipfs://req", REQ_HASH);

        vm.prank(validator);
        vm.expectRevert(abi.encodeWithSelector(ValidationRegistry.InvalidResponseCode.selector, 0));
        registry.validationResponse(REQ_HASH, 0, "uri", RESP_HASH, "tag");
    }

    function test_validationResponse_reverts_invalid_response_four() public {
        vm.prank(requester);
        registry.validationRequest(validator, 1, "ipfs://req", REQ_HASH);

        vm.prank(validator);
        vm.expectRevert(abi.encodeWithSelector(ValidationRegistry.InvalidResponseCode.selector, 4));
        registry.validationResponse(REQ_HASH, 4, "uri", RESP_HASH, "tag");
    }

    // --- getRequest ---

    function test_getRequest_reverts_not_found() public {
        vm.expectRevert(abi.encodeWithSelector(ValidationRegistry.RequestNotFound.selector, REQ_HASH));
        registry.getRequest(REQ_HASH);
    }

    // --- getResponse ---

    function test_getResponse_reverts_no_request() public {
        vm.expectRevert(abi.encodeWithSelector(ValidationRegistry.RequestNotFound.selector, REQ_HASH));
        registry.getResponse(REQ_HASH);
    }

    function test_getResponse_reverts_not_responded() public {
        vm.prank(requester);
        registry.validationRequest(validator, 1, "ipfs://req", REQ_HASH);

        vm.expectRevert(abi.encodeWithSelector(ValidationRegistry.RequestNotFound.selector, REQ_HASH));
        registry.getResponse(REQ_HASH);
    }

    // --- getAgentRequestCount ---

    function test_getAgentRequestCount_zero_for_unknown() public view {
        assertEq(registry.getAgentRequestCount(999), 0);
    }

    // --- Response code constants ---

    function test_response_constants() public view {
        assertEq(registry.RESPONSE_PENDING(), 0);
        assertEq(registry.RESPONSE_APPROVED(), 1);
        assertEq(registry.RESPONSE_REJECTED(), 2);
        assertEq(registry.RESPONSE_INCONCLUSIVE(), 3);
    }
}
