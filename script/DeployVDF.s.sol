// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {VDFVerifier} from "../src/VDFVerifier.sol";
import {InheritanceVault} from "../src/InheritanceVault.sol";
import {Script} from "forge-std/Script.sol";
import {console} from "forge-std/console.sol";

contract DeployVDF is Script {
    uint256 internal constant ANVIL_PK = 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80;

    function run() external returns (VDFVerifier vdf, InheritanceVault vault) {
        uint256 pk = vm.envOr("DEPLOYER_PRIVATE_KEY", ANVIL_PK);
        address deployer = vm.addr(pk);

        vm.startBroadcast(pk);
        // Use a test modulus and parameters
        uint256 testN = 0x10001 * 0x7fffffff12345678;
        uint256 testT = 100;
        uint256 requiredDelay = 10; // blocks
        vdf = new VDFVerifier(testN, testT, requiredDelay);
        vault = new InheritanceVault(address(vdf));
        vm.stopBroadcast();

        console.log("deployer", deployer);
        console.log("VDFVerifier", address(vdf));
        console.log("InheritanceVault", address(vault));
    }
}
