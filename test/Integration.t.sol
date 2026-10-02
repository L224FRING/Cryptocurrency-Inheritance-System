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
    
    address public owner = makeAddr("owner");
    address public beneficiary = makeAddr("beneficiary");
    
    uint256 constant TEST_N = 0x10001 * 0x7fffffff12345678;
    uint256 constant TEST_T = 100;

    function setUp() public {
        vm.startPrank(owner);
        vdf = new VDFVerifier(TEST_N, TEST_T, 10);
        frost = new FROSTVerifier();
        vault = new InheritanceVault(address(vdf), address(frost));
        vm.stopPrank();
    }

    function test_FullDeployment() public view {
        assertFalse(vault.released());
        assertFalse(vault.beneficiarySet());
        assertEq(vault.owner(), owner);
    }

    function test_SetBeneficiary() public {
        vm.prank(owner);
        vault.setBeneficiary(beneficiary);
        assertTrue(vault.beneficiarySet());
        assertEq(vault.beneficiary(), beneficiary);
    }

    function test_CannotReleaseWithoutVDF() public {
        vm.prank(owner);
        vault.setBeneficiary(beneficiary);
        bytes memory sig = new bytes(65);
        vm.expectRevert("vdf not confirmed");
        vault.release(sig);
    }

    function test_CheckInWorks() public {
        vm.prank(owner);
        bytes32 before = vault.currentChallenge();
        vm.roll(block.number + 1);
        vault.checkIn();
        assertNotEq(vault.currentChallenge(), before);
    }
}
