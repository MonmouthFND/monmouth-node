// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "forge-std/Script.sol";
import "../src/IdentityRegistry.sol";
import "../src/ReputationRegistry.sol";
import "../src/ValidationRegistry.sol";

/// @title Deploy - Monmouth ERC-8004 registry deployment
/// @author Monmouth Team
/// @notice Deploys IdentityRegistry, then the Reputation and Validation
///         registries wired to it.
/// @dev ReputationRegistry and ValidationRegistry both take the identity
///      registry address in their constructor, so ordering is mandatory.
///
///      Cooldowns (seconds between submissions per sender, 0 = disabled) are
///      read from the environment with sane defaults:
///        FEEDBACK_COOLDOWN (default 60)
///        REQUEST_COOLDOWN  (default 60)
///
///      Usage:
///        forge script script/Deploy.s.sol:Deploy \
///          --rpc-url <rpc> --private-key <key> --broadcast
contract Deploy is Script {
    /// @notice Default seconds between feedback submissions per sender.
    uint256 public constant DEFAULT_FEEDBACK_COOLDOWN = 60;

    /// @notice Default seconds between validation requests per sender.
    uint256 public constant DEFAULT_REQUEST_COOLDOWN = 60;

    function run()
        external
        returns (IdentityRegistry identity, ReputationRegistry reputation, ValidationRegistry validation)
    {
        uint256 feedbackCooldown = vm.envOr("FEEDBACK_COOLDOWN", DEFAULT_FEEDBACK_COOLDOWN);
        uint256 requestCooldown = vm.envOr("REQUEST_COOLDOWN", DEFAULT_REQUEST_COOLDOWN);

        vm.startBroadcast();

        identity = new IdentityRegistry();
        reputation = new ReputationRegistry(address(identity), feedbackCooldown);
        validation = new ValidationRegistry(address(identity), requestCooldown);

        vm.stopBroadcast();

        console2.log("IdentityRegistry   ", address(identity));
        console2.log("ReputationRegistry ", address(reputation));
        console2.log("ValidationRegistry ", address(validation));
        console2.log("feedbackCooldown   ", feedbackCooldown);
        console2.log("requestCooldown    ", requestCooldown);
    }
}
