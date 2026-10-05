# Live Presentation Demo Flow

## Goal
Show the full end-to-end Cryptocurrency Inheritance System flow from both the Owner and Trustee perspectives. This is designed to be run live during a presentation with clear, actionable steps.

## Prerequisites

- Terminal windows (recommend 4-5 for clarity)
- Foundry installed (`forge`, `cast`, `anvil`)
- Rust toolchain
- Repo at: `/Users/somamacbook/Cryptocurrency-Inheritance-System`

Ensure PATH is set:
```bash
export PATH="$PATH:$HOME/.foundry/bin:$HOME/.cargo/bin"
```

---

## 1. Quick Sanity Check (30 seconds)

Before starting the full flow, quickly demonstrate the core cryptography works.

```bash
cd /Users/somamacbook/Cryptocurrency-Inheritance-System

# Show FROST 3-of-5 DKG + signing in one go
./rust/frost-service/target/debug/frost-service selftest
```

**Point to highlight:** Returns `status: ok`, shows 3 trustees can sign, 2 cannot (below threshold). Group verifying key is generated with no single party holding the full private key.

```bash
# Show all threshold combinations work correctly
./rust/frost-service/target/debug/frost-service matrix
```

**Point to highlight:** 18 test cases, all pass - threshold enforcement is correct.

```bash
# Show VDF computation and verification
./rust/frost-service/target/debug/frost-service vdf selftest
```

**Point to highlight:** VDF proves elapsed time via sequential squaring - output is verifiable with fast proof.

---

## 2. Full Multi-Party DKG (Owner Sets Up Trustees)

### Terminal 1: Start Relay (Untrusted Message Broker)

The relay routes encrypted messages between trustees. It sees envelopes, not secrets.

```bash
./rust/frost-service/target/debug/frost-relay --listen 127.0.0.1:8477 --trustees 5
```

### Terminal 2: Distributed DKG — each trustee is its own process

Each trustee runs its own client, reads and writes only its own share file, and
talks to the relay. Five processes, five shares.

```bash
export RELAY=http://127.0.0.1:8477
export SESSION=$(python3 -c "import os;print(os.urandom(32).hex())")
mkdir -p /tmp/shares

for i in 1 2 3 4 5; do
  ./rust/frost-service/target/debug/frost-service dkg-party \
    --index $i --trustees 5 --threshold 3 \
    --session $SESSION --relay $RELAY \
    --out /tmp/shares/share-$i.json &
done
wait

# Every trustee must have derived the same group key.
for i in 1 2 3 4 5; do
  python3 -c "import json;print(json.load(open('/tmp/shares/share-$i.json'))['group_verifying_key'])"
done
```

**Point to highlight:** every line is a separate OS process holding only its own
secret share. The relay is the only thing they share, and it sees ciphertext-like
envelopes, never secrets.

> **Shortcut (less impressive, used in tests):** `dkg --out-dir /tmp/shares`
> runs the whole ceremony in one process and writes one file per trustee.

**Key point:** At the end, each trustee has their own secret share (its file).
The owner holds only the group public key.

```bash
export GROUP_PUBKEY=$(python3 -c "import json;print(json.load(open('/tmp/shares/share-1.json'))['group_verifying_key'])")
```

---

## 3. Deploy Smart Contracts (Owner Deploys On-Chain)

### Terminal 3: Start Local Blockchain

```bash
anvil
```

Note the first account (`0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266`) - this is the **Owner**. Second (`0x70997970C51812dc3A010C7d01b50e0d17dc79C8`) - **Beneficiary**.

### Terminal 4: Deploy & Configure

Set variables from previous step.

