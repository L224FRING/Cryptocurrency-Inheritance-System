// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {Test} from "forge-std/Test.sol";
import {VDFVerifier} from "../src/VDFVerifier.sol";

contract VDFTest is Test {
    VDFVerifier public vdf;
    uint256 constant TEST_N = 0x10001 * 0x7fffffff12345678;
    uint256 constant TEST_T = 100;

    function setUp() public {
        vdf = new VDFVerifier(TEST_N, TEST_T, 10);
    }

    function test_InitialState() public view {
        assertFalse(vdf.inactivityConfirmed());
        assertEq(vdf.T(), TEST_T);
        assertEq(vdf.N(), TEST_N);
        assertGt(vdf.lastCheckIn(), 0);
        assertGt(vdf.currentChallenge(), 0);
    }

    function test_CheckInResetsState() public {
        bool initialConfirmed = vdf.inactivityConfirmed();
        uint256 initialChallenge = vdf.currentChallenge();
        uint256 initialLastCheckIn = vdf.lastCheckIn();
        
        vm.roll(block.number + 5);
        vdf.checkIn();
        
        assertFalse(vdf.inactivityConfirmed());
        assertNotEq(vdf.currentChallenge(), initialChallenge);
        assertEq(vdf.lastCheckIn(), block.number);
    }
}
