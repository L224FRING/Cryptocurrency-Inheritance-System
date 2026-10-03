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
cd /Users/somamacbook/Cryptocurrency-Inheritance-System
./rust/frost-service/target/debug/frost-relay --listen 127.0.0.1:8477 --trustees 5
```

### Terminal 2: Run DKG Ceremony (Owner Orchestrates)

Simulate 5 trustees doing 3-of-5 DKG. In reality, each trustee runs their own client.

```bash
cd /Users/somamacbook/Cryptocurrency-Inheritance-System
./rust/frost-service/target/debug/frost-service dkg \
  --relay http://127.0.0.1:8477 \
  --trustees 5 \
  --threshold 3
```

**Save the result.** Copy `group_verifying_key` from output.

Example:
```json
{
  "group_verifying_key": "02e49d08a3d768f9016c10522821b8eb2d0fb0f965b13256ec8044c3c4368c0351",
  "status": "ok"
}
```

**Key point:** At the end, each trustee has their own secret share. The owner holds only the group public key.

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
cd /Users/somamacbook/Cryptocurrency-Inheritance-System

# Owner address from anvil
export OWNER=0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
export BENEFICIARY=0x70997970C51812dc3A010C7d01b50e0d17dc79C8
export GROUP_PUBKEY="02e49d08a3d768f9016c10522821b8eb2d0fb0f965b13256ec8044c3c4368c0351"

# Deploy full stack (VDFVerifier + FROSTVerifier + InheritanceVault)
forge script script/DeployFull.s.sol \
  --rpc-url http://127.0.0.1:8545 \
  --broadcast \
  --private-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80
```

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
  --from $OWNER
```

**Point to highlight:** The group public key (from distributed DKG) is stored on-chain. No private key is ever deployed.

---

## 4. Fund Vault & Set Beneficiary (Owner Preparation)

```bash
# Set beneficiary
cast send $VAULT_ADDR "setBeneficiary(address)" $BENEFICIARY \
  --rpc-url http://127.0.0.1:8545 \
  --from $OWNER

# Fund vault with 10 ETH
cast send $VAULT_ADDR --value 10ether \
  --rpc-url http://127.0.0.1:8545 \
  --from $OWNER

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
  --from $OWNER
```

**Point to highlight:** As long as the owner checks in regularly, the VDF timer never completes. No trustee action is ever triggered. This is passive protection.

You can repeat this to demonstrate it's safe and doesn't move funds.

---

## 6. Inactivity Scenario: Compute & Submit VDF Proof

Now simulate owner becoming inactive. Someone computes VDF off-chain to prove time has elapsed.

**Get challenge and params:**

```bash
CHALLENGE=$(cast call $VAULT_ADDR "getCurrentChallenge()(bytes32)" --rpc-url http://127.0.0.1:8545 | cut -c3-)
echo "Challenge: 0x$CHALLENGE"
T_VAL=$(cast call $VDF_ADDR "T()(uint256)" --rpc-url http://127.0.0.1:8545)
echo "T: $T_VAL"
```

**Compute VDF off-chain (Terminal 4):**

```bash
./rust/frost-service/target/debug/frost-service vdf --t $T_VAL --input 0x$CHALLENGE
```

Save `y` and all `proof_points`. For a live demo with small T (like 5-10), it's instant. With T=100, it takes time - showing it's truly sequential.

```bash
# Example extraction (adjust with actual values)
export VDF_Y=<y from output>
export VDF_PROOF='["<p1>","<p2>",...]'
```

**Advance time/blocks to satisfy required delay:**

```bash
# Mine blocks past required delay
cast rpc evm_mine --rpc-url http://127.0.0.1:8545
```

**Submit proof on-chain (permissionless - anyone can do):**

```bash
cast send $VDF_ADDR "submitVDFProof(uint256,uint256[])" \
  $VDF_Y \
  $VDF_PROOF \
  --rpc-url http://127.0.0.1:8545 \
  --from $BENEFICIARY  # or any address
```

**Verify:**

```bash
cast call $VDF_ADDR "isInactivityConfirmed()(bool)" --rpc-url http://127.0.0.1:8545
# returns: true
```

**Point to highlight:** VDF proof is self-verifying on-chain. No trusted party needed to submit it. The sequential computation proves real time passed.

**False alarm demo (optional):** If you check-in again with `cast send $VAULT_ADDR "checkIn()" --from $OWNER` before submitting proof, you reset the timer. This prevents false triggers.

---

## 7. Trustee Perspective: Threshold Signing for Release

Now trustees see inactivity is confirmed and must decide - if death is confirmed, they proceed.

### Step 7a: Trustees attest (off-chain human decision)

Each trustee runs attestation on their own device:

```bash
./rust/frost-service/target/debug/frost-service attest --message "death-confirmed"
```

**Point to highlight:** This represents the trustee's independent judgment. The protocol doesn't replace human verification.

### Step 7b: Trustees produce threshold FROST signature

Once ≥3 trustees agree, they sign. With relay running (Terminal 1), do signing with 3 participants.

```bash
cd /Users/somamacbook/Cryptocurrency-Inheritance-System
./rust/frost-service/target/debug/frost-service sign \
  --relay http://127.0.0.1:8477 \
  --trustees 5 \
  --threshold 3 \
  --participants 1,2,3 \
  --message "inheritance-release"
```

Copy the aggregated signature from `signature.value`.

Example:
```json
"signature": {
  "value": "026cf81c...",
  "verified_against_group_key": true,
  "shares_aggregated": 3
}
```

**Point to highlight:** Each trustee contributes their partial signature locally. Only partials are exchanged. The full key is never reconstructed at any point.

---

## 8. Final Release (Submit to Contract)

Anyone submits the aggregated signature. The vault verifies both conditions.

```bash
export SIG="<signature.value from step 7b>"

# Submit release
cast send $VAULT_ADDR "release(bytes)" $SIG \
  --rpc-url http://127.0.0.1:8545 \
  --from $BENEFICIARY
```

**Verify release:**

```bash
cast call $VAULT_ADDR "isReleased()(bool)" --rpc-url http://127.0.0.1:8545
# true

cast balance $BENEFICIARY --rpc-url http://127.0.0.1:8545
# Shows increased balance by ~10 ETH
```

**Success!** Both gates passed:
- ✅ VDF confirmed inactivity (time elapsed, tamper-resistant)
- ✅ Valid threshold FROST signature (≥3 trustees cooperated)

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
