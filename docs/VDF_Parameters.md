# VDF Parameter Selection & Trusted Setup

## Recommended Parameters

For production deployment, the following parameters are recommended:

| Parameter | Value | Rationale |
|---|---|---|
| Modulus size (bits) | 2048 | Standard RSA security level; balances verification gas cost vs security |
| T (sequential squarings) | 10^6 - 10^7 | Calibrated to desired delay (e.g., 1 day requires benchmarking hardware) |
| Hash function for challenges | keccak256 | Matches the on-chain verifier's `abi.encodePacked` Fiat-Shamir derivation |

## Trusted Setup Considerations

The RSA modulus N must be of unknown factorization. Options:
1. **RSA UFO (Unfinished or Unfactored Objects)** - Use published moduli with unknown factors
2. **Multi-party computation ceremony** - Generate N collaboratively so no single party learns factors
3. **Academic/public ceremonies** - Leverage existing RSA setup ceremonies

This is the only trust assumption in the VDF component as documented in the formal spec.
