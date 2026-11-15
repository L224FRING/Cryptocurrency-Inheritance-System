# Evaluation Report

Empirical evaluation of the Cryptocurrency Inheritance System, covering the
metrics agreed for the report: threshold correctness, dual-gate enforcement,
per-operation gas, VDF verification cost vs. `T`, FROST latency, security
properties, and a comparison against a naive timer.

All numbers in this report were produced on this repository (base commit
`a99c9ac` plus the uncommitted `test/Evaluation.t.sol` harness) using the
commands in [Reproduction](#reproduction). Nothing here is estimated unless
explicitly labelled.

## 1. Environment & Method

| Item | Value |
|---|---|
| OS | Linux 7.1.9-arch1-2 |
| CPU | 11th Gen Intel Core i7-1185G7 @ 3.00 GHz (8 threads) |
| Rust | rustc 1.92.0 |
| Foundry | forge 1.8.3 |
| Solidity | 0.8.28, optimizer on, `optimizer_runs = 200`, `via_ir = true` |
| Python | 3.14.7 |

Gas figures are **execution gas** measured with `gasleft()` around the external
call: they exclude the 21,000 intrinsic transaction cost and calldata. Add those
to get the full transaction cost. Latency figures are medians of 5-7 runs.

Two measurement harnesses were used:

- `test/Evaluation.t.sol` (added for this report) for on-chain gas, dual-gate,
  and VDF verification scaling.
- The `frost-service` CLI for matrix, latency, and VDF prover timing.

## 2. Threshold Correctness — 18/18

`frost-service matrix` sweeps committees of size 2, 3, and 5 and, for each
threshold, exercises exactly-`t`, more-than-`t`, and `(t-1)`-of-`n` subsets.

```text
status: ok   passed: 18   failed: 0   trustees_swept: [2, 3, 5]
```

| n | t | Subset | Expected | Observed |
|---|---|---|---|---|
| 2 | 2 | [1,2] | signature verifies | signature verifies |
| 2 | 2 | [1] | rejected | rejected |
| 3 | 2 | [1,2] | signature verifies | signature verifies |
| 3 | 2 | [1,2,3] | signature verifies | signature verifies |
| 3 | 2 | [1] | rejected | rejected |
| 3 | 3 | [1,2,3] | signature verifies | signature verifies |
| 3 | 3 | [1,2] | rejected | rejected |
| 5 | 2 | [1,2] | signature verifies | signature verifies |
| 5 | 2 | [1,2,3,4,5] | signature verifies | signature verifies |
| 5 | 2 | [1] | rejected | rejected |
| 5 | 3 | [1,2,3] | signature verifies | signature verifies |
| 5 | 3 | [1,2,3,4,5] | signature verifies | signature verifies |
| 5 | 3 | [1,2] | rejected | rejected |
| 5 | 4 | [1,2,3,4] | signature verifies | signature verifies |
| 5 | 4 | [1,2,3,4,5] | signature verifies | signature verifies |
| 5 | 4 | [1,2,3] | rejected | rejected |
| 5 | 5 | [1,2,3,4,5] | signature verifies | signature verifies |
| 5 | 5 | [1,2,3,4] | rejected | rejected |

The default `selftest` independently confirms the headline case (3-of-5 signs,
2-of-5 refused) and reports a real signature over
`inheritance-release-attestation` that verifies against the group key.

**Result: 18/18 cases correct; every exactly-`t` and above-`t` subset signs, and
every `(t-1)` subset is rejected.**

Limitation: `matrix` exercises contiguous subsets starting at trustee 1, not all
`C(n, k)` combinations. The FROST scheme itself is subset-agnostic; this is a
sweep-coverage limitation, not a protocol one.

## 3. Dual-Gate Enforcement

Release requires **both** a confirmed VDF inactivity proof and a valid threshold
signature over the vault's release hash. `test/Evaluation.t.sol` exercises each
path:

| Scenario | Expected | Result |
|---|---|---|
| VDF not confirmed, valid-looking signature | revert `vdf not confirmed` | pass |
| VDF confirmed, wrong signature | revert `invalid signature` | pass |
| VDF confirmed + signature over release hash | release succeeds | pass |
| VDF proof submitted before `requiredDelay` | revert `too early` | pass |
| Proof submitted with caller-chosen `x/t/n` | cannot: only `submitVDFProof(y, proof)` exists | pass |
| Release signature replay after a release | revert `already released` / `signature already used` | pass |
| Release called twice | revert `already released` | pass |

The successful-path test derives the release hash inside the vault as
`keccak256(abi.encodePacked("inheritance-release", block.chainid, vault,
beneficiary, currentChallenge))`, has a fresh 3-of-5 FROST committee sign exactly
those bytes (`--raw-message`), installs that group key, confirms inactivity, and
releases 1 ETH to the beneficiary.

**Result: the gate is conjunctive — either factor missing blocks release; both
present releases funds.**

## 4. Gas Cost per Operation

Measured in `test/Evaluation.t.sol` (execution gas, excludes 21k intrinsic).

| Operation | Gas | Notes |
|---|---:|---|
| `VDFVerifier.checkIn()` | 35,076 | standalone reset of liveness clock + challenge |
| `InheritanceVault.checkIn()` | 54,610 | one tx: vault challenge + timestamps + `vdfVerifier.checkIn()` |
| `VDFVerifier.submitVDFProof(y, proof)` | 63,270 | `T = 100`, 7 proof points |
| `FROSTVerifier.verifyFROSTSignature(hash, sig)` | 1,173,390 | stateless `view`; secp256k1 scalar mult in EVM |
| `InheritanceVault.release(sig)` | ~1.25M | FROST verify + replay write + effects + 1 ETH transfer |

Observations:

- The owner now performs one check-in transaction instead of two; it costs
  ~55k because it also resets the VDF clock.
- VDF proof submission is ~63k at `T = 100`; about 23k of that is the verifier
  loop (see §5), the rest is timing/storage/event overhead.
- `release()` is dominated by on-chain secp256k1 verification (~1.225M for the
  fixed KAT signature). Because the double-and-add loop branches on scalar bits,
  gas varies a few percent per signature (measured 1.18-1.27M over 8 runs). This
  is the price of not trusting an off-chain oracle or ECDSA precompile: the EVM
  has no secp256k1 Schnorr precompile, so the curve math is done in Solidity.
- The high release cost is paid **once per inheritance**, not per check-in, so
  its amortized impact is small relative to the safety it buys.

## 5. VDF Verification Gas vs. `T`, and Prover Time

### 5.1 Verification gas grows ~log2(T)

Each Pietrzak round halves `T`, so the number of proof points is
`ceil(log2(T))`-ish and verification is `O(log T)` modexps.
`verifyVDF` gas, measured on-chain:

| `T` | log2(`T`) | Proof points | Verify gas |
|---:|---:|---:|---:|
| 8 | 3.00 | 3 | 13,533 |
| 16 | 4.00 | 4 | 16,013 |
| 32 | 5.00 | 5 | 18,450 |
| 64 | 6.00 | 6 | 20,918 |
| 128 | 7.00 | 7 | 23,412 |
| 256 | 8.00 | 8 | 25,857 |
| 512 | 9.00 | 9 | 28,377 |
| 1024 | 10.00 | 10 | 30,875 |

A linear fit over the table gives roughly:

```text
verify_gas ≈ 6,100 + 2,480 · log2(T)
```

i.e. each doubling of `T` adds ~2.5k gas. This is the expected logarithmic
verification cost: work is moved off-chain into the prover, and the chain only
checks `O(log T)` midpoints.

### 5.2 Prover time grows linearly in `T`

Sequential squarings dominate the off-chain prover. With the same modulus:

| `T` | Proof points | Median prover time |
|---:|---:|---:|
| 1,000 | 10 | 9.2 ms |
| 10,000 | 14 | 44.7 ms |
| 100,000 | 17 | 343.6 ms |
| 1,000,000 | 20 | 3,292.6 ms |

Normalising for fixed process start-up, the marginal cost converges to about
**3.3 ms per 1,000 squarings**, i.e. prover time is linear in `T`
(`10×` squarings → ~`10×` time at large `T`), while verifier gas is logarithmic.

**Caveat:** the repository's test modulus is
`N = 0x80007fff123368ac5678 ≈ 2^79`, so absolute modexp gas is far below a
production 2048-bit RSA modulus. The **shape** of both curves
(`O(log T)` verify, `O(T)` prove) is independent of modulus size, but production
per-round gas will be larger. See `docs/VDF_Parameters.md`.

## 6. FROST Latency

In-process (`MemoryTransport`) DKG + signing, debug build, medians. "Signing
only" is `sign` median minus `dkg` median (both include process start-up, so it
is an estimate).

