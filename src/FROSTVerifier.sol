// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {Secp256k1} from "./Secp256k1.sol";

contract FROSTVerifier {
    address public owner;
    uint256 public groupPublicKeyX;
    uint256 public groupPublicKeyY;
    bool public hasGroupKey;
    bool public groupKeyIsCompressed;
    uint8 public groupKeyPrefix;

    event GroupKeySet(uint256 x, uint256 y, bool compressed);

    constructor() {
        owner = msg.sender;
        hasGroupKey = false;
    }

    /// @notice The committee key is one-shot. Letting the owner swap it after
    /// trustees sign would let them invalidate or redirect a release at will.
    function setGroupPublicKey(uint256 x, uint256 y) external {
        require(msg.sender == owner, "not owner");
        require(!hasGroupKey, "group key already set");
        require(Secp256k1.isOnCurve(x, y), "point not on curve");
        groupPublicKeyX = x;
        groupPublicKeyY = y;
        groupKeyPrefix = uint8(y & 1 == 1 ? 0x03 : 0x02);
        hasGroupKey = true;
        groupKeyIsCompressed = false;
        emit GroupKeySet(x, y, false);
    }

    function setGroupPublicKeyCompressed(bytes calldata pubkey) external {
        require(msg.sender == owner, "not owner");
        require(!hasGroupKey, "group key already set");
        require(pubkey.length == 33, "must be compressed 33 bytes");
        uint8 prefix = uint8(pubkey[0]);
        require(prefix == 0x02 || prefix == 0x03, "invalid prefix");
        uint256 x;
        assembly {
            x := calldataload(add(pubkey.offset, 0x01))
        }
        (uint256 px, uint256 py) = Secp256k1.decompress(x, prefix == 0x03);
        groupPublicKeyX = px;
        groupPublicKeyY = py;
        groupKeyPrefix = prefix;
        hasGroupKey = true;
        groupKeyIsCompressed = true;
        emit GroupKeySet(px, py, true);
    }

    /// @notice Stateless verification. Replay protection lives in the vault so
    /// that a third party cannot consume a valid release signature by calling
    /// this verifier first.
    function verifyFROSTSignature(bytes32 messageHash, bytes calldata signature) external view returns (bool) {
        require(hasGroupKey, "group key not set");
        require(signature.length == 65, "invalid signature length");
        uint8 prefix = uint8(signature[0]);
        require(prefix == 0x02 || prefix == 0x03, "invalid signature prefix");

        uint256 s;
        assembly {
            s := calldataload(add(signature.offset, 0x21))
        }
        require(s > 0 && s < Secp256k1.N, "invalid signature scalar");

        bytes memory rCompressed = signature[0:33];
        bytes memory zCompressed = abi.encodePacked(groupKeyPrefix, bytes32(groupPublicKeyX));
        require(Secp256k1.verifySchnorr(messageHash, rCompressed, zCompressed, s), "invalid signature");

        return true;
    }

    function getGroupPublicKey() external view returns (uint256 x, uint256 y, bool compressed) {
        require(hasGroupKey, "group key not set");
        return (groupPublicKeyX, groupPublicKeyY, groupKeyIsCompressed);
    }
}
