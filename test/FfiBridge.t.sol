// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {Test} from "forge-std/Test.sol";
import {Vm} from "forge-std/Vm.sol";

/// @dev Runs the FFI call in a separate frame so a failing service shows up as a
/// revert at a lower depth than the cheatcode, which `vm.expectRevert` can observe.
contract FfiCaller {
    Vm internal constant vm = Vm(address(uint160(uint256(keccak256("hevm cheat code")))));

    function run(string[] calldata cmd) external returns (bytes memory) {
        return vm.ffi(cmd);
    }
}

/// @notice Verifies the Solidity <-> Rust boundary. Foundry shells out to the FROST
/// binary over FFI and consumes its JSON report via `parseJson`, so a change to
/// the report shape fails here rather than silently degrading.
contract FfiBridgeTest is Test {
    string internal constant FROST_BIN = "rust/frost-service/target/debug/frost-service";

    FfiCaller internal ffiCaller;

    function setUp() public {
        ffiCaller = new FfiCaller();
    }

    function test_ReportsOkStatus() public {
        string memory json = _selftest();

        assertEq(vm.parseJsonString(json, ".status"), "ok");
        assertEq(vm.parseJsonString(json, ".scheme"), "frost-secp256k1");
    }

    function test_ReportsThresholdShape() public {
        string memory json = _selftest();

        assertEq(vm.parseJsonUint(json, ".dkg.trustees"), 5);
        assertEq(vm.parseJsonUint(json, ".dkg.threshold"), 3);
        assertEq(vm.parseJsonString(json, ".dkg.label"), "3-of-5");
    }

    function test_GroupVerifyingKeyIsACompressedSecp256k1Point() public {
        string memory vkey = vm.parseJsonString(_selftest(), ".group_verifying_key");

        assertEq(bytes(vkey).length, 66, "expected a 33-byte compressed key");
        string memory prefix = _slice(vkey, 0, 2);
        assertTrue(_eq(prefix, "02") || _eq(prefix, "03"), "unexpected compressed point prefix");
    }

    function test_ThresholdSubsetSigned() public {
        string memory json = _selftest();

        assertEq(vm.parseJsonString(json, ".signature.message"), "inheritance-release-attestation");
        assertEq(vm.parseJsonUint(json, ".signature.shares_aggregated"), 3);
        assertEq(vm.parseJsonArrayLength(json, ".signature.signers"), 3);
        assertTrue(vm.parseJsonBool(json, ".signature.verified_against_group_key"));
    }

    function test_SignatureIsA65ByteCompactSig() public {
        string memory sig = vm.parseJsonString(_selftest(), ".signature.value");

        assertEq(bytes(sig).length, 130, "expected a 65-byte compact signature");
    }

    function test_BelowThresholdSubsetRejected() public {
        string memory json = _selftest();

        // Two of three: one short of what the committee agreed to require.
        assertEq(vm.parseJsonArrayLength(json, ".below_threshold.signers"), 2);

        // The attempt is refused before any round runs, so there is one
        // explanation, not one line per trustee.
        assertEq(vm.parseJsonArrayLength(json, ".below_threshold.rejections"), 1);
        assertTrue(vm.parseJsonBool(json, ".below_threshold.aggregate_rejected"));

        string memory err = vm.parseJsonString(json, ".below_threshold.aggregate_error");
        assertGt(bytes(err).length, 0, "a refusal must say why");
        assertGt(_countOf(err, "threshold"), 0, "the refusal must name the threshold");
    }

    /// @dev Count non-overlapping occurrences of `needle` in `haystack`.
    function _countOf(string memory haystack, string memory needle) internal pure returns (uint256 n) {
        bytes memory h = bytes(haystack);
        bytes memory needleBytes = bytes(needle);
        if (needleBytes.length == 0 || needleBytes.length > h.length) return 0;
        for (uint256 i = 0; i + needleBytes.length <= h.length; i++) {
            bool hit = true;
            for (uint256 j = 0; j < needleBytes.length; j++) {
                if (h[i + j] != needleBytes[j]) {
                    hit = false;
                    break;
                }
            }
            if (hit) {
                n++;
                i += needleBytes.length - 1;
            }
        }
    }

    /// @dev A non-zero exit from the service must surface as a revert, so Solidity
    /// never parses a half-written report as if it were a success.
    function test_NonZeroExitReverts() public {
        string[] memory cmd = new string[](2);
        cmd[0] = FROST_BIN;
        cmd[1] = "not-a-command";

        vm.expectRevert();
        ffiCaller.run(cmd);
    }

    function _selftest() internal returns (string memory) {
        string[] memory cmd = new string[](1);
        cmd[0] = FROST_BIN;
        return string(vm.ffi(cmd));
    }

    function _slice(string memory self, uint256 start, uint256 end) internal pure returns (string memory) {
        bytes memory raw = bytes(self);
        bytes memory out = new bytes(end - start);
        for (uint256 i = start; i < end; i++) {
            out[i - start] = raw[i];
        }
        return string(out);
    }

    function _eq(string memory a, string memory b) internal pure returns (bool) {
        return keccak256(bytes(a)) == keccak256(bytes(b));
    }
}
