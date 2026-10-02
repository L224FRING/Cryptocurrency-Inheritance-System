// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

contract FROSTVerifier {
    address public owner;
    uint256 public groupPublicKeyX;
    uint256 public groupPublicKeyY;
    bool public hasGroupKey;
    
    // Track used signatures to prevent replay
    mapping(bytes32 => bool) public usedSignatures;

    event GroupKeySet(uint256 x, uint256 y);
    event SignatureVerified(bytes32 messageHash, address submitter);

    constructor() {
        owner = msg.sender;
        hasGroupKey = false;
    }

    function setGroupPublicKey(uint256 x, uint256 y) external {
        require(msg.sender == owner, "not owner");
        groupPublicKeyX = x;
        groupPublicKeyY = y;
        hasGroupKey = true;
        emit GroupKeySet(x, y);
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

    function getGroupPublicKey() external view returns (uint256 x, uint256 y) {
        require(hasGroupKey, "group key not set");
        return (groupPublicKeyX, groupPublicKeyY);
    }
}
