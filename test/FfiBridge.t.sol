// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {Test} from "forge-std/Test.sol";

/// @notice Verifies the Solidity <-> Rust boundary works: Foundry shells out to the
/// FROST binary over FFI and parses its stdout. Real contracts will consume the
/// same JSON interface.
contract FfiBridgeTest is Test {
    function test_FfiCallsFrostService() public {
        string memory output = _frost();
        // The service prints a human-readable summary; assert on a stable marker.
        assertTrue(_contains(output, "threshold             3-of-5"));
        assertTrue(_contains(output, "verified against vkey ok"));
    }

    function _frost() internal returns (string memory) {
        string[] memory cmd = new string[](1);
        cmd[0] = "rust/frost-service/target/debug/frost-service";
        bytes memory result = vm.ffi(cmd);
        return string(result);
    }

    function _contains(string memory haystack, string memory needle) internal pure returns (bool) {
        bytes memory h = bytes(haystack);
        bytes memory n = bytes(needle);
        if (bytes(n).length == 0) return true;
        if (h.length < n.length) return false;
        for (uint256 i = 0; i <= h.length - n.length; i++) {
            bool matchFound = true;
            for (uint256 j = 0; j < n.length; j++) {
                if (h[i + j] != n[j]) {
                    matchFound = false;
                    break;
                }
            }
            if (matchFound) return true;
        }
        return false;
    }
}
