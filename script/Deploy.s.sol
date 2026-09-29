// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {Smoke} from "../src/Smoke.sol";
import {Script} from "forge-std/Script.sol";
import {console} from "forge-std/console.sol";

/// @notice Deploys contracts to the local anvil testnet.
contract Deploy is Script {
    /// @dev Anvil's first deterministic account, used when no key is supplied.
    uint256 internal constant ANVIL_PK = 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80;

    function run() external returns (Smoke deployed) {
        uint256 pk = vm.envOr("DEPLOYER_PRIVATE_KEY", ANVIL_PK);
        address deployer = vm.addr(pk);

        vm.startBroadcast(pk);
        deployed = new Smoke();
        vm.stopBroadcast();

        console.log("deployer", deployer);
        console.log("Smoke", address(deployed));
    }
}
