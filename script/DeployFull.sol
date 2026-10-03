// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {VDFVerifier} from "../src/VDFVerifier.sol";
import {FROSTVerifier} from "../src/FROSTVerifier.sol";
import {InheritanceVault} from "../src/InheritanceVault.sol";
import {Script} from "forge-std/Script.sol";
import {console} from "forge-std/console.sol";

contract DeployFull is Script {
    uint256 internal constant ANVIL_PK = 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80;

    function run() external returns (VDFVerifier vdf, FROSTVerifier frost, InheritanceVault vault) {
        uint256 pk = vm.envOr("DEPLOYER_PRIVATE_KEY", ANVIL_PK);
        address deployer = vm.addr(pk);

        vm.startBroadcast(pk);
        uint256 testN = 604472132917888474175096;
        uint256 testT = 100;
        uint256 requiredDelay = 10;
        vdf = new VDFVerifier(testN, testT, requiredDelay);
        frost = new FROSTVerifier();
        vault = new InheritanceVault(address(vdf), address(frost));
        vm.stopBroadcast();

        console.log("deployer", deployer);
        console.log("VDFVerifier", address(vdf));
        console.log("FROSTVerifier", address(frost));
        console.log("InheritanceVault", address(vault));
    }
}
