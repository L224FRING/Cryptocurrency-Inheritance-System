import {
  encodePacked,
  keccak256,
  parseAbi,
  type Address,
  type Hex,
} from "viem";

/**
 * Human-readable ABIs for the three contracts the dApp talks to.
 * Keep these in sync with `src/InheritanceVault.sol`, `src/VDFVerifier.sol`
 * and `src/FROSTVerifier.sol`.
 */

export const vaultAbi = parseAbi([
  "function owner() view returns (address)",
  "function vdfVerifier() view returns (address)",
  "function frostVerifier() view returns (address)",
  "function beneficiary() view returns (address)",
  "function beneficiarySet() view returns (bool)",
  "function released() view returns (bool)",
  "function currentChallenge() view returns (bytes32)",
  "function getCurrentChallenge() view returns (bytes32)",
  "function releaseHash() view returns (bytes32)",
  "function usedSignatures(bytes32) view returns (bool)",
  "function lastCheckInBlock() view returns (uint256)",
  "function lastCheckInTime() view returns (uint256)",
  "function setBeneficiary(address _beneficiary)",
  "function checkIn()",
  "function release(bytes signature)",
]);

export const vdfAbi = parseAbi([
  "function N() view returns (uint256)",
  "function T() view returns (uint256)",
  "function currentChallenge() view returns (uint256)",
  "function getCurrentChallenge() view returns (uint256)",
  "function lastCheckIn() view returns (uint256)",
  "function requiredDelay() view returns (uint256)",
  "function inactivityConfirmed() view returns (bool)",
  "function isInactivityConfirmed() view returns (bool)",
  "function owner() view returns (address)",
  "function vault() view returns (address)",
  "function submitVDFProof(uint256 y, uint256[] proof)",
]);

export const frostAbi = parseAbi([
  "function owner() view returns (address)",
  "function hasGroupKey() view returns (bool)",
  "function groupPublicKeyX() view returns (uint256)",
  "function groupPublicKeyY() view returns (uint256)",
  "function groupKeyPrefix() view returns (uint8)",
  "function groupKeyIsCompressed() view returns (bool)",
  "function setGroupPublicKeyCompressed(bytes pubkey)",
  "function verifyFROSTSignature(bytes32 messageHash, bytes signature) view returns (bool)",
]);

/** Packed prefix used by `InheritanceVault.releaseHash` when hashing the message. */
export const RELEASE_DOMAIN = "inheritance-release";

/**
 * The exact bytes trustees must sign for `InheritanceVault.release`.
 *
 * Mirrors Solidity:
 * `keccak256(abi.encodePacked("inheritance-release", block.chainid, address(this), beneficiary, currentChallenge))`
 *
 * The chain id prevents a signature from being replayed on a fork or another
 * deployment of the same vault code.
 */
export function computeReleaseHash(
  vault: Address,
  beneficiary: Address,
  challenge: Hex,
  chainId: bigint,
): Hex {
  return keccak256(
    encodePacked(
      ["string", "uint256", "address", "address", "bytes32"],
      [RELEASE_DOMAIN, chainId, vault, beneficiary, challenge],
    ),
  );
}

/**
 * Compare a compressed group key returned by the FROST service with the key
 * stored in the verifier. The DKG key is `0x` + 33 bytes (prefix || x).
 */
export function groupKeyMatches(
  groupKey: Hex | undefined,
  storedX: bigint | undefined,
  storedPrefix: number | undefined,
): boolean {
  if (!groupKey || storedX === undefined || storedPrefix === undefined) {
    return false;
  }
  if (groupKey.length !== 66) {
    return false;
  }
  const prefix = Number.parseInt(groupKey.slice(2, 4), 16);
  const x = BigInt(`0x${groupKey.slice(4)}`);
  return prefix === storedPrefix && x === storedX;
}
