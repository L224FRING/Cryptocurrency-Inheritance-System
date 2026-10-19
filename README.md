# Crypto Inheritance System

Threshold-signature inheritance vault: FROST (secp256k1) for key custody, VDF-backed liveness gating for inactivity detection, and threshold trustee attestation. The design is **threshold, not multisig** — there is one group key from distributed key generation (DKG). Each trustee holds a secret share; partial signatures aggregate into a single Schnorr signature. No single party ever holds the full private key.

## Layout

```
foundry.toml              Foundry config (solc 0.8.28, FFI enabled)
src/                      Solidity contracts
  VDFVerifier.sol         Pietrzak VDF verifier using MODEXP precompile
  FROSTVerifier.sol       FROST threshold signature verifier (on-chain)
  Secp256k1.sol           secp256k1 point math + FROST challenge (used by FROSTVerifier)
  InheritanceVault.sol    Main vault contract coordinating VDF + FROST
  Smoke.sol               toolchain smoke contract
script/DeployFull.s.sol   Canonical full-stack deployment (VDF+FROST+Vault)
script/DeployVDF.s.sol    Deploy VDF+Vault
script/Deploy.s.sol       Basic deploy for anvil
test/                     Foundry tests
test/fixtures/            Known-answer vectors from the Rust service
lib/forge-std/            Foundry test library
rust/frost-service/       Off-chain FROST crate (frost-core 3.0)
  src/lib.rs              Protocol core (party, wire, transport, coordinator, client, persistence)
  src/main.rs             frost-service CLI
  src/bin/relay.rs        frost-relay (untrusted message relay)
  tests/                  Integration tests
```

## Requirements

| Tool | Version |
| --- | --- |
| forge/cast/anvil | 1.8.3 |
| solc | 0.8.28 (via forge) |
| rustc/cargo | 1.92.0 |
| frost-core/frost-secp256k1 | 3.0.0 |

Add to PATH if needed:

```bash
export PATH="$PATH:$HOME/.foundry/bin:$HOME/.cargo/bin"
```

## Build & Test

```bash
cargo build --manifest-path rust/frost-service/Cargo.toml
cargo test  --manifest-path rust/frost-service/Cargo.toml  # 47 Rust tests
forge test                                                  # 58 Solidity tests
```

`forge test` shells out to `rust/frost-service/target/debug/frost-service`, so always rebuild Rust first before running Forge tests.

## FROST CLI

```bash
# In-process (convenient, all parties in one process)
./target/debug/frost-service selftest
./target/debug/frost-service dkg --trustees 5 --threshold 3
./target/debug/frost-service sign --participants 1,2,3
./target/debug/frost-service matrix

# Per-trustee (each trustee runs its own process, talks via relay)
./target/debug/frost-relay --listen 127.0.0.1:8477 --trustees 5

SESSION=$(python3 -c "import os; print(os.urandom(32).hex())")
mkdir -p /tmp/shares
for i in 1 2 3 4 5; do
  ./target/debug/frost-service dkg-party --index $i --trustees 5 --threshold 3 \
    --session $SESSION --relay http://127.0.0.1:8477 --out /tmp/shares/share-$i.json &
done
wait

SIGSESSION=$(python3 -c "import os; print(os.urandom(32).hex())")
./target/debug/frost-service sign-party --index 1 --share /tmp/shares/share-1.json \
  --participants 1,2,3 --session $SIGSESSION --relay http://127.0.0.1:8477 --aggregate &
./target/debug/frost-service sign-party --index 2 --share /tmp/shares/share-2.json \
  --participants 1,2,3 --session $SIGSESSION --relay http://127.0.0.1:8477 &
./target/debug/frost-service sign-party --index 3 --share /tmp/shares/share-3.json \
  --participants 1,2,3 --session $SIGSESSION --relay http://127.0.0.1:8477 &
wait
```

Key flags: `--out-dir DIR` (write all shares from in-process DKG), `--aggregate` (aggregator combines partials), `--timeout-ms N` (peer wait budget). `sign-party` takes `--message HEX` and `--raw-message`. All commands emit JSON to stdout.

By default a signature covers a canonical JSON binding `{domain, payload_hash, session}`. The vault, however, verifies the signature over its `releaseHash` bytes, so pass `--raw-message --message <releaseHash>` when signing for release (see `docs/Presentation_Demo_Flow.md`, step 7).

## Share files

Each trustee saves its share to `share-N.json`. It contains:

- `trustee_id`, `committee`, `threshold`
- `group_verifying_key` (hex checksum of the group public key)
- `key_package` (serialized `frost-secp256k1::KeyPackage`) — contains the secret share `F(i)`. **Keep secret.**
- `public_key_package` (serialized `frost-secp256k1::PublicKeyPackage`) — group/public verification material

During DKG, each party generates a random polynomial; the final secret share is the evaluation `F(i)` of the joint polynomial. The group secret `F(0)` is never reconstructed. `Party::from_share` verifies the group key matches, preventing file swaps.

## Cryptography summary

- **FROST-secp256k1** (frost-core 3.0.0, frost-secp256k1 3.0.0): DKG + 2-round threshold signing. Signature is a 65-byte compact Schnorr signature (130 hex). Nonces are single-use; envelopes are bound to session/domain/payload. Relay is untrusted; DKG round-2 is point-to-point.
- **VDF** (Pietrzak over RSA): Proof is generated off-chain and verified on-chain. Fiat–Shamir uses Keccak256 over four 32-byte big-endian words `(x,y,mu,n)`, matching `VDFVerifier` exactly. Modulus `N` is a fixed demo composite; production needs a properly generated RSA modulus with unknown factorization.

## On-chain

- `VDFVerifier`: Pietrzak verification (uses MODEXP). Rust and Solidity share identical challenge encoding.
- `FROSTVerifier`: Real threshold Schnorr verification on-chain. `Secp256k1.sol` implements secp256k1 point arithmetic (Jacobian add/double/mul, compressed-point decompression) and the FROST challenge `e = H2(R ‖ Z ‖ msg)` (RFC 9380 `expand_message_xmd` over SHA-256). Verification checks `s·G == R + e·Z`, plus group-key-on-curve, 65-byte compact signature, `0 < s < N` range, and replay protection. Cross-validated against a real Rust-produced signature in `test/fixtures/frost_kat.json`.
- `InheritanceVault`: Requires both VDF confirmation and threshold signature before releasing funds to beneficiary. Owner check-ins reset the inactivity window.

## Development notes

- `dkg`/`sign` run in-process (convenient). `dkg-party`/`sign-party` run each trustee in its own process with only its own share (true separation of duties).
- In per-trustee mode, the coordinator holds no shares. Peers must all participate or the waiting trustee times out with `Stalled`.
- `forge test` doesn't build Rust automatically — `cargo build` first.
- The trustees sign a canonical (domain/payload/session) message by default; `--raw-message` makes them sign the payload bytes verbatim, which is what the vault's `releaseHash` requires.
- On-chain EC math needs `via_ir = true` (set in `foundry.toml`).
- Tests: 47 Rust (including the KAT generator, per-trustee CLI, and threaded protocol tests) + 58 Solidity. All pass.
- For a live demo, see `docs/Presentation_Demo_Flow.md`. For design details, see `docs/FrostService.md`.

## Deployment

```bash
anvil
forge script script/DeployFull.s.sol --rpc-url http://127.0.0.1:8545 --broadcast --private-key $ANVIL_KEY
```

Capture `FROSTVerifier`, `VDFVerifier`, `InheritanceVault` addresses from broadcast output. Set the DKG group public key on `FROSTVerifier` via `setGroupPublicKeyCompressed`.