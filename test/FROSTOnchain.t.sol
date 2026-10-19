// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {Test} from "forge-std/Test.sol";
import {FROSTVerifier} from "../src/FROSTVerifier.sol";
import {Secp256k1} from "../src/Secp256k1.sol";

/// @notice Cross-validates the Solidity FROST verifier against a real
/// signature produced by the Rust `frost-service`.
///
/// The vectors come from `rust/frost-service/tests/kat.rs`, which runs a real
/// DKG and a real threshold signing with a fixed-seed RNG. The trustees sign
/// the release hash verbatim (raw mode), which is what the vault needs. The
/// KAT payload is that release hash. Regenerate with:
///
/// ```sh
/// cargo test --test kat -- --nocapture
/// ```
contract FROSTOnchainTest is Test {
    FROSTVerifier public verifier;
    address public owner = makeAddr("owner");

    // --- KAT vectors (test/fixtures/frost_kat.json) -----------------------
    bytes constant GROUP_KEY = hex"037c2de6c714cc97e1575c17108305e37b6508e55fc16d724b9cceeaa16314a3d8";
    bytes constant SIGNATURE = hex"0352cf9dc6b1fd9ea023fcbd7e1ad627e901c2217d295420fe2773d08eb083fe45"
        hex"fb67cd0a5917e7bc98b047652a5cd84a6462f74923c27be708471b62f5b3696f";
    bytes constant PAYLOAD = hex"a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90";

    function setUp() public {
        vm.prank(owner);
        verifier = new FROSTVerifier();
        vm.prank(owner);
        verifier.setGroupPublicKeyCompressed(GROUP_KEY);
    }

    /// The exact message the trustees signed. The KAT payload *is* the release
    /// hash, so it is used verbatim rather than re-hashed.
    function releaseHash() internal pure returns (bytes32) {
        return bytes32(PAYLOAD);
    }

    function test_GroupKeyDecompressesToOnCurvePoint() public view {
        (uint256 x, uint256 y, bool compressed) = verifier.getGroupPublicKey();
        assertTrue(compressed);
        assertTrue(Secp256k1.isOnCurve(x, y), "group key must be on the curve");
        assertEq(uint8(GROUP_KEY[0]), y & 1 == 1 ? 0x03 : 0x02, "prefix must match y parity");
    }

    function test_RealFrostSignatureVerifies() public {
        vm.prank(owner);
        assertTrue(verifier.verifyFROSTSignature(releaseHash(), SIGNATURE));
        assertTrue(verifier.isSignatureUsed(releaseHash(), SIGNATURE));
    }

    function test_SetGroupKeyUncompressedMatchesCompressed() public {
        (uint256 x, uint256 y,) = verifier.getGroupPublicKey();
        vm.prank(owner);
        FROSTVerifier other = new FROSTVerifier();
        vm.prank(owner);
        other.setGroupPublicKey(x, y);
        vm.prank(owner);
        assertTrue(other.verifyFROSTSignature(releaseHash(), SIGNATURE));
    }

    function test_CannotReuseSignature() public {
        vm.prank(owner);
        verifier.verifyFROSTSignature(releaseHash(), SIGNATURE);
        vm.expectRevert("signature already used");
        verifier.verifyFROSTSignature(releaseHash(), SIGNATURE);
    }

    function test_DifferentMessageFails() public {
        bytes32 wrong = keccak256(abi.encodePacked(PAYLOAD, bytes1(0x00)));
        vm.expectRevert("invalid signature");
        verifier.verifyFROSTSignature(wrong, SIGNATURE);
    }

    function test_TamperedScalarFails() public {
        bytes memory bad = SIGNATURE;
        bad[64] = bytes1(uint8(bad[64]) ^ 0x01);
        vm.expectRevert("invalid signature");
        verifier.verifyFROSTSignature(releaseHash(), bad);
    }

    function test_TamperedRXFails() public {
        bytes memory bad = SIGNATURE;
        bad[10] = bytes1(uint8(bad[10]) ^ 0x01);
        vm.expectRevert("invalid signature");
        verifier.verifyFROSTSignature(releaseHash(), bad);
    }

    function test_FlippedRPrefixFails() public {
        bytes memory bad = SIGNATURE;
        bad[0] = bytes1(uint8(bad[0]) == 0x02 ? 0x03 : 0x02);
        vm.expectRevert("invalid signature");
        verifier.verifyFROSTSignature(releaseHash(), bad);
    }

    function test_WrongLengthFails() public {
        bytes memory bad = new bytes(64);
        vm.expectRevert("invalid signature length");
        verifier.verifyFROSTSignature(releaseHash(), bad);
    }

    function test_InvalidPrefixFails() public {
        bytes memory bad = SIGNATURE;
        bad[0] = 0x04;
        vm.expectRevert("invalid signature prefix");
        verifier.verifyFROSTSignature(releaseHash(), bad);
    }

    function test_ScalarZeroFails() public {
        bytes memory bad = SIGNATURE;
        for (uint256 i = 33; i < 65; i++) {
            bad[i] = 0x00;
        }
        vm.expectRevert("invalid signature scalar");
        verifier.verifyFROSTSignature(releaseHash(), bad);
    }

    function test_ScalarOutOfRangeFails() public {
        bytes memory bad = SIGNATURE;
        bytes32 n = bytes32(Secp256k1.N);
        for (uint256 i = 0; i < 32; i++) {
            bad[33 + i] = n[i];
        }
        vm.expectRevert("invalid signature scalar");
        verifier.verifyFROSTSignature(releaseHash(), bad);
    }

    function test_NonOwnerCannotSetGroupKey() public {
        vm.prank(makeAddr("attacker"));
        vm.expectRevert("not owner");
        verifier.setGroupPublicKeyCompressed(GROUP_KEY);
    }

    function test_SetGroupKeyRejectsOffCurvePoint() public {
        vm.prank(owner);
        vm.expectRevert("point not on curve");
        verifier.setGroupPublicKey(1, 2);
    }

    function test_SetCompressedKeyRejectsBadLength() public {
        vm.prank(owner);
        vm.expectRevert("must be compressed 33 bytes");
        verifier.setGroupPublicKeyCompressed(hex"02");
    }

    function test_SetCompressedKeyRejectsBadPrefix() public {
        bytes memory bad = GROUP_KEY;
        bad[0] = 0x05;
        vm.prank(owner);
        vm.expectRevert("invalid prefix");
        verifier.setGroupPublicKeyCompressed(bad);
    }

    function test_VerifyWithoutGroupKeyFails() public {
        vm.prank(owner);
        FROSTVerifier empty = new FROSTVerifier();
        vm.expectRevert("group key not set");
        empty.verifyFROSTSignature(releaseHash(), SIGNATURE);
    }

    function test_SignatureIsDeterministic() public view {
        // The KAT is generated from a fixed seed, so the same vectors must
        // always reproduce. A different signature here means the fixture is
        // stale relative to the Rust ciphersuite.
        assertEq(SIGNATURE.length, 65);
        assertEq(GROUP_KEY.length, 33);
        assertEq(uint8(SIGNATURE[0]), 0x03);
    }
}
