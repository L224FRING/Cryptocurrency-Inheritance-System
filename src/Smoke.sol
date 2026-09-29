// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

/// @notice Smoke test for the Solidity toolchain. Replaced by real contracts.
contract Smoke {
    uint256 public value;

    event ValueSet(uint256 previous, uint256 current);

    function setValue(uint256 newValue) external {
        emit ValueSet(value, newValue);
        value = newValue;
    }
}
