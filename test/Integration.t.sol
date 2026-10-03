// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {Test} from "forge-std/Test.sol";
import {VDFVerifier} from "../src/VDFVerifier.sol";
import {FROSTVerifier} from "../src/FROSTVerifier.sol";
import {InheritanceVault} from "../src/InheritanceVault.sol";

contract IntegrationTest is Test {
    VDFVerifier public vdf;
    FROSTVerifier public frost;
    InheritanceVault public vault;
    address public owner;
    address public beneficiary;

    function setUp() public {
        owner = makeAddr("owner");
        beneficiary = makeAddr("beneficiary");
        vm.startPrank(owner);
        vdf = new VDFVerifier(0x10001 * 0x7fffffff12345678, 100, 5);
        frost = new FROSTVerifier();
        vault = new InheritanceVault(address(vdf), address(frost));
        vm.stopPrank();
    }

    function test_CheckInWorks() public {
        vm.startPrank(owner);
        vault.setBeneficiary(beneficiary);
        vm.roll(block.number + 10);
        vault.checkIn();
        vm.stopPrank();
        assertTrue(true);
    }
}