| t / n | DKG (ms) | DKG + sign (ms) | Signing only (ms) | DKG envelopes | Cumulative envelopes |
|---|---:|---:|---:|---:|---:|
| 2/2 | 18.45 | 37.71 | 19.26 | 4 | 8 |
| 2/3 | 38.96 | 59.29 | 20.33 | 12 | 20 |
| 2/5 | 108.35 | 128.62 | 20.27 | 40 | 56 |
| 3/3 | 50.44 | 79.27 | 28.83 | 12 | 24 |
| 3/5 | 138.84 | 168.61 | 29.77 | 40 | 64 |
| 4/5 | 167.81 | 211.42 | 43.60 | 40 | 72 |
| 5/5 | 199.58 | 253.49 | 53.91 | 40 | 80 |

Observations:

- "Cumulative envelopes" is the `sign` run's transport counter, which includes
  the DKG that `sign` performs internally (difference from the DKG column is the
  signing-only traffic).
- DKG cost is driven by committee size `n` (all-to-all verification rounds); it
  is essentially independent of `t` in envelope count.
- Signing cost grows with the number of participants `t` (commitments and
  partial-signature fan-out), roughly ~10-15 ms per additional signer here.
- These are single-machine, in-process numbers. The real `--relay` path adds
  network round trips and process scheduling; expect higher wall-clock latency
  but the same scaling in `t` and `n`.

