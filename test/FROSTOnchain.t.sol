// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {Test} from "forge-std/Test.sol";
import {FROSTVerifier} from "../src/FROSTVerifier.sol";

contract FROSTOnchainTest is Test {
    FROSTVerifier public verifier;
    address public owner = makeAddr("owner");

    function setUp() public {
        vm.prank(owner);
        verifier = new FROSTVerifier();
    }

    function test_SetGroupKey() public {
        vm.prank(owner);
        verifier.setGroupPublicKey(1, 2);
        (uint256 x, uint256 y, bool compressed) = verifier.getGroupPublicKey();
        assertEq(x, 1);
        assertEq(y, 2);
        assertFalse(compressed);
    }

    function test_SetCompressedKey() public {
        vm.prank(owner);
        bytes memory compressed = new bytes(33);
        compressed[0] = 0x02;
        for (uint i = 1; i < 33; i++) {
            compressed[i] = bytes1(uint8(i));
        }
        verifier.setGroupPublicKeyCompressed(compressed);
        (uint256 x, uint256 y, bool comp) = verifier.getGroupPublicKey();
        assertTrue(comp);
    }

    function test_VerifySignature() public {
        vm.prank(owner);
        verifier.setGroupPublicKey(1, 2);
        bytes32 msgHash = keccak256("test");
        bytes memory sig = new bytes(65);
        sig[64] = 0x1b;
        verifier.verifyFROSTSignature(msgHash, sig);
        assertTrue(verifier.isSignatureUsed(msgHash, sig));
    }

    function test_CannotReuseSignature() public {
        vm.prank(owner);
        verifier.setGroupPublicKey(1, 2);
        bytes32 msgHash = keccak256("test");
        bytes memory sig = new bytes(65);
        verifier.verifyFROSTSignature(msgHash, sig);
        vm.expectRevert("signature already used");
        verifier.verifyFROSTSignature(msgHash, sig);
    }
}
