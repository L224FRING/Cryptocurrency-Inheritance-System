// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

/// @notice secp256k1 point arithmetic and FROST(secp256k1, SHA-256) challenge
/// derivation. The challenge reproduces frost-secp256k1 3.0 exactly: H2 is
/// hash_to_field with ExpandMsgXmd-SHA256 and DST "FROST-secp256k1-SHA256-v1chal",
/// reduced modulo the group order via the ciphersuite's 48-byte OKM split.
library Secp256k1 {
    uint256 internal constant P = 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEFFFFFC2F;
    uint256 internal constant N = 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141;
    uint256 internal constant GX = 0x79BE667EF9DCBBAC55A06295CE870B07029BFCDB2DCE28D959F2815B16F81798;
    uint256 internal constant GY = 0x483ADA7726A3C4655DA4FBFC0E1108A8FD17B448A68554199C47D08FFB10D4B8;

    uint256 private constant SQRT_P_PLUS_1_OVER_4 = 0x3FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFBFFFFF0C;

    struct Jac {
        uint256 x;
        uint256 y;
        uint256 z;
    }

    function submod(uint256 a, uint256 b, uint256 m) internal pure returns (uint256) {
        return a >= b ? a - b : m - (b - a);
    }

    function isOnCurve(uint256 x, uint256 y) internal pure returns (bool) {
        if (x >= P || y >= P) return false;
        return mulmod(y, y, P) == addmod(mulmod(mulmod(x, x, P), x, P), 7, P);
    }

    function decompress(uint256 x, bool odd) internal view returns (uint256 px, uint256 py) {
        require(x < P, "x out of range");
        uint256 ySquared = addmod(mulmod(mulmod(x, x, P), x, P), 7, P);
        uint256 y = modExp(ySquared, SQRT_P_PLUS_1_OVER_4, P);
        require(mulmod(y, y, P) == ySquared, "point not on curve");
        if ((y & 1) != (odd ? 1 : 0)) {
            y = P - y;
        }
        return (x, y);
    }

    function modExp(uint256 base, uint256 exponent, uint256 modulus) internal view returns (uint256 result) {
        assembly {
            let ptr := mload(0x40)
            mstore(ptr, 0x20)
            mstore(add(ptr, 0x20), 0x20)
            mstore(add(ptr, 0x40), 0x20)
            mstore(add(ptr, 0x60), base)
            mstore(add(ptr, 0x80), exponent)
            mstore(add(ptr, 0xa0), modulus)
            if iszero(staticcall(gas(), 0x05, ptr, 0xc0, ptr, 0x20)) {
                revert(0, 0)
            }
            result := mload(ptr)
        }
    }

    function sha256Of(bytes memory data) internal view returns (bytes32 result) {
        assembly {
            let ok := staticcall(gas(), 0x02, add(data, 0x20), mload(data), 0x00, 0x20)
            result := mload(0x00)
            if iszero(ok) { revert(0, 0) }
        }
    }

    function frostChallenge(bytes32 message, bytes memory rCompressed, bytes memory zCompressed)
        internal
        view
        returns (uint256)
    {
        bytes memory dstPrime = abi.encodePacked(bytes("FROST-secp256k1-SHA256-v1chal"), bytes1(0x1D));
        bytes memory msgPrime =
            abi.encodePacked(new bytes(64), rCompressed, zCompressed, message, hex"0030", hex"00", dstPrime);

        bytes32 b0 = sha256Of(msgPrime);
        bytes32 b1 = sha256Of(abi.encodePacked(b0, bytes1(0x01), dstPrime));
        bytes32 b2 = sha256Of(abi.encodePacked(b0 ^ b1, bytes1(0x02), dstPrime));

        uint256 hi;
        uint256 lo;
        assembly {
            hi := shr(64, b1)
            lo := or(shl(128, and(b1, 0xffffffffffffffff)), shr(128, b2))
        }
        return addmod(mulmod(hi, 1 << 192, N), lo, N);
    }

    function verifySchnorr(bytes32 message, bytes memory rCompressed, bytes memory zCompressed, uint256 s)
        internal
        view
        returns (bool)
    {
        (uint256 rPx, uint256 rPy) = pointFromCompressed(rCompressed);
        (uint256 gX, uint256 gY) = pointFromCompressed(zCompressed);
        uint256 e = frostChallenge(message, rCompressed, zCompressed);
        return verifyEquation(e, rPx, rPy, s, gX, gY);
    }

    function pointFromCompressed(bytes memory compressed) private view returns (uint256, uint256) {
        uint8 prefix = uint8(compressed[0]);
        uint256 x;
        assembly {
            x := mload(add(compressed, 0x21))
        }
        return decompress(x, prefix == 3);
    }

    function verifyEquation(uint256 e, uint256 rPx, uint256 rPy, uint256 s, uint256 groupX, uint256 groupY)
        private
        view
        returns (bool)
    {
        Jac memory lhs = jacMul(s, GX, GY);
        Jac memory eTimesGroup = jacMul(e, groupX, groupY);
        Jac memory rhs = jacAdd(Jac(rPx, rPy, 1), eTimesGroup);
        return equalUpToInfinity(lhs, rhs);
    }

    function equalUpToInfinity(Jac memory a, Jac memory b) private view returns (bool) {
        (uint256 aAffX, uint256 aAffY, bool aInf) = toAffine(a);
        (uint256 bAffX, uint256 bAffY, bool bInf) = toAffine(b);
        if (aInf || bInf) return aInf && bInf;
        return aAffX == bAffX && aAffY == bAffY;
    }

    function jacMul(uint256 k, uint256 px, uint256 py) internal pure returns (Jac memory r) {
        r = Jac(0, 0, 0);
        Jac memory base = Jac(px, py, 1);
        for (uint256 i = 256; i > 0; i--) {
            r = jacDouble(r);
            if (((k >> (i - 1)) & 1) == 1) {
                r = jacAdd(r, base);
            }
        }
    }

    function jacDouble(Jac memory p) internal pure returns (Jac memory r) {
        if (p.z == 0 || p.y == 0) return Jac(0, 0, 0);
        r = Jac(0, 0, 0);
        uint256 ySquared = mulmod(p.y, p.y, P);
        uint256 s = mulmod(4, mulmod(p.x, ySquared, P), P);
        uint256 m = mulmod(3, mulmod(p.x, p.x, P), P);
        r.x = submod(mulmod(m, m, P), mulmod(2, s, P), P);
        r.y = submod(mulmod(m, submod(s, r.x, P), P), mulmod(8, mulmod(ySquared, ySquared, P), P), P);
        r.z = mulmod(2, mulmod(p.y, p.z, P), P);
    }

    function jacAdd(Jac memory p1, Jac memory p2) internal pure returns (Jac memory r) {
        if (p1.z == 0) return Jac(p2.x, p2.y, p2.z);
        if (p2.z == 0) return Jac(p1.x, p1.y, p1.z);
        r = Jac(0, 0, 0);
        uint256 z1z1 = mulmod(p1.z, p1.z, P);
        uint256 z2z2 = mulmod(p2.z, p2.z, P);
        uint256 u1 = mulmod(p1.x, z2z2, P);
        uint256 u2 = mulmod(p2.x, z1z1, P);
        uint256 s1 = mulmod(p1.y, mulmod(p2.z, z2z2, P), P);
        uint256 s2 = mulmod(p2.y, mulmod(p1.z, z1z1, P), P);
        if (u1 == u2) {
            if (s1 != s2) return r;
            return jacDouble(p1);
        }
        uint256 h = submod(u2, u1, P);
        uint256 twoH = mulmod(2, h, P);
        uint256 i = mulmod(twoH, twoH, P);
        uint256 j = mulmod(h, i, P);
        uint256 rCurve = mulmod(2, submod(s2, s1, P), P);
        uint256 v = mulmod(u1, i, P);
        r.x = submod(submod(mulmod(rCurve, rCurve, P), j, P), mulmod(2, v, P), P);
        r.y = submod(mulmod(rCurve, submod(v, r.x, P), P), mulmod(2, mulmod(s1, j, P), P), P);
        uint256 zSum = addmod(p1.z, p2.z, P);
        r.z = mulmod(submod(submod(mulmod(zSum, zSum, P), z1z1, P), z2z2, P), h, P);
    }

    function toAffine(Jac memory p) internal view returns (uint256, uint256, bool) {
        if (p.z == 0) return (0, 0, true);
        uint256 zInverse = modExp(p.z, P - 2, P);
        uint256 zInverseSquared = mulmod(zInverse, zInverse, P);
        return (mulmod(p.x, zInverseSquared, P), mulmod(p.y, mulmod(zInverseSquared, zInverse, P), P), false);
    }
}
