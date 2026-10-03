// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {Test} from "forge-std/Test.sol";
import {VDFVerifier} from "../src/VDFVerifier.sol";

contract VDFRealTest is Test {
    VDFVerifier public vdf;
    uint256 constant TEST_N = 0x10001 * 0x7fffffff12345678;
    uint256 constant TEST_T = 8; // Small for fast testing

    function setUp() public {
        vdf = new VDFVerifier(TEST_N, TEST_T, 5);
    }

    function test_VDFProofStructure() public view {
        assertEq(vdf.T(), TEST_T);
        assertEq(vdf.N(), TEST_N);
        assertFalse(vdf.inactivityConfirmed());
    }

    function test_SubmitProofWithParams() public {
        vm.roll(block.number + 10);
        uint256 x = uint256(keccak256("test")) % TEST_N;
        uint256[] memory proof = new uint256[](3);
        proof[0] = 1;
        proof[1] = 2;
        proof[2] = 3;
        // This will fail verification with dummy proof - but tests the interface
        vm.expectRevert("invalid proof");
        vdf.submitVDFProofWithParams(x, 12345, proof, TEST_T, TEST_N);
    }
}
