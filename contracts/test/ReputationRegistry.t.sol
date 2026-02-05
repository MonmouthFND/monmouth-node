// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "forge-std/Test.sol";
import "../src/ReputationRegistry.sol";

contract ReputationRegistryTest is Test {
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
        registry = new ReputationRegistry();
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
        vm.prank(alice);
        vm.expectEmit(true, true, true, true);
        emit FeedbackGiven(1, 42, alice, 100, 2, "speed", "inference");
        registry.giveFeedback(42, 100, 2, "speed", "inference", "/infer", "ipfs://fb", keccak256("fb"));
    }

    function test_giveFeedback_negative_value() public {
        vm.prank(alice);
        registry.giveFeedback(1, -50, 2, "reliability", "api", "/v1", "uri", keccak256("neg"));

        (,, int128 value,,) = registry.getFeedback(1);
        assertEq(value, -50);
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
        (uint256 count, int256 sum, uint8 decimals) = registry.getSummary(1, noClients, "", "");
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
        (uint256 count, int256 sum,) = registry.getSummary(1, noClients, "", "");
        assertEq(count, 1);
        assertEq(sum, 50);
    }

    function test_getSummary_filter_by_tag1() public {
        vm.startPrank(alice);
        registry.giveFeedback(1, 100, 2, "speed", "api", "", "", bytes32(0));
        registry.giveFeedback(1, 200, 2, "reliability", "api", "", "", bytes32(0));
        vm.stopPrank();

        address[] memory noClients = new address[](0);
        (uint256 count, int256 sum,) = registry.getSummary(1, noClients, "speed", "");
        assertEq(count, 1);
        assertEq(sum, 100);
    }

    function test_getSummary_filter_by_tag2() public {
        vm.startPrank(alice);
        registry.giveFeedback(1, 100, 2, "speed", "api", "", "", bytes32(0));
        registry.giveFeedback(1, 200, 2, "speed", "inference", "", "", bytes32(0));
        vm.stopPrank();

        address[] memory noClients = new address[](0);
        (uint256 count, int256 sum,) = registry.getSummary(1, noClients, "", "inference");
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
        (uint256 count, int256 sum,) = registry.getSummary(1, clients, "", "");
        assertEq(count, 1);
        assertEq(sum, 100);
    }

    function test_getSummary_negative_values() public {
        vm.startPrank(alice);
        registry.giveFeedback(1, 100, 2, "", "", "", "", bytes32(0));
        registry.giveFeedback(1, -150, 2, "", "", "", "", bytes32(0));
        vm.stopPrank();

        address[] memory noClients = new address[](0);
        (uint256 count, int256 sum,) = registry.getSummary(1, noClients, "", "");
        assertEq(count, 2);
        assertEq(sum, -50);
    }

    function test_getSummary_empty_for_nonexistent_agent() public view {
        address[] memory noClients = new address[](0);
        (uint256 count, int256 sum, uint8 decimals) = registry.getSummary(999, noClients, "", "");
        assertEq(count, 0);
        assertEq(sum, 0);
        assertEq(decimals, 0);
    }
}
