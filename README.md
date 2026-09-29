# Crypto Inheritance System — Dev Environment

Toolchain for a threshold-signature inheritance vault: FROST (secp256k1) for key
custody, VDF-backed liveness gating for inactivity detection, threshold trustee
attestation on top of the VDF timer.

## Layout

```
foundry.toml              Foundry config (solc 0.8.28, FFI enabled)
src/                      Solidity contracts
  Smoke.sol               placeholder, delete once real contracts land
script/Deploy.s.sol       deploy script for anvil
test/                     Foundry tests
  Smoke.t.sol             toolchain sanity check
  FfiBridge.t.sol         proves the Solidity -> Rust boundary works
lib/forge-std/            Foundry test library
rust/frost-service/       off-chain FROST crate (frost-core 3.0)
```

## Prerequisites

Toolchain is installed. Add to your shell if the paths are not already there:

```sh
export PATH="$PATH:$HOME/.foundry/bin:$HOME/.cargo/bin"
```

Verified versions:

| Tool | Version |
| --- | --- |
| forge / cast / anvil | 1.8.3 |
| solc | 0.8.28 (downloaded by forge) |
| rustc / cargo | 1.92.0 |
| frost-core / frost-secp256k1 | 3.0.0 |

## Build and test

```sh
cargo build --manifest-path rust/frost-service/Cargo.toml   # FROST binary
forge test                                                  # both test suites
```

`forge test` shells out to `rust/frost-service/target/debug/frost-service`, so
build the Rust crate in debug mode first. `ffi = true` and the `fs_permissions`
entries in `foundry.toml` exist for that call.

## Local testnet

```sh
anvil                                                   # terminal 1, chain id 31337
forge script script/Deploy.s.sol --rpc-url http://127.0.0.1:8545 --broadcast
```

`Deploy.s.sol` defaults to anvil's first account. Override with
`DEPLOYER_PRIVATE_KEY`.

## FROST service (current state)

`rust/frost-service` is a smoke test of the library, not the final service. It
runs a 3-of-5 DKG with `keys::dkg::part1/part2/part3`, produces a signature from a
3-signer subset, and confirms a 2-signer subset is rejected. Sample output:

```
trustees              5
threshold             3-of-5
group verifying key   03702a01567f6db0cf7b121c8fc7f8f180ad6db9468dabf7676cdc7c33ae273374
verified against vkey ok
below-threshold sign   rejected: Incorrect number of commitments.
```

No Shamir-style reconstruction is involved: the group secret is never assembled,
including during signing.

## Notes on frost-core 3.0

The 3.0 API differs from 2.x in ways that matter here:

- `SigningPackage::new` takes `BTreeMap<Identifier, SigningCommitments>`, not a
  `SigningCommitments` struct.
- `round1::commit` takes a `&SigningShare`, with no identifier argument.
- `round2::sign(signing_package, signing_nonces, key_package)` — the signing
  nonces, not the raw commitments, are passed.
- `aggregate` takes a `PublicKeyPackage`, not a `VerifyingKey`.
- `verify_signature_share(identifier, verifying_share, share, signing_package, verifying_key)` — five args.
- `VerifyingKey::verify(&self, msg, &signature)` is a method; `serialize()` returns `Result`.
- `SigningPackage` is exported at the crate root as `frostk::SigningPackage`.

## Not yet set up

Deliberately out of scope so far:

- VDF evaluation. Wesolowski over a class group or RSA group is the likely pick;
  `kilic/evmvdf` has a Solidity reference around 173k gas per verification.
  No EVM VDF precompile exists on Ethereum mainnet today.
- The actual inheritance contracts, trustee registry, and attestation flow.
- Serialization format for the Foundry/Rust boundary. The FFI bridge currently
  scrapes human-readable stdout; real work needs structured JSON.