```bash
# Anvil's first two deterministic accounts
export OWNER=0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
export BENEFICIARY=0x70997970C51812dc3A010C7d01b50e0d17dc79C8
# Anvil's first account private key (used for signing txs)
export ANVIL_KEY=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80
# The group key produced by the distributed DKG in Terminal 2.
export GROUP_PUBKEY=$(python3 -c "import json;print(json.load(open('/tmp/shares/share-1.json'))['group_verifying_key'])")

# Deploy full stack (VDFVerifier + FROSTVerifier + InheritanceVault)
forge script script/DeployFull.s.sol \
  --rpc-url http://127.0.0.1:8545 \
  --broadcast \
  --private-key $ANVIL_KEY
```

> **Note:** `cast` cannot read a private key from just `--from <address>`. With a local
> Anvil node, use `--unlocked --from <address>` (Anvil keeps the account unlocked),
> or pass `--private-key $ANVIL_KEY`. All `cast send` commands below use `--unlocked`.

From broadcast output, capture addresses:
- `FROSTVerifier`
- `VDFVerifier` 
- `InheritanceVault`

Set them:
```bash
export FROST_ADDR=<FROSTVerifier address>
export VDF_ADDR=<VDFVerifier address>
export VAULT_ADDR=<InheritanceVault address>
```

**Register group public key on-chain (Owner only):**

```bash
cast send $FROST_ADDR "setGroupPublicKeyCompressed(bytes)" $GROUP_PUBKEY \
  --rpc-url http://127.0.0.1:8545 \
  --unlocked --from $OWNER
```

**Point to highlight:** The group public key (from distributed DKG) is stored on-chain. No private key is ever deployed.

---

## 4. Fund Vault & Set Beneficiary (Owner Preparation)

```bash
# Set beneficiary
cast send $VAULT_ADDR "setBeneficiary(address)" $BENEFICIARY \
  --rpc-url http://127.0.0.1:8545 \
  --unlocked --from $OWNER

# Fund vault with 10 ETH
cast send $VAULT_ADDR --value 10ether \
  --rpc-url http://127.0.0.1:8545 \
  --unlocked --from $OWNER

# Verify
cast call $VAULT_ADDR "beneficiarySet()(bool)" --rpc-url http://127.0.0.1:8545
cast balance $VAULT_ADDR --rpc-url http://127.0.0.1:8545
```

**Point to highlight:** Assets stay in the vault contract. Owner retains control via check-ins.

---

## 5. Normal Operation: Periodic Check-Ins (Owner Alive)

This is what happens while the owner is alive and well.

```bash
# Owner checks in to reset inactivity timer
cast send $VAULT_ADDR "checkIn()" \
  --rpc-url http://127.0.0.1:8545 \
  --unlocked --from $OWNER
```

**Point to highlight:** As long as the owner checks in regularly, the VDF timer never completes. No trustee action is ever triggered. This is passive protection.

You can repeat this to demonstrate it's safe and doesn't move funds.

---

## 6. Inactivity Scenario: Compute & Submit VDF Proof

Now simulate the owner becoming inactive. The VDF chain-verifier has its own
challenge (separate from the vault's), so read it from `VDF_ADDR`.

**Get the VDF challenge and params:**

```bash
CHALLENGE=$(cast call $VDF_ADDR "getCurrentChallenge()(uint256)" --rpc-url http://127.0.0.1:8545 | awk '{print $1}')
echo "Challenge: $CHALLENGE"
T_VAL=$(cast call $VDF_ADDR "T()(uint256)" --rpc-url http://127.0.0.1:8545)
DELAY=$(cast call $VDF_ADDR "requiredDelay()(uint256)" --rpc-url http://127.0.0.1:8545)
echo "T: $T_VAL  requiredDelay: $DELAY"
```

**Compute the VDF off-chain (anyone can run this):**

```bash
# challenge as hex (no 0x prefix is also accepted)
CHEX=$(python3 -c "print(hex($CHALLENGE)[2:])")
./rust/frost-service/target/debug/frost-service vdf --t $T_VAL --input 0x$CHEX > /tmp/vdf.json
cat /tmp/vdf.json
```

Extract `y` and the `proof_points` as a Solidity `uint256[]`:

```bash
Y=$(python3 -c "import json;print(int(json.load(open('/tmp/vdf.json'))['y'],16))")
PROOF=$(python3 -c "import json;d=json.load(open('/tmp/vdf.json'));print('['+','.join(str(int(p,16)) for p in d['proof_points'])+']')")
echo "y=$Y"; echo "proof=$PROOF"
```

**Advance blocks past the required delay** (each `anvil_mine` argument is a hex
block count; the contract requires `block.number > lastCheckIn + requiredDelay`):

```bash
cast rpc anvil_mine 0x10 --rpc-url http://127.0.0.1:8545
```

**Submit the proof on-chain (permissionless - anyone can do this):**

```bash
cast send $VDF_ADDR "submitVDFProof(uint256,uint256[])" $Y "$PROOF" \
  --rpc-url http://127.0.0.1:8545 \
  --unlocked --from $BENEFICIARY   # any account may submit
```

**Verify:**

```bash
cast call $VDF_ADDR "isInactivityConfirmed()(bool)" --rpc-url http://127.0.0.1:8545
# returns: true
```

**Point to highlight:** The proof is self-verifying on-chain (Pietrzak VDF checked
via the `MODEXP` precompile). No trusted party is needed to submit it, and the
sequential computation is what proves real time passed.

**False-alarm demo (optional):** Before submitting, call
`cast send $VAULT_ADDR "checkIn()" --unlocked --from $OWNER` to show the owner
returning resets the clock so the release never happens.

---

## 7. Trustee Perspective: Threshold Signing for Release

Trustees observe that inactivity is confirmed and independently decide whether to
attest. Only if they conclude death/incapacity do they proceed.

### Step 7a: Trustees attest (off-chain human decision)

```bash
./rust/frost-service/target/debug/frost-service attest --message "death-confirmed"
```

**Point to highlight:** this represents the trustee's own real-world diligence.
The protocol cannot replace human judgment.

### Step 7b: Trustees produce a threshold FROST signature (each in its own process)

FROST is a **threshold** scheme, not a multisig: there is one group key, and each
trustee holds a *share* of it. Trustees never hold a personal signing key. Each
produces a **partial signature** from its own share, and the partials are
aggregated into a single ordinary Schnorr signature. Fewer than `threshold`
shares cannot produce one.

Each trustee loads only its own share file from the DKG (Step 2) and runs its own
process:

```bash
export RELAY=http://127.0.0.1:8477
export SIGSESSION=$(python3 -c "import os;print(os.urandom(32).hex())")

# Trustee 1 is the aggregator: it contributes a partial AND combines the rest.
./rust/frost-service/target/debug/frost-service sign-party \
  --index 1 --share /tmp/shares/share-1.json \
  --participants 1,2,3 --session $SIGSESSION \
  --relay $RELAY --aggregate > /tmp/sign1.json &

# Trustees 2 and 3 each contribute a partial, then exit.
./rust/frost-service/target/debug/frost-service sign-party \
  --index 2 --share /tmp/shares/share-2.json \
  --participants 1,2,3 --session $SIGSESSION --relay $RELAY &

./rust/frost-service/target/debug/frost-service sign-party \
  --index 3 --share /tmp/shares/share-3.json \
  --participants 1,2,3 --session $SIGSESSION --relay $RELAY &
wait

# The single aggregated Schnorr signature (no `.signing.` wrapper here).
SIG=$(python3 -c "import json;print(json.load(open('/tmp/sign1.json'))['signature'])")
echo "signature=$SIG"
```

**Point to highlight:** three separate OS processes, three separate secret files.
Only nonce commitments (public) and partial signatures cross the relay; a share
never leaves its process. The one aggregate signature is indistinguishable from a
normal Schnorr signature and does not reveal *which* trustees signed.

**Say this out loud (trust model):** the relay is untrusted and sees every
envelope, but FROST's own package verification rejects anything that does not
belong to the sender it claims. `sign-party` is a thin wrapper — it never holds a
share. The remaining assumption is that each share file stays secret; in
production each trustee generates its own during DKG rather than receiving it.

> **Shortcut (in-process demo):** `frost-service sign --trustees 5 --threshold 3
> --participants 1,2,3` runs the same protocol with all parties in one process.
> It is convenient for a quick check but hides the trust boundary, so prefer the
> `sign-party` form above when the point *is* separation of duties.

---

## 8. Final Release (Submit to Contract)

Anyone may submit the aggregated signature. The vault checks both gates before
releasing custody.

```bash
# Submit release
cast send $VAULT_ADDR "release(bytes)" 0x$SIG \
  --rpc-url http://127.0.0.1:8545 \
  --unlocked --from $BENEFICIARY
```

**Verify release:**

```bash
cast call $VAULT_ADDR "isReleased()(bool)" --rpc-url http://127.0.0.1:8545
# true

cast balance $VAULT_ADDR --rpc-url http://127.0.0.1:8545
# 0

cast balance $BENEFICIARY --rpc-url http://127.0.0.1:8545
# increased by the 10 ETH the vault was holding
```

**Success!** Both gates passed and the vault's assets moved to the beneficiary:
- ✅ VDF confirmed inactivity (time elapsed, tamper-resistant)
- ✅ Threshold signature accepted (threshold trustees cooperated)

---

## Current Scope & Honesty Notes (for Q&A)

Be upfront about these if asked; they are documented limitations, not hidden gaps.

- **On-chain FROST verification is structural.** `FROSTVerifier` enforces that a
  group key is set, that the signature is well-formed (≥64 bytes), and that each
  signature is used at most once (replay protection). Full secp256k1 Schnorr
  verification on-chain (point math, `sG == R + eP`) is a future enhancement;
  the cryptographic verification is done off-chain by the Rust FROST protocol.
- **RSA modulus is a fixed test value.** The deployed `N` is a small demo
  composite. Production needs a properly generated RSA modulus with unknown
  factorization (trusted-setup considerations are in `docs/VDF_Parameters.md`).
- **The in-process `dkg` / `sign` commands are conveniences.** They run every
  party in one process, so the group key printed by `dkg` is not the same one a
  standalone `sign` uses. The deployed shape is `dkg-party` / `sign-party`, where
  each trustee is a separate process that loads only its own persisted share
  (`persistence.rs`). Those shares survive across runs, so `sign-party` signs
  under the exact group key its DKG produced.
- **T should be chosen to match the real delay.** `T=100` is a fast demo value;
  the contract parameter is what enforces the wall-clock window. VDF proofs now
  verify on-chain exactly (the Rust prover and Solidity verifier share the same
  `keccak256` Fiat-Shamir derivation over 32-byte big-endian words - see
  `test/VDFRealProof.t.sol`).

---

## Presentation Talking Points

| Step | What to Say |
|---|---|
| 1. Quick check | "Threshold crypto works - need 3 of 5 trustees to sign. Below threshold is rejected." |
| 2. DKG | "Distributed key generation - no single person ever has the full private key." |
| 3. Deploy | "Group public key goes on-chain. Private key shares stay with trustees only." |
| 4. Fund/beneficiary | "Non-custodial - owner controls check-ins. Funds sit in vault." |
| 5. Check-ins | "While alive, owner checks in. This resets the timer. No one can trigger release." |
| 6. VDF | "If inactive, VDF computes sequentially proving time passed. Anyone can submit proof - it's permissionless & verifiable." |
| 7. Trustees | "Human judgment matters - trustees independently verify death. Then produce threshold signature together." |
| 8. Release | "Contract requires BOTH: VDF confirmed AND threshold signature. Only then funds release." |

## Cleanup

```bash
# Kill relay
pkill -f frost-relay

# Kill anvil (Ctrl+C in its terminal)
```
