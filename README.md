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
  src/lib.rs              protocol core: party, wire, transport, coordinator
  src/main.rs             frost-service CLI (selftest, dkg, sign, matrix)
  src/bin/relay.rs        frost-relay, the untrusted message relay
  tests/protocol.rs       n-of-m, sub-threshold, replay, live-relay tests
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
cargo build --manifest-path rust/frost-service/Cargo.toml   # FROST binaries
cargo test  --manifest-path rust/frost-service/Cargo.toml   # 41 Rust tests
forge test                                                  # both Solidity suites
```

`forge test` shells out to `rust/frost-service/target/debug/frost-service`, so
build the Rust crate in debug mode first. `ffi = true` and the `fs_permissions`
entries in `foundry.toml` exist for that call. See the stale-binary footgun below.

Check both halves at once:

```sh
cargo build --manifest-path rust/frost-service/Cargo.toml && forge test
```

## Local testnet

```sh
anvil                                                   # terminal 1, chain id 31337
forge script script/Deploy.s.sol --rpc-url http://127.0.0.1:8545 --broadcast
```

`Deploy.s.sol` defaults to anvil's first account. Override with
`DEPLOYER_PRIVATE_KEY`.

## FROST service

`rust/frost-service` runs real multi-party FROST (secp256k1) over secp256k1-SHA256:
a three-part DKG across the committee, then two-round threshold signing over any
subset at or above the threshold.

Split into a protocol core and the things that move bytes for it:

| Module | Role |
| --- | --- |
| `party` | one trustee. Holds only its own key share, never the group secret. |
| `wire` | message format. Every message is bound to a 32-byte session id. |
| `transport` | carries messages. `memory` in-process, `http` over a relay. |
| `coordinator` | drives a session by pumping envelopes until quiet. |
| `error` / `report` | one error type, one JSON report shape. |

No Shamir-style reconstruction is involved: the group secret is never assembled,
at setup or at signing.

### Commands

```sh
frost-service selftest                  # in-process DKG + 3-of-5 signing check
frost-service dkg    --threshold 3      # just the DKG
frost-service sign   --participants 2,3,5
frost-service matrix                    # sweep n-of-m and sub-threshold subsets
frost-relay --listen 127.0.0.1:8477     # untrusted message relay
```

`--trustees`, `--threshold`, `--session`, `--domain`, `--relay`, `--message`,
`--participants`, `--expect-failure`. `frost-service --help` lists them.

Every subcommand writes one JSON document to stdout and nothing else. Exit code is
0 on success, non-zero on failure, so a caller can never mistake a partial report
for a good one. The DKG is randomized, so the group key and signature differ every
run.

### The relay is untrusted

`frost-relay` routes opaque bytes between trustee mailboxes and verifies nothing.
It cannot forge a sender, cannot read a DKG round-2 package meant for someone else
(those are point-to-point, never broadcast), and gains no information beyond
delivery: a relay is a worse courier, not a trusted participant. Correctness comes
from FROST's own package verification, not from trusting the transport.

```sh
frost-relay --listen 127.0.0.1:8477 --trustees 5   # terminal 1
frost-service sign --relay http://127.0.0.1:8477    # terminal 2
```

### Session ids are single-use

Every `Envelope` and every signed message is bound to a 32-byte session id, hashed
under a domain separator (`cis/frost/release-attestation/v1`) together with the
payload. A session id may be signed over **once**: a second run with the same id is
refused, so commitments from a finished run can never be recycled. A signature
verifies for exactly one (session, domain, payload) triple and no other.

Signing nonces are one-shot too: consumed before the work, with spent commitment
sets recorded, because reusing a nonce pair across two messages leaks a key share.

### What the tests cover

`rust/frost-service/tests/protocol.rs` checks the claims, not just the happy path:

- DKG converges on one group key across all trustees.
- Any subset at or above the threshold signs; signatures are verified independently
  with `frost` against the group key, not trusted from the crate's own bookkeeping.
- Sub-threshold subsets are refused across the full 2..5 matrix, and no single trustee
  can sign a 3-of-5 alone.
- A signature binds only to its own session, domain, and payload.
- Session reuse, duplicate commitments, duplicate shares, and nonce reuse are refused.
- The full protocol runs over a real relay process, and reset leaves no state behind.

`frost-service matrix` runs the same n-of-m sweep from the CLI and reports
`passed`/`failed` per case.

### Solidity FFI

`test/FfiBridge.t.sol` consumes the `selftest` report with `vm.parseJson*`. It used
to scrape `println!` output by matching the literal `"threshold             3-of-5"`,
which broke silently on any whitespace change. Renaming a field in the report now
fails the suite with a path error instead.

## Known footgun: stale Rust binary

`forge test` does not build the Rust crate. It shells out to
`rust/frost-service/target/debug/frost-service` as it exists on disk, so editing
`main.rs` and running `forge test` without an intervening `cargo build` tests the
*previous* binary and passes anyway. Always rebuild first:

```sh
cargo build --manifest-path rust/frost-service/Cargo.toml && forge test
```

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
- `Identifier` has no `Display` impl. `Trustee` in `main.rs` keeps the numeric
  index alongside the identifier so error messages can name a trustee.
- `keys::dkg::part1(.., rng)` and `round1::commit(.., rng)` take `&mut C::Rng`;
  in a loop that needs a reborrow (`&mut *rng`) or the borrow is moved.

## Not yet set up

Deliberately out of scope so far:

- VDF evaluation. Wesolowski over a class group or RSA group is the likely pick;
  `kilic/evmvdf` has a Solidity reference around 173k gas per verification.
  No EVM VDF precompile exists on Ethereum mainnet today.
- The actual inheritance contracts, trustee registry, and attestation flow.
- Real subcommands. `selftest` is the only one; DKG and signing are not
  independently invocable, so the report is generated and thrown away each run.
  A caller cannot yet ask for a signature over a message of its choosing, which
  is the shape the vault contracts will need.
- `k256`, `serde`, `serde_json`, and `sha2` are declared in `Cargo.toml`.
  `serde`/`serde_json` are now used; `k256` and `sha2` are still unused. A
  `message_digest` field (SHA-256 of the message) would be the natural way to
  give `sha2` a purpose and give contracts a stable value to bind an attestation
  to.
