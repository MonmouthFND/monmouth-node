// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "forge-std/Test.sol";
import "../src/IdentityRegistry.sol";

contract IdentityRegistryTest is Test {
    IdentityRegistry public registry;
    address public alice = address(0xA11CE);
    address public bob = address(0xB0B);

    event AgentRegistered(uint256 indexed agentId, address indexed owner, string agentURI);
    event AgentWalletSet(uint256 indexed agentId, address indexed wallet);
    event MetadataSet(uint256 indexed agentId, string key);

    function setUp() public {
        registry = new IdentityRegistry();
    }

    // --- register ---

    function test_register_success() public {
        vm.prank(alice);
        uint256 agentId = registry.register("ipfs://agent1");
        assertEq(agentId, 1);
        assertEq(registry.ownerOf(1), alice);
        assertEq(registry.tokenURI(1), "ipfs://agent1");
    }

    function test_register_incrementing_ids() public {
        vm.startPrank(alice);
        uint256 id1 = registry.register("ipfs://agent1");
        uint256 id2 = registry.register("ipfs://agent2");
        vm.stopPrank();

        assertEq(id1, 1);
        assertEq(id2, 2);
    }

    function test_register_emits_event() public {
        vm.prank(alice);
        vm.expectEmit(true, true, false, true);
        emit AgentRegistered(1, alice, "ipfs://agent1");
        registry.register("ipfs://agent1");
    }

    function test_register_reverts_empty_uri() public {
        vm.prank(alice);
        vm.expectRevert(IdentityRegistry.EmptyURI.selector);
        registry.register("");
    }

    function test_register_multiple_owners() public {
        vm.prank(alice);
        uint256 aliceAgent = registry.register("ipfs://alice");
        vm.prank(bob);
        uint256 bobAgent = registry.register("ipfs://bob");

        assertEq(registry.ownerOf(aliceAgent), alice);
        assertEq(registry.ownerOf(bobAgent), bob);
    }

    // --- totalAgents ---

    function test_totalAgents_initial() public view {
        assertEq(registry.totalAgents(), 0);
    }

    function test_totalAgents_after_registers() public {
        vm.startPrank(alice);
        registry.register("ipfs://1");
        registry.register("ipfs://2");
        registry.register("ipfs://3");
        vm.stopPrank();

        assertEq(registry.totalAgents(), 3);
    }

    // --- setAgentWallet ---

    function test_setAgentWallet_success() public {
        vm.prank(alice);
        uint256 agentId = registry.register("ipfs://agent");

        vm.prank(alice);
        registry.setAgentWallet(agentId, bob);

        assertEq(registry.getAgentWallet(agentId), bob);
    }

    function test_setAgentWallet_emits_event() public {
        vm.prank(alice);
        uint256 agentId = registry.register("ipfs://agent");

        vm.prank(alice);
        vm.expectEmit(true, true, false, false);
        emit AgentWalletSet(agentId, bob);
        registry.setAgentWallet(agentId, bob);
    }

    function test_setAgentWallet_reverts_not_owner() public {
        vm.prank(alice);
        uint256 agentId = registry.register("ipfs://agent");

        vm.prank(bob);
        vm.expectRevert(abi.encodeWithSelector(IdentityRegistry.NotAgentOwner.selector, agentId, bob));
        registry.setAgentWallet(agentId, bob);
    }

    function test_setAgentWallet_zero_address() public {
        vm.prank(alice);
        uint256 agentId = registry.register("ipfs://agent");

        vm.prank(alice);
        registry.setAgentWallet(agentId, address(0));
        assertEq(registry.getAgentWallet(agentId), address(0));
    }

    // --- getAgentWallet ---

    function test_getAgentWallet_default_zero() public {
        vm.prank(alice);
        uint256 agentId = registry.register("ipfs://agent");
        assertEq(registry.getAgentWallet(agentId), address(0));
    }

    // --- setMetadata ---

    function test_setMetadata_success() public {
        vm.prank(alice);
        uint256 agentId = registry.register("ipfs://agent");

        vm.prank(alice);
        registry.setMetadata(agentId, "version", abi.encode(uint256(1)));

        bytes memory val = registry.getMetadata(agentId, "version");
        assertEq(abi.decode(val, (uint256)), 1);
    }

    function test_setMetadata_emits_event() public {
        vm.prank(alice);
        uint256 agentId = registry.register("ipfs://agent");

        vm.prank(alice);
        vm.expectEmit(true, false, false, true);
        emit MetadataSet(agentId, "key1");
        registry.setMetadata(agentId, "key1", "value1");
    }

    function test_setMetadata_reverts_not_owner() public {
        vm.prank(alice);
        uint256 agentId = registry.register("ipfs://agent");

        vm.prank(bob);
        vm.expectRevert(abi.encodeWithSelector(IdentityRegistry.NotAgentOwner.selector, agentId, bob));
        registry.setMetadata(agentId, "key", "value");
    }

    function test_setMetadata_reverts_empty_key() public {
        vm.prank(alice);
        uint256 agentId = registry.register("ipfs://agent");

        vm.prank(alice);
        vm.expectRevert(IdentityRegistry.EmptyKey.selector);
        registry.setMetadata(agentId, "", "value");
    }

    function test_setMetadata_overwrite() public {
        vm.prank(alice);
        uint256 agentId = registry.register("ipfs://agent");

        vm.startPrank(alice);
        registry.setMetadata(agentId, "key", "v1");
        registry.setMetadata(agentId, "key", "v2");
        vm.stopPrank();

        assertEq(string(registry.getMetadata(agentId, "key")), string("v2"));
    }

    // --- getMetadata ---

    function test_getMetadata_empty_for_unset_key() public {
        vm.prank(alice);
        uint256 agentId = registry.register("ipfs://agent");
        bytes memory val = registry.getMetadata(agentId, "nonexistent");
        assertEq(val.length, 0);
    }

    // --- ERC721 properties ---

    function test_name_and_symbol() public view {
        assertEq(registry.name(), "Monmouth Agent Identity");
        assertEq(registry.symbol(), "MAID");
    }

    function test_transfer_agent() public {
        vm.prank(alice);
        uint256 agentId = registry.register("ipfs://agent");

        vm.prank(alice);
        registry.transferFrom(alice, bob, agentId);

        assertEq(registry.ownerOf(agentId), bob);
    }

    function test_new_owner_can_set_wallet_after_transfer() public {
        vm.prank(alice);
        uint256 agentId = registry.register("ipfs://agent");

        vm.prank(alice);
        registry.transferFrom(alice, bob, agentId);

        // Bob is now the owner
        vm.prank(bob);
        registry.setAgentWallet(agentId, address(0xDEAD));
        assertEq(registry.getAgentWallet(agentId), address(0xDEAD));

        // Alice can no longer set wallet
        vm.prank(alice);
        vm.expectRevert(abi.encodeWithSelector(IdentityRegistry.NotAgentOwner.selector, agentId, alice));
        registry.setAgentWallet(agentId, address(0));
    }

    // --- Fuzz tests ---

    /// @dev Fuzz: agentId counter is always monotonically increasing.
    function testFuzz_agentId_monotonic(string calldata uri1, string calldata uri2) public {
        vm.assume(bytes(uri1).length > 0 && bytes(uri2).length > 0);

        vm.prank(alice);
        uint256 id1 = registry.register(uri1);
        vm.prank(bob);
        uint256 id2 = registry.register(uri2);

        assertTrue(id2 > id1);
    }

    /// @dev Fuzz: only the NFT owner can set metadata.
    function testFuzz_metadata_owner_only(bytes calldata value) public {
        vm.prank(alice);
        uint256 agentId = registry.register("ipfs://agent");

        // Bob should always fail
        vm.prank(bob);
        vm.expectRevert(abi.encodeWithSelector(IdentityRegistry.NotAgentOwner.selector, agentId, bob));
        registry.setMetadata(agentId, "key", value);

        // Alice should always succeed
        vm.prank(alice);
        registry.setMetadata(agentId, "key", value);
        assertEq(registry.getMetadata(agentId, "key"), value);
    }
}