## 7. Security Tests

| Property | Test | Result |
|---|---|---|
| Replay/griefing of a release signature prevented | `EdgeCases.test_FrostVerifierIsStateless`, `FROSTOnchain.test_VerificationIsStateless`, `Evaluation.test_Security_DoubleReleaseRejected` | pass |
| Public cannot reset the liveness clock | `VDF.test_OnlyOwnerOrBoundVaultCanCheckIn`, `VDF.test_BindVaultIsOneShot` | pass |
| VDF bypass via caller-chosen params removed | `VDFReal.test_SubmitProofRejectsInvalidProof`, `test_SubmitProofRejectsEarlySubmission` | pass |
| FROST group key cannot be rotated | `FROSTOnchain.test_GroupKeyIsOneShot`, `EdgeCases.test_GroupKeyCannotBeRotated` | pass |
| Early VDF proof rejected | `Evaluation.test_Security_EarlyProofRejected` | pass |
| Release without VDF rejected | `Evaluation.test_DualGate_RequiresVDF` | pass |
| Release without valid signature rejected | `Evaluation.test_DualGate_RequiresSignature` | pass |
| Double release rejected | `Evaluation.test_Security_DoubleReleaseRejected` | pass |
| Sub-threshold collusion rejected | `matrix` (18/18), `selftest` (2-of-3), `FfiBridge.test_BelowThresholdSubsetRejected` | pass |
| Tampered VDF proof / wrong output rejected | `VDFRealProof.test_TamperedProofRejected`, `test_WrongOutputRejected` | pass |
| Tampered FROST signature (scalar / R / prefix) rejected | `FROSTOnchain.test_TamperedScalarFails`, `test_TamperedRXFails`, `test_FlippedRPrefixFails` | pass |
| Non-owner cannot check in / set beneficiary / set key | `EdgeCases`, `FROSTOnchain` | pass |
| Malformed signature (length, prefix, scalar range) rejected | `FROSTOnchain` | pass |

