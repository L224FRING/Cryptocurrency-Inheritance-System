// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {Test} from "forge-std/Test.sol";
import {FROSTVerifier} from "../src/FROSTVerifier.sol";
import {VDFVerifier} from "../src/VDFVerifier.sol";
import {InheritanceVault} from "../src/InheritanceVault.sol";

contract FROSTEndToEndTest is Test {
    FROSTVerifier public frost;
    VDFVerifier public vdf;
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

    function test_FullSetup() public {
        vm.prank(owner);
        vault.setBeneficiary(beneficiary);
        vm.prank(owner);
        frost.setGroupPublicKeyCompressed(hex"037c2de6c714cc97e1575c17108305e37b6508e55fc16d724b9cceeaa16314a3d8");
        assertTrue(vault.beneficiarySet());
        assertTrue(frost.hasGroupKey());
    }
}
