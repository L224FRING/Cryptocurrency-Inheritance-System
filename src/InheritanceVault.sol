// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {VDFVerifier} from "./VDFVerifier.sol";

contract InheritanceVault {
    struct CheckIn {
        uint256 blockNumber;
        uint256 timestamp;
        bytes32 challenge;
    }

    VDFVerifier public immutable vdfVerifier;
    address public immutable owner;
    uint256 public lastCheckInBlock;
    uint256 public lastCheckInTime;
    bytes32 public currentChallenge;
    bool public released;

    event CheckedIn(address indexed owner, uint256 blockNumber, uint256 timestamp, bytes32 challenge);
    event Released(address indexed beneficiary, uint256 timestamp);

    modifier onlyOwner() {
        require(msg.sender == owner, "not owner");
        _;
    }

    constructor(address _vdfVerifier) {
        require(_vdfVerifier != address(0), "zero address");
        vdfVerifier = VDFVerifier(_vdfVerifier);
        owner = msg.sender;
        lastCheckInBlock = block.number;
        lastCheckInTime = block.timestamp;
        currentChallenge = keccak256(abi.encodePacked(blockhash(block.number - 1), block.timestamp, msg.sender));
        released = false;
    }

    function checkIn() external onlyOwner {
        require(!released, "already released");
        lastCheckInBlock = block.number;
        lastCheckInTime = block.timestamp;
        currentChallenge = keccak256(abi.encodePacked(
            blockhash(block.number - 1),
            block.timestamp,
            msg.sender,
            lastCheckInBlock
        ));
        emit CheckedIn(owner, lastCheckInBlock, lastCheckInTime, currentChallenge);
    }

    function getCurrentChallenge() external view returns (bytes32) {
        return currentChallenge;
    }

    function isReleased() external view returns (bool) {
        return released;
    }
}
