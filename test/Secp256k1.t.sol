// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {Test} from "forge-std/Test.sol";
import {Secp256k1} from "../src/Secp256k1.sol";

contract Secp256k1Test is Test {
    uint256 internal constant TWO_GX = 0xC6047F9441ED7D6D3045406E95C07CD85C778E4B8CEF3CA7ABAC09B95C709EE5;
    uint256 internal constant TWO_GY = 0x1AE168FEA63DC339A3C58419466CEAEEF7F632653266D0E1236431A950CFE52A;

    function test_generatorIsOnCurve() public pure {
        assertTrue(Secp256k1.isOnCurve(Secp256k1.GX, Secp256k1.GY));
    }

    function test_pointAtInfinityNotOnCurve() public pure {
        assertFalse(Secp256k1.isOnCurve(0, 0));
    }

    function test_decompressGenerator() public view {
        (uint256 x, uint256 y) = Secp256k1.decompress(Secp256k1.GX, false);
        assertEq(x, Secp256k1.GX);
        assertEq(y, Secp256k1.GY);
    }

    function test_decompressGeneratorOdd() public view {
        (uint256 x, uint256 y) = Secp256k1.decompress(Secp256k1.GX, true);
        assertEq(x, Secp256k1.GX);
        assertEq(y, Secp256k1.P - Secp256k1.GY);
    }

    function test_decompressRejectsOffCurvePoint() public {
        vm.expectRevert(bytes("point not on curve"));
        this.externalDecompress(5, false);
    }

    function test_decompressRejectsOutOfRangeX() public {
        vm.expectRevert(bytes("x out of range"));
        this.externalDecompress(Secp256k1.P, false);
    }

    function externalDecompress(uint256 x, bool odd) external view returns (uint256, uint256) {
        return Secp256k1.decompress(x, odd);
    }

    function test_scalarMul1IsGenerator() public view {
        Secp256k1.Jac memory p = Secp256k1.jacMul(1, Secp256k1.GX, Secp256k1.GY);
        (uint256 x, uint256 y, bool inf) = Secp256k1.toAffine(p);
        assertFalse(inf);
        assertEq(x, Secp256k1.GX);
        assertEq(y, Secp256k1.GY);
    }

    function test_scalarMul2IsTwoG() public view {
        Secp256k1.Jac memory p = Secp256k1.jacMul(2, Secp256k1.GX, Secp256k1.GY);
        (uint256 x, uint256 y, bool inf) = Secp256k1.toAffine(p);
        assertFalse(inf);
        assertEq(x, TWO_GX);
        assertEq(y, TWO_GY);
    }

    function test_scalarMulZeroIsInfinity() public view {
        Secp256k1.Jac memory p = Secp256k1.jacMul(0, Secp256k1.GX, Secp256k1.GY);
        (,, bool inf) = Secp256k1.toAffine(p);
        assertTrue(inf);
    }

    function test_scalarMulGroupOrderIsInfinity() public view {
        Secp256k1.Jac memory p = Secp256k1.jacMul(Secp256k1.N, Secp256k1.GX, Secp256k1.GY);
        (,, bool inf) = Secp256k1.toAffine(p);
        assertTrue(inf);
    }

    function test_scalarMulOrderMinusOneIsNegGenerator() public view {
        Secp256k1.Jac memory p = Secp256k1.jacMul(Secp256k1.N - 1, Secp256k1.GX, Secp256k1.GY);
        (uint256 x, uint256 y, bool inf) = Secp256k1.toAffine(p);
        assertFalse(inf);
        assertEq(x, Secp256k1.GX);
        assertEq(y, Secp256k1.P - Secp256k1.GY);
    }

    function test_scalarMulDistributes() public view {
        // 3G via jacMul(3) must equal jacAdd(2G, G)
        Secp256k1.Jac memory viaMul = Secp256k1.jacMul(3, Secp256k1.GX, Secp256k1.GY);
        Secp256k1.Jac memory viaAdd =
            Secp256k1.jacAdd(Secp256k1.Jac(TWO_GX, TWO_GY, 1), Secp256k1.Jac(Secp256k1.GX, Secp256k1.GY, 1));
        (uint256 ax, uint256 ay, bool aInf) = Secp256k1.toAffine(viaMul);
        (uint256 bx, uint256 by, bool bInf) = Secp256k1.toAffine(viaAdd);
        assertFalse(aInf);
        assertFalse(bInf);
        assertEq(ax, bx);
        assertEq(ay, by);
    }

    function test_jacAddWithInfinity() public view {
        Secp256k1.Jac memory inf = Secp256k1.Jac(0, 0, 0);
        Secp256k1.Jac memory g = Secp256k1.Jac(Secp256k1.GX, Secp256k1.GY, 1);
        Secp256k1.Jac memory sum1 = Secp256k1.jacAdd(inf, g);
        Secp256k1.Jac memory sum2 = Secp256k1.jacAdd(g, inf);
        (uint256 x1, uint256 y1, bool inf1) = Secp256k1.toAffine(sum1);
        (uint256 x2, uint256 y2, bool inf2) = Secp256k1.toAffine(sum2);
        assertFalse(inf1);
        assertFalse(inf2);
        assertEq(x1, Secp256k1.GX);
        assertEq(y1, Secp256k1.GY);
        assertEq(x2, Secp256k1.GX);
        assertEq(y2, Secp256k1.GY);
    }

    function test_jacAddPointPlusNegationIsInfinity() public view {
        Secp256k1.Jac memory g = Secp256k1.Jac(Secp256k1.GX, Secp256k1.GY, 1);
        Secp256k1.Jac memory negG = Secp256k1.Jac(Secp256k1.GX, Secp256k1.P - Secp256k1.GY, 1);
        Secp256k1.Jac memory sum = Secp256k1.jacAdd(g, negG);
        (,, bool inf) = Secp256k1.toAffine(sum);
        assertTrue(inf);
    }

    function test_jacAddDoublingIsConsistentWithjacDouble() public view {
        Secp256k1.Jac memory g = Secp256k1.Jac(Secp256k1.GX, Secp256k1.GY, 1);
        Secp256k1.Jac memory viaAdd = Secp256k1.jacAdd(g, g);
        Secp256k1.Jac memory viaDouble = Secp256k1.jacDouble(g);
        (uint256 ax, uint256 ay, bool aInf) = Secp256k1.toAffine(viaAdd);
        (uint256 bx, uint256 by, bool bInf) = Secp256k1.toAffine(viaDouble);
        assertFalse(aInf);
        assertFalse(bInf);
        assertEq(ax, bx);
        assertEq(ay, by);
    }

    function test_frostChallengeLengthAndRange() public view {
        bytes memory rCompressed = abi.encodePacked(uint8(0x02), bytes32(uint256(1)));
        bytes memory zCompressed = abi.encodePacked(uint8(0x02), bytes32(Secp256k1.GX));
        uint256 e1 = Secp256k1.frostChallenge(bytes32(uint256(0xdead)), rCompressed, zCompressed);
        uint256 e2 = Secp256k1.frostChallenge(bytes32(uint256(0xbeef)), rCompressed, zCompressed);
        assertGt(e1, 0);
        assertLt(e1, Secp256k1.N);
        assertGt(e2, 0);
        assertLt(e2, Secp256k1.N);
        assertTrue(e1 != e2);
    }

    function test_frostChallengeIsDeterministic() public view {
        bytes memory rCompressed = abi.encodePacked(uint8(0x03), bytes32(uint256(0x1234)));
        bytes memory zCompressed = abi.encodePacked(uint8(0x02), bytes32(Secp256k1.GX));
        bytes32 msg = bytes32(uint256(0xcafe));
        uint256 e1 = Secp256k1.frostChallenge(msg, rCompressed, zCompressed);
        uint256 e2 = Secp256k1.frostChallenge(msg, rCompressed, zCompressed);
        assertEq(e1, e2);
    }

    function test_frostChallengeChangesWithR() public view {
        bytes memory zCompressed = abi.encodePacked(uint8(0x02), bytes32(Secp256k1.GX));
        bytes32 msg = bytes32(uint256(0xcafe));
        uint256 e1 = Secp256k1.frostChallenge(msg, abi.encodePacked(uint8(0x02), bytes32(uint256(1))), zCompressed);
        uint256 e2 = Secp256k1.frostChallenge(msg, abi.encodePacked(uint8(0x03), bytes32(uint256(1))), zCompressed);
        uint256 e3 = Secp256k1.frostChallenge(msg, abi.encodePacked(uint8(0x02), bytes32(uint256(2))), zCompressed);
        assertTrue(e1 != e2);
        assertTrue(e1 != e3);
    }
}
