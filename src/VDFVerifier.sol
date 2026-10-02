// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {Test} from "forge-std/Test.sol";

contract VDFVerifier {
    struct VDFChallenge {
        uint256 x;
        uint256 y;
        uint256 n;
        uint256 t;
        uint256 timestamp;
    }

    uint256 public N;
    uint256 public T;
    uint256 public currentChallenge;
    uint256 public lastCheckIn;
    uint256 public requiredDelay;
    bool public inactivityConfirmed;

    event CheckIn(uint256 challenge, uint256 blockNumber, uint256 timestamp);
    event VDFSubmitted(uint256 y, address submitter);
    event InactivityConfirmed();

    constructor(uint256 _n, uint256 _t, uint256 _requiredDelay) {
        N = _n;
        T = _t;
        requiredDelay = _requiredDelay;
        lastCheckIn = block.number;
        currentChallenge = generateChallenge();
        inactivityConfirmed = false;
    }

    function generateChallenge() internal view returns (uint256) {
        return uint256(keccak256(abi.encodePacked(
            blockhash(block.number - 1),
            block.timestamp,
            lastCheckIn
        )));
    }

    function checkIn() external {
        lastCheckIn = block.number;
        currentChallenge = generateChallenge();
        inactivityConfirmed = false;
        emit CheckIn(currentChallenge, block.number, block.timestamp);
    }

    function mulmod(uint256 a, uint256 b, uint256 modulus) internal pure returns (uint256) {
        return (a * b) % modulus;
    }

    function modexp(uint256 base, uint256 exponent, uint256 modulus) internal view returns (uint256 result) {
        require(modulus > 1, "modulus must be > 1");
        assembly {
            let free_ptr := mload(0x40)
            mstore(free_ptr, 0x20)
            mstore(add(free_ptr, 0x20), 0x20)
            mstore(add(free_ptr, 0x40), 0x20)
            mstore(add(free_ptr, 0x60), base)
            mstore(add(free_ptr, 0x80), exponent)
            mstore(add(free_ptr, 0xA0), modulus)
            let success := staticcall(gas(), 0x05, free_ptr, 0xC0, free_ptr, 0x20)
            switch success
            case 0 {
                revert(0, 0)
            }
            default {
                result := mload(free_ptr)
            }
        }
    }

    function verifyVDF(
        uint256 x,
        uint256 y,
        uint256[] calldata halfwayPoints,
        uint256 t,
        uint256 n
    ) public view returns (bool) {
        uint256 curX = x;
        uint256 curY = y;
        uint256 curT = t;
        uint256 idx = 0;

        while (curT > 1 && idx < halfwayPoints.length) {
            uint256 mu = halfwayPoints[idx];
            uint256 r = uint256(keccak256(abi.encodePacked(curX, curY, mu, n))) % n;
            curX = mulmod(modexp(curX, r, n), mu, n);
            curY = mulmod(modexp(mu, r, n), curY, n);
            curT = curT / 2;
            idx++;
        }

        if (curT == 1) {
            uint256 x2 = mulmod(curX, curX, n);
            return x2 == curY;
        } else {
            uint256 exp = 2 ** curT;
            uint256 computed = modexp(curX, exp, n);
            return computed == curY;
        }
    }

    function submitVDFProof(uint256 y, uint256[] calldata proof) external {
        require(block.number > lastCheckIn + requiredDelay, "too early");
        require(!inactivityConfirmed, "already confirmed");
        require(verifyVDF(currentChallenge, y, proof, T, N), "invalid proof");
        inactivityConfirmed = true;
        emit VDFSubmitted(y, msg.sender);
        emit InactivityConfirmed();
    }

    function isInactivityConfirmed() external view returns (bool) {
        return inactivityConfirmed;
    }
}
