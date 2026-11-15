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

    function test_SubmitProofRejectsInvalidProof() public {
        vm.roll(block.number + 10);
        uint256[] memory proof = new uint256[](3);
        proof[0] = 1;
        proof[1] = 2;
        proof[2] = 3;
        // The proof is bound to currentChallenge; a dummy proof must fail.
        vm.expectRevert("invalid proof");
        vdf.submitVDFProof(12345, proof);
    }

    function test_SubmitProofRejectsEarlySubmission() public {
        uint256[] memory proof = new uint256[](1);
        vm.expectRevert("too early");
        vdf.submitVDFProof(12345, proof);
    }
}
