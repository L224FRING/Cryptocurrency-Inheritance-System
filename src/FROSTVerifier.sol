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

    mapping(bytes32 => bool) public usedSignatures;

    event GroupKeySet(uint256 x, uint256 y, bool compressed);
    event SignatureVerified(bytes32 messageHash, address submitter);

    constructor() {
        owner = msg.sender;
        hasGroupKey = false;
    }

    function setGroupPublicKey(uint256 x, uint256 y) external {
        require(msg.sender == owner, "not owner");
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

    function verifyFROSTSignature(bytes32 messageHash, bytes calldata signature) external returns (bool) {
        require(hasGroupKey, "group key not set");
        require(signature.length == 65, "invalid signature length");
        uint8 prefix = uint8(signature[0]);
        require(prefix == 0x02 || prefix == 0x03, "invalid signature prefix");

        uint256 s;
        assembly {
            s := calldataload(add(signature.offset, 0x21))
        }
        require(s > 0 && s < Secp256k1.N, "invalid signature scalar");

        bytes32 sigHash = keccak256(abi.encodePacked(messageHash, signature));
        require(!usedSignatures[sigHash], "signature already used");

        bytes memory rCompressed = signature[0:33];
        bytes memory zCompressed = abi.encodePacked(groupKeyPrefix, bytes32(groupPublicKeyX));
        require(Secp256k1.verifySchnorr(messageHash, rCompressed, zCompressed, s), "invalid signature");

        usedSignatures[sigHash] = true;
        emit SignatureVerified(messageHash, msg.sender);
        return true;
    }

    function getGroupPublicKey() external view returns (uint256 x, uint256 y, bool compressed) {
        require(hasGroupKey, "group key not set");
        return (groupPublicKeyX, groupPublicKeyY, groupKeyIsCompressed);
    }

    function isSignatureUsed(bytes32 messageHash, bytes calldata signature) external view returns (bool) {
        return usedSignatures[keccak256(abi.encodePacked(messageHash, signature))];
    }
}
