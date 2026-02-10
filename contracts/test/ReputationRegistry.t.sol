// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "forge-std/Test.sol";
import "../src/IdentityRegistry.sol";
import "../src/ReputationRegistry.sol";

contract ReputationRegistryTest is Test {
    IdentityRegistry public identity;
    ReputationRegistry public registry;
    address public alice = address(0xA11CE);
    address public bob = address(0xB0B);
    address public charlie = address(0xC0C);

    event FeedbackGiven(
        uint256 indexed feedbackId,
        uint256 indexed agentId,
        address indexed client,
        int128 value,
        uint8 decimals,
        string tag1,
        string tag2
    );
    event FeedbackRevoked(uint256 indexed feedbackId, uint256 indexed agentId, address indexed client);

    function setUp() public {
        identity = new IdentityRegistry();
        registry = new ReputationRegistry(address(identity), 0); // no cooldown for tests

        // Register agents used in tests
        vm.startPrank(alice);
        identity.register("ipfs://agent1"); // agentId = 1
        identity.register("ipfs://agent2"); // agentId = 2
        vm.stopPrank();
        vm.prank(bob);
        identity.register("ipfs://agent42"); // agentId = 3
    }

    /// @dev Helper to register a specific agent and return its ID.
    function _registerAgent(address owner, string memory uri) internal returns (uint256) {
        vm.prank(owner);
        return identity.register(uri);
    }

    // --- giveFeedback ---

    function test_giveFeedback_success() public {
        vm.prank(alice);
        registry.giveFeedback(1, 100, 2, "reliability", "api", "/v1/chat", "ipfs://fb1", keccak256("fb1"));

        (uint256 agentId, address client, int128 value, uint8 decimals, bool revoked) = registry.getFeedback(1);
        assertEq(agentId, 1);
        assertEq(client, alice);
        assertEq(value, 100);
        assertEq(decimals, 2);
        assertFalse(revoked);
    }

    function test_giveFeedback_incrementing_ids() public {
        vm.startPrank(alice);
        registry.giveFeedback(1, 50, 2, "t1", "t2", "ep", "uri1", keccak256("1"));
        registry.giveFeedback(1, 75, 2, "t1", "t2", "ep", "uri2", keccak256("2"));
        vm.stopPrank();

        (,address c1,,,) = registry.getFeedback(1);
        (,address c2,,,) = registry.getFeedback(2);
        assertEq(c1, alice);
        assertEq(c2, alice);
    }

    function test_giveFeedback_emits_event() public {
        uint256 agent42 = _registerAgent(alice, "ipfs://agent42");
        vm.prank(alice);
        vm.expectEmit(true, true, true, true);
        emit FeedbackGiven(1, agent42, alice, 100, 2, "speed", "inference");
        registry.giveFeedback(agent42, 100, 2, "speed", "inference", "/infer", "ipfs://fb", keccak256("fb"));
    }

    function test_giveFeedback_negative_value() public {
        vm.prank(alice);
        registry.giveFeedback(1, -50, 2, "reliability", "api", "/v1", "uri", keccak256("neg"));

        (,, int128 value,,) = registry.getFeedback(1);
        assertEq(value, -50);
    }

    function test_giveFeedback_reverts_agent_not_found() public {
        vm.prank(alice);
        vm.expectRevert(abi.encodeWithSelector(ReputationRegistry.AgentNotFound.selector, 999));
        registry.giveFeedback(999, 100, 2, "", "", "", "", bytes32(0));
    }

    function test_giveFeedback_reverts_decimals_mismatch() public {
        vm.startPrank(alice);
        registry.giveFeedback(1, 100, 2, "", "", "", "", bytes32(0));
        vm.expectRevert(abi.encodeWithSelector(ReputationRegistry.DecimalsMismatch.selector, 1, 2, 4));
        registry.giveFeedback(1, 200, 4, "", "", "", "", bytes32(0));
        vm.stopPrank();
    }

    // --- totalFeedback ---

    function test_totalFeedback_initial() public view {
        assertEq(registry.totalFeedback(), 0);
    }

    function test_totalFeedback_after_submissions() public {
        vm.startPrank(alice);
        registry.giveFeedback(1, 10, 0, "", "", "", "", bytes32(0));
        registry.giveFeedback(2, 20, 0, "", "", "", "", bytes32(0));
        vm.stopPrank();

        assertEq(registry.totalFeedback(), 2);
    }

    // --- revokeFeedback ---

    function test_revokeFeedback_success() public {
        vm.prank(alice);
        registry.giveFeedback(1, 100, 2, "t1", "t2", "ep", "uri", keccak256("fb"));

        vm.prank(alice);
        registry.revokeFeedback(1);

        (,,,, bool revoked) = registry.getFeedback(1);
        assertTrue(revoked);
    }

    function test_revokeFeedback_emits_event() public {
        vm.prank(alice);
        registry.giveFeedback(1, 100, 2, "t1", "t2", "ep", "uri", keccak256("fb"));

        vm.prank(alice);
        vm.expectEmit(true, true, true, false);
        emit FeedbackRevoked(1, 1, alice);
        registry.revokeFeedback(1);
    }

    function test_revokeFeedback_reverts_not_owner() public {
        vm.prank(alice);
        registry.giveFeedback(1, 100, 2, "t1", "t2", "ep", "uri", keccak256("fb"));

        vm.prank(bob);
        vm.expectRevert(abi.encodeWithSelector(ReputationRegistry.NotFeedbackOwner.selector, 1, bob));
        registry.revokeFeedback(1);
    }

    function test_revokeFeedback_reverts_already_revoked() public {
        vm.prank(alice);
        registry.giveFeedback(1, 100, 2, "t1", "t2", "ep", "uri", keccak256("fb"));

        vm.startPrank(alice);
        registry.revokeFeedback(1);
        vm.expectRevert(abi.encodeWithSelector(ReputationRegistry.AlreadyRevoked.selector, 1));
        registry.revokeFeedback(1);
        vm.stopPrank();
    }

    function test_revokeFeedback_reverts_not_found_zero() public {
        vm.prank(alice);
        vm.expectRevert(abi.encodeWithSelector(ReputationRegistry.FeedbackNotFound.selector, 0));
        registry.revokeFeedback(0);
    }

    function test_revokeFeedback_reverts_not_found_nonexistent() public {
        vm.prank(alice);
        vm.expectRevert(abi.encodeWithSelector(ReputationRegistry.FeedbackNotFound.selector, 999));
        registry.revokeFeedback(999);
    }

    // --- getFeedback ---

    function test_getFeedback_reverts_not_found() public {
        vm.expectRevert(abi.encodeWithSelector(ReputationRegistry.FeedbackNotFound.selector, 1));
        registry.getFeedback(1);
    }

    // --- getSummary ---

    function test_getSummary_basic() public {
        vm.startPrank(alice);
        registry.giveFeedback(1, 100, 2, "speed", "api", "/v1", "uri1", keccak256("1"));
        registry.giveFeedback(1, 50, 2, "speed", "api", "/v1", "uri2", keccak256("2"));
        vm.stopPrank();

        address[] memory noClients = new address[](0);
        (uint256 count, int256 sum, uint8 decimals,) = registry.getSummary(1, noClients, "", "", 0, 0);
        assertEq(count, 2);
        assertEq(sum, 150);
        assertEq(decimals, 2);
    }

    function test_getSummary_excludes_revoked() public {
        vm.startPrank(alice);
        registry.giveFeedback(1, 100, 2, "", "", "", "", bytes32(0));
        registry.giveFeedback(1, 50, 2, "", "", "", "", bytes32(0));
        registry.revokeFeedback(1);
        vm.stopPrank();

        address[] memory noClients = new address[](0);
        (uint256 count, int256 sum,,) = registry.getSummary(1, noClients, "", "", 0, 0);
        assertEq(count, 1);
        assertEq(sum, 50);
    }

    function test_getSummary_filter_by_tag1() public {
        vm.startPrank(alice);
        registry.giveFeedback(1, 100, 2, "speed", "api", "", "", bytes32(0));
        registry.giveFeedback(1, 200, 2, "reliability", "api", "", "", bytes32(0));
        vm.stopPrank();

        address[] memory noClients = new address[](0);
        (uint256 count, int256 sum,,) = registry.getSummary(1, noClients, "speed", "", 0, 0);
        assertEq(count, 1);
        assertEq(sum, 100);
    }

    function test_getSummary_filter_by_tag2() public {
        vm.startPrank(alice);
        registry.giveFeedback(1, 100, 2, "speed", "api", "", "", bytes32(0));
        registry.giveFeedback(1, 200, 2, "speed", "inference", "", "", bytes32(0));
        vm.stopPrank();

        address[] memory noClients = new address[](0);
        (uint256 count, int256 sum,,) = registry.getSummary(1, noClients, "", "inference", 0, 0);
        assertEq(count, 1);
        assertEq(sum, 200);
    }

    function test_getSummary_filter_by_clients() public {
        vm.prank(alice);
        registry.giveFeedback(1, 100, 2, "", "", "", "", bytes32(0));
        vm.prank(bob);
        registry.giveFeedback(1, 200, 2, "", "", "", "", bytes32(0));

        address[] memory clients = new address[](1);
        clients[0] = alice;
        (uint256 count, int256 sum,,) = registry.getSummary(1, clients, "", "", 0, 0);
        assertEq(count, 1);
        assertEq(sum, 100);
    }

    function test_getSummary_negative_values() public {
        vm.startPrank(alice);
        registry.giveFeedback(1, 100, 2, "", "", "", "", bytes32(0));
        registry.giveFeedback(1, -150, 2, "", "", "", "", bytes32(0));
        vm.stopPrank();

        address[] memory noClients = new address[](0);
        (uint256 count, int256 sum,,) = registry.getSummary(1, noClients, "", "", 0, 0);
        assertEq(count, 2);
        assertEq(sum, -50);
    }

    function test_getSummary_pagination() public {
        vm.startPrank(alice);
        registry.giveFeedback(1, 10, 2, "", "", "", "", bytes32(0));
        registry.giveFeedback(1, 20, 2, "", "", "", "", bytes32(0));
        registry.giveFeedback(1, 30, 2, "", "", "", "", bytes32(0));
        registry.giveFeedback(1, 40, 2, "", "", "", "", bytes32(0));
        vm.stopPrank();

        address[] memory noClients = new address[](0);

        // Page 1: first 2 entries
        (uint256 c1, int256 s1,, uint256 next1) = registry.getSummary(1, noClients, "", "", 0, 2);
        assertEq(c1, 2);
        assertEq(s1, 30); // 10 + 20
        assertEq(next1, 2);

        // Page 2: next 2 entries
        (uint256 c2, int256 s2,, uint256 next2) = registry.getSummary(1, noClients, "", "", next1, 2);
        assertEq(c2, 2);
        assertEq(s2, 70); // 30 + 40
        assertEq(next2, 0); // no more

        // Full total matches
        assertEq(c1 + c2, 4);
        assertEq(s1 + s2, 100);
    }

    function test_getSummary_offset_beyond_length() public {
        vm.prank(alice);
        registry.giveFeedback(1, 100, 2, "", "", "", "", bytes32(0));

        address[] memory noClients = new address[](0);
        (uint256 count, int256 sum,, uint256 nextOffset) = registry.getSummary(1, noClients, "", "", 999, 0);
        assertEq(count, 0);
        assertEq(sum, 0);
        assertEq(nextOffset, 0);
    }

    function test_getSummary_empty_for_nonexistent_agent() public view {
        address[] memory noClients = new address[](0);
        (uint256 count, int256 sum, uint8 decimals,) = registry.getSummary(999, noClients, "", "", 0, 0);
        assertEq(count, 0);
        assertEq(sum, 0);
        assertEq(decimals, 0);
    }

    // --- Fuzz tests ---

    /// @dev Fuzz: feedback counter is always monotonically increasing.
    function testFuzz_feedbackId_monotonic(int128 value) public {
        vm.prank(alice);
        registry.giveFeedback(1, value, 2, "", "", "", "", bytes32(0));
        uint256 total1 = registry.totalFeedback();

        vm.prank(bob);
        registry.giveFeedback(1, value, 2, "", "", "", "", bytes32(0));
        uint256 total2 = registry.totalFeedback();

        assertTrue(total2 > total1);
    }

    /// @dev Fuzz: getSummary count never exceeds number of feedbacks given.
    function testFuzz_summary_count_bounded(uint8 numFeedbacks) public {
        uint8 n = numFeedbacks % 20; // cap at 20 for gas
        vm.startPrank(alice);
        for (uint8 i = 0; i < n; i++) {
            registry.giveFeedback(1, int128(int8(i)) + 1, 2, "", "", "", "", bytes32(0));
        }
        vm.stopPrank();

        address[] memory noClients = new address[](0);
        (uint256 count,,,) = registry.getSummary(1, noClients, "", "", 0, 0);
        assertTrue(count <= n);
    }

    /// @dev Fuzz: pagination always returns correct nextOffset.
    function testFuzz_pagination_nextOffset(uint8 numFeedbacks, uint8 pageSize) public {
        uint8 n = (numFeedbacks % 10) + 1; // 1-10 feedbacks
        uint8 ps = (pageSize % 5) + 1;      // 1-5 page size

        vm.startPrank(alice);
        for (uint8 i = 0; i < n; i++) {
            registry.giveFeedback(1, 10, 2, "", "", "", "", bytes32(0));
        }
        vm.stopPrank();

        address[] memory noClients = new address[](0);
        uint256 totalCount = 0;
        uint256 offset = 0;

        // Paginate through all results
        for (uint256 page = 0; page < 20; page++) {
            (uint256 count,,, uint256 next) = registry.getSummary(1, noClients, "", "", offset, ps);
            totalCount += count;
            if (next == 0) break;
            offset = next;
        }

        assertEq(totalCount, n);
    }
}
