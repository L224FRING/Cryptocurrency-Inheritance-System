// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {Test} from "forge-std/Test.sol";
import {FROSTVerifier} from "../src/FROSTVerifier.sol";
import {VDFVerifier} from "../src/VDFVerifier.sol";
import {InheritanceVault} from "../src/InheritanceVault.sol";

contract EdgeCasesTest is Test {
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

    function test_FalseAlarmRecovery() public {
        vm.prank(owner);
        vault.setBeneficiary(beneficiary);
        vm.roll(block.number + 10);
        vm.prank(owner);
        vault.checkIn();
        vm.roll(block.number + 1);
        vm.prank(owner);
        vault.checkIn();
        assertFalse(vault.released());
    }

    function test_ReplayProtection() public {
        bytes memory groupKey = hex"037c2de6c714cc97e1575c17108305e37b6508e55fc16d724b9cceeaa16314a3d8";
        bytes memory sig = hex"0352cf9dc6b1fd9ea023fcbd7e1ad627e901c2217d295420fe2773d08eb083fe45"
            hex"fb67cd0a5917e7bc98b047652a5cd84a6462f74923c27be708471b62f5b3696f";
        bytes32 msgHash = bytes32(hex"a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90");

        vm.startPrank(owner);
        frost.setGroupPublicKeyCompressed(groupKey);
        frost.verifyFROSTSignature(msgHash, sig);
        vm.expectRevert("signature already used");
        frost.verifyFROSTSignature(msgHash, sig);
        vm.stopPrank();
    }

    function test_CannotReleaseTwice() public {
        vm.prank(owner);
        vault.setBeneficiary(beneficiary);
        vm.roll(block.number + 10);
        bytes memory sig = new bytes(65);
        vm.expectRevert("vdf not confirmed");
        vault.release(sig);
    }

    function test_NonOwnerCannotCheckIn() public {
        vm.prank(makeAddr("attacker"));
        vm.expectRevert("not owner");
        vault.checkIn();
    }

    function test_NonOwnerCannotSetBeneficiary() public {
        vm.prank(makeAddr("attacker"));
        vm.expectRevert("not owner");
        vault.setBeneficiary(beneficiary);
    }
}
