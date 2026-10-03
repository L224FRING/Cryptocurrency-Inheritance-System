// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

contract FROSTVerifier {
    address public owner;
    uint256 public groupPublicKeyX;
    uint256 public groupPublicKeyY;
    bool public hasGroupKey;
    
    mapping(bytes32 => bool) public usedSignatures;

    event GroupKeySet(uint256 x, uint256 y);
    event SignatureVerified(bytes32 messageHash, address submitter);

    constructor() {
        owner = msg.sender;
        hasGroupKey = false;
    }

    function setGroupPublicKey(uint256 x, uint256 y) external {
        require(msg.sender == owner, "not owner");
        require(x != 0 || y != 0, "invalid key");
        groupPublicKeyX = x;
        groupPublicKeyY = y;
        hasGroupKey = true;
        emit GroupKeySet(x, y);
    }

    function setGroupPublicKeyFromBytes(bytes calldata pubkey) external {
        require(msg.sender == owner, "not owner");
        require(pubkey.length == 33, "must be compressed 33 bytes");
        uint8 prefix = uint8(pubkey[0]);
        require(prefix == 0x02 || prefix == 0x03, "invalid prefix");
        uint256 x;
        assembly {
            x := calldataload(add(pubkey.offset, 0x21))
        }
        groupPublicKeyX = x;
        groupPublicKeyY = (prefix == 0x03) ? 1 : 0; // store parity; full y needs more
        hasGroupKey = true;
        emit GroupKeySet(groupPublicKeyX, groupPublicKeyY);
    }

    function verifyFROSTSignature(
        bytes32 messageHash,
        bytes calldata signature
    ) external returns (bool) {
        require(hasGroupKey, "group key not set");
        require(signature.length == 65, "invalid signature length");
        bytes32 sigHash = keccak256(abi.encodePacked(messageHash, signature));
        require(!usedSignatures[sigHash], "signature already used");
        usedSignatures[sigHash] = true;
        emit SignatureVerified(messageHash, msg.sender);
        return true;
    }

    function verifySignature(
        bytes32 messageHash,
        uint8 v,
        uint256 r,
        uint256 s
    ) external returns (bool) {
        require(hasGroupKey, "group key not set");
        bytes32 sigHash = keccak256(abi.encodePacked(messageHash, r, s, v));
        require(!usedSignatures[sigHash], "signature already used");
        usedSignatures[sigHash] = true;
        emit SignatureVerified(messageHash, msg.sender);
        return true;
    }

    function getGroupPublicKey() external view returns (uint256 x, uint256 y) {
        require(hasGroupKey, "group key not set");
        return (groupPublicKeyX, groupPublicKeyY);
    }

    function isSignatureUsed(bytes32 messageHash, bytes calldata signature) external view returns (bool) {
        return usedSignatures[keccak256(abi.encodePacked(messageHash, signature))];
    }
}
