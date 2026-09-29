// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {Smoke} from "../src/Smoke.sol";
import {Test} from "forge-std/Test.sol";

/// @notice Verifies the Foundry toolchain compiles, deploys, and runs assertions.
contract SmokeTest is Test {
    Smoke internal smoke;

    function setUp() public {
        smoke = new Smoke();
    }

    function test_SetValue() public {
        smoke.setValue(42);
        assertEq(smoke.value(), 42);
    }
}
