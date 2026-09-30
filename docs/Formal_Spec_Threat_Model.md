# Formal Specification & Threat Model

*Blockchain-Based Cryptocurrency Inheritance System (FROST + VDF)*

---

## 1. Target Chain Decision

The system is designed for deployment on an Ethereum-compatible Layer 2 (e.g. Arbitrum or Optimism), rather than Ethereum L1, primarily to keep on-chain VDF verification affordable.

- **EVM-equivalence:** Solidity contracts (VDF verifier, Schnorr verifier, release logic) require no changes to deploy on L2.
- **Gas cost:** on-chain VDF verification has been benchmarked at roughly 2 to 4 million gas depending on implementation; L2 fees are typically an order of magnitude or more cheaper than L1 for the same computation.
- **Development target:** an L2 testnet (e.g. Arbitrum Sepolia or Optimism Sepolia) is used for implementation and evaluation, avoiding real funds while preserving representative gas-cost behavior.

*VDF verification gas cost is treated as the single largest technical risk item and is benchmarked early in the implementation timeline rather than deferred to final integration.*

---

## 2. Actors

The system defines four roles. Three are held by human participants; the fourth (VDF proving/verification) is a system function with no trust assumption attached.

| Actor | Role | Holds |
|---|---|---|
| **Owner** | Original asset holder; sets up the will and performs periodic check-ins to reset the VDF-gated inactivity clock. | The assets themselves, in their own wallet (noncustodial design). Implicitly, the ability to prevent release by remaining active. |
| **Trustees** (n total, threshold t) | Jointly hold FROST key shares from a one-time distributed key generation (DKG). Participate in signing rounds for both the release signature and the death-attestation statement. | One FROST key share each. No single trustee holds anything usable alone; a sub-threshold subset cannot produce a valid signature. |
| **Beneficiaries** | Designated recipients of assets upon a valid, verified release. | Nothing until release. May be required to submit a claim proof, depending on final design choice. |
| **VDF Prover / Verifier** (system role, not a human party) | Not a trusted actor. Anyone may submit a VDF proof; the smart contract verifies it deterministically on-chain. | No custody or trust assumption. Removes the need for an oracle or trusted timekeeper. |

### 2.1 Trustee-Beneficiary Overlap

Whether a trustee may also be a named beneficiary is an explicit design decision. Permitting overlap increases collusion incentive, since a trustee-beneficiary benefits directly from a falsely triggered release. The system either disallows overlap outright, or requires a strictly higher signing threshold when overlap exists.

---

## 3. Threat Model

### 3.1 Assets to Protect

- Confidentiality of the owner's private key and of individual FROST key shares, before, during, and after release.
- Integrity of the release condition: release must fire only under legitimate death, never prematurely.
- Availability: a legitimate death must eventually result in release; the system must not be able to become permanently stuck.

### 3.2 Adversary Model

The adversary model follows the UC-style static/adaptive corruption framing used in *Dead Man's Switch Cryptography* (Banerjee, Bozhko, Heitjohann, and Rupp, IACR ePrint 2026/1352), which provides the formal baseline for dead-man's-switch security definitions adopted here. The adversary may corrupt up to t-1 trustees, where t is the signing threshold.

### 3.3 Threats and Mitigations

| Threat | Defense / Mitigation | Status |
|---|---|---|
| **Colluding minority of trustees** (< threshold t) | Cryptographically impossible to produce a valid signature or attestation. FROST's core security guarantee: a sub-threshold set of key shares carries no signing power, by construction. | *Solved (cryptographic)* |
| **Colluding threshold-or-more trustees, acting maliciously** (not merely prematurely) | No pure cryptographic defense exists once a legitimate threshold cooperates; this is a residual trust assumption inherent to any threshold scheme, not unique to this design. | *Residual / out of scope for cryptographic guarantees* |
| **Owner impersonation or stolen check-in credentials** | Check-ins must be signed with the owner's key. If that key is compromised, an attacker can stall a legitimate release indefinitely. Mitigation is key-hygiene, not protocol-level. | *Partially mitigated* |
| **VDF manipulation via specialized/faster-than-assumed hardware** | Standard VDF assumption: adversary sequential-compute bound must be stated explicitly and is inherited from Boneh et al. (2018), not unique to this design. | *Bounded by assumption* |
| **Replay attacks** (reusing a prior valid signature or attestation) | Bind a unique nonce or session identifier into every signed message (release signature and attestation) so no prior signature verifies in a new context. | *Mitigated by design* |
| **Oracle / attestation manipulation** | Attestation is itself a FROST-signed statement, so it inherits the same sub-threshold collusion resistance as the release signature. No external oracle is trusted. | *Solved (cryptographic)* |

---

## 4. Explicitly Out of Scope

- **Legal validity of the will across jurisdictions.** This remains an open, unsolved problem in the broader literature (Prost, 2022) and is not addressed by this project's cryptographic contribution.
- **Physical loss or destruction of a trustee's key share.** A resilience/operational concern rather than a cryptographic security concern; noted as potential future work.
- **General smart contract vulnerabilities unrelated to the cryptographic core** (e.g. reentrancy, access-control bugs). Standard audit territory, not the project's novel contribution.
