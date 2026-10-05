# Crypto Inheritance System

Threshold-signature inheritance vault: FROST (secp256k1) for key custody, VDF-backed liveness gating for inactivity detection, and threshold trustee attestation. The design is **threshold, not multisig** — there is one group key from distributed key generation (DKG). Each trustee holds a secret share; partial signatures aggregate into a single Schnorr signature. No single party ever holds the full private key.

## Layout

```
foundry.toml              Foundry config (solc 0.8.28, FFI enabled)
src/                      Solidity contracts
  VDFVerifier.sol         Pietrzak VDF verifier using MODEXP precompile
  FROSTVerifier.sol       FROST threshold signature verifier (on-chain)
  InheritanceVault.sol    Main vault contract coordinating VDF + FROST
  Smoke.sol               placeholder
script/DeployFull.s.sol   Canonical full-stack deployment (VDF+FROST+Vault)
script/DeployVDF.s.sol    Deploy VDF+Vault
script/Deploy.s.sol       Basic deploy for anvil
test/                     Foundry tests
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
cargo test  --manifest-path rust/frost-service/Cargo.toml  # 46 Rust tests
forge test                                                  # 26 Solidity tests
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

Key flags: `--out-dir DIR` (write all shares from in-process DKG), `--aggregate` (aggregator combines partials), `--timeout-ms N` (peer wait budget). All commands emit JSON to stdout.

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
- `FROSTVerifier`: Structural checks (group key set, well-formed signature, replay protection). Full secp256k1 Schnorr point verification is a future enhancement.
- `InheritanceVault`: Requires both VDF confirmation and threshold signature before releasing funds to beneficiary. Owner check-ins reset the inactivity window.

## Development notes

- `dkg`/`sign` run in-process (convenient). `dkg-party`/`sign-party` run each trustee in its own process with only its own share (true separation of duties).
- In per-trustee mode, the coordinator holds no shares. Peers must all participate or the waiting trustee times out with `Stalled`.
- `forge test` doesn't build Rust automatically — `cargo build` first.
- Tests: 46 Rust (including per-trustee CLI + threaded tests) + 26 Solidity. All pass.
- For a live demo, see `docs/Presentation_Demo_Flow.md`. For design details, see `docs/FrostService.md`.

## Deployment

```bash
anvil
forge script script/DeployFull.s.sol --rpc-url http://127.0.0.1:8545 --broadcast --private-key $ANVIL_KEY
```

Capture `FROSTVerifier`, `VDFVerifier`, `InheritanceVault` addresses from broadcast output. Set the DKG group public key on `FROSTVerifier` via `setGroupPublicKeyCompressed`.