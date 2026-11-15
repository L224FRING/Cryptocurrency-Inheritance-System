// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {VDFVerifier} from "./VDFVerifier.sol";
import {FROSTVerifier} from "./FROSTVerifier.sol";

contract InheritanceVault {
    VDFVerifier public immutable vdfVerifier;
    FROSTVerifier public immutable frostVerifier;
    address public immutable owner;
    address public beneficiary;

    uint256 public lastCheckInBlock;
    uint256 public lastCheckInTime;
    bytes32 public currentChallenge;
    bool public released;
    bool public beneficiarySet;

    /// @notice Release signatures consumed by this vault. Keeping replay state
    /// here, instead of in the verifier, stops anyone from griefing a valid
    /// signature by front-running `verifyFROSTSignature`.
    mapping(bytes32 => bool) public usedSignatures;

    event CheckedIn(address indexed owner, uint256 blockNumber, uint256 timestamp, bytes32 challenge);
    event BeneficiarySet(address indexed beneficiary);
    event Released(address indexed beneficiary, uint256 timestamp);

    modifier onlyOwner() {
        require(msg.sender == owner, "not owner");
        _;
    }

    constructor(address _vdfVerifier, address _frostVerifier) {
        require(_vdfVerifier != address(0), "zero vdf address");
        require(_frostVerifier != address(0), "zero frost address");
        vdfVerifier = VDFVerifier(_vdfVerifier);
        frostVerifier = FROSTVerifier(_frostVerifier);
        owner = msg.sender;
        lastCheckInBlock = block.number;
        lastCheckInTime = block.timestamp;
        currentChallenge = keccak256(abi.encodePacked(_prevBlockHash(), block.timestamp, msg.sender));
        released = false;
        beneficiarySet = false;
    }

    /// @dev blockhash(block.number - 1) underflows at genesis; guard it.
    function _prevBlockHash() internal view returns (bytes32) {
        return block.number > 0 ? blockhash(block.number - 1) : bytes32(0);
    }

    /// @notice One owner action resets both the vault challenge and the VDF
    /// liveness clock, so the two clocks can never drift apart.
    function checkIn() external onlyOwner {
        require(!released, "already released");
        lastCheckInBlock = block.number;
        lastCheckInTime = block.timestamp;
        currentChallenge = keccak256(abi.encodePacked(
            _prevBlockHash(),
            block.timestamp,
            msg.sender,
            lastCheckInBlock
        ));
        vdfVerifier.checkIn();
        emit CheckedIn(owner, lastCheckInBlock, lastCheckInTime, currentChallenge);
    }

    function setBeneficiary(address _beneficiary) external onlyOwner {
        require(!released, "already released");
        require(_beneficiary != address(0), "zero address");
        require(!beneficiarySet || beneficiary != _beneficiary, "already set");
        beneficiary = _beneficiary;
        beneficiarySet = true;
        emit BeneficiarySet(_beneficiary);
    }

    /// @notice The exact digest trustees sign. Includes the chain id so a
    /// signature cannot be replayed on a fork or a different deployment.
    function releaseHash() public view returns (bytes32) {
        return keccak256(abi.encodePacked(
            "inheritance-release",
            block.chainid,
            address(this),
            beneficiary,
            currentChallenge
        ));
    }

    function release(bytes calldata signature) external {
        require(!released, "already released");
        require(beneficiarySet, "beneficiary not set");
        require(vdfVerifier.isInactivityConfirmed(), "vdf not confirmed");

        bytes32 digest = releaseHash();
        bytes32 signatureId = keccak256(abi.encodePacked(digest, signature));
        require(!usedSignatures[signatureId], "signature already used");

        // The verifier is a pure/view check now: consuming a signature is this
        // vault's job, after it has decided to release.
        require(frostVerifier.verifyFROSTSignature(digest, signature), "invalid signature");
        usedSignatures[signatureId] = true;

        // Effects before interaction (reentrancy-safe).
        released = true;
        emit Released(beneficiary, block.timestamp);

        // Forward any native assets held by the vault to the beneficiary.
        uint256 balance = address(this).balance;
        if (balance > 0) {
            (bool ok, ) = payable(beneficiary).call{value: balance}("");
            require(ok, "transfer failed");
        }
    }

    /// @notice The vault may hold native assets that are only releasable once
    /// both the VDF inactivity condition and the FROST threshold signature are met.
    receive() external payable {}

    function getCurrentChallenge() external view returns (bytes32) {
        return currentChallenge;
    }

    function isReleased() external view returns (bool) {
        return released;
    }
}