Test suite totals at this commit: **77 Solidity tests** (11 suites) and
**47 Rust tests**, all passing, plus the 18-case matrix.

## 8. Comparison: This System vs. a Naive Timer

A "naive timer" is the obvious baseline: a contract that releases when
`block.timestamp > lastCheckIn + delay`.

| Dimension | Naive timer | This system |
|---|---|---|
| Release trigger | Wall-clock since last owner tx | VDF-proven sequential time since last check-in **and** a `t`-of-`n` signature |
| Can elapsed time be faked/accelerated? | Timestamp is validator-influenced and there is no proof of real work | VDF output cannot be computed faster than `T` sequential squarings; challenge binds to the check-in so it cannot be precomputed |
| Effect of one missed check-in | Funds release immediately at the deadline | Still requires `requiredDelay`, threshold trustees, and a valid proof |
| Single point of failure | Owner key / owner liveness | Owner key for liveness, but release needs `t` independent trustees; no single trustee can release |
| Premature-release risk (travel, hospital) | High | Mitigated by configurable delay and reset on check-in |
| On-chain cost | check-in ~21-40k, release tens of k | one check-in ~55k, VDF submit ~63k, release ~1.25M |
| Off-chain cost | None | VDF prover `O(T)`; FROST DKG + signing |
| Trust assumptions | Chain + owner | Chain + owner + threshold trustees + unknown-factorization RSA modulus (trusted setup) |
| Audit / implementation complexity | Low | High (EC math, hash-to-scalar, VDF, DKG, relay) |
| Privacy | Release timing observable | Release timing similarly observable once check-ins stop |

*Naive-timer gas ranges in this table are illustrative typical values, not
measured in this repository; the "this system" column is measured (§4).*

**Summary:** the naive timer is cheaper and simpler, but it trades away exactly
the properties an inheritance system needs: resistance to false alarms, no
single party able to move funds, and a release condition that cannot be
accelerated. This system pays a one-time ~1.25M gas release cost and off-chain
prover work for those properties.

## 9. Threats to Validity

- Absolute VDF gas uses the ~80-bit test modulus, not production 2048-bit RSA;
  verify-gas constants will be higher in production. Scaling trends hold.
- FROST latency is in-process, debug build, single machine; the relay path and
  optimized builds differ in absolute terms.
- The matrix sweeps contiguous subsets only (see §2).
- Gas excludes 21k intrinsic and calldata; add them for full tx cost.
- Timings are medians of few runs on a laptop-class CPU; treat them as
  order-of-magnitude and scaling evidence, not benchmarks of a specific server.

## 10. Reproduction

```bash
# Build
cargo build --manifest-path rust/frost-service/Cargo.toml

# Threshold correctness (18 cases)
./rust/frost-service/target/debug/frost-service matrix

# Selftest (3-of-5 sign, 2-of-5 reject)
./rust/frost-service/target/debug/frost-service selftest

# Gas, dual-gate, VDF scaling (prints GAS/T=... lines with -vv)
forge test --match-contract EvaluationTest -vv

# Full suites
forge test
cargo test --manifest-path rust/frost-service/Cargo.toml

# FROST latency / VDF prover timing
#   (see the tables above; loop the CLI over (t,n) and T, timing medians)
./rust/frost-service/target/debug/frost-service dkg  --trustees 5 --threshold 3
./rust/frost-service/target/debug/frost-service sign --trustees 5 --threshold 3 --participants 1,2,3
./rust/frost-service/target/debug/frost-service vdf --t 10000 --n 0x80007fff123368ac5678 --input 0x1234
```
