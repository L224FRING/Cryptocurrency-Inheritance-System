# QA Test Guide - Full User Journey

## 1. FROST DKG & Threshold Signing (Trustee Setup)
```bash
cd /Users/somamacbook/Cryptocurrency-Inheritance-System
./rust/frost-service/target/debug/frost-service selftest
```
Expected: status "ok", 3-of-5 signs successfully, 2-of-5 rejected.

## 2. Matrix Test (All Threshold Combinations)
```bash
./rust/frost-service/target/debug/frost-service matrix
```
Expected: status "ok", passed 18, failed 0.

## 3. VDF Computation & Verification
```bash
./rust/frost-service/target/debug/frost-service vdf selftest
```
Expected: status "ok", proof verification succeeds.

```bash
./rust/frost-service/target/debug/frost-service vdf --t 20 --input 0x1234abcd
```
Expected: Returns x, y, t, n, proof_points in JSON.

## 4. Death Attestation
```bash
./rust/frost-service/target/debug/frost-service attest
```
Expected: status "ok", signature returned.

## 5. Full Relay Test (Multi-Party Simulation)
Terminal 1:
```bash
./rust/frost-service/target/debug/frost-relay --listen 127.0.0.1:8477 --trustees 5
```

Terminal 2:
```bash
./rust/frost-service/target/debug/frost-service sign --relay http://127.0.0.1:8477 --participants 2,3,5
```
Expected: Signing succeeds over relay transport.

## 6. Rust Unit & Integration Tests
```bash
cargo test --manifest-path rust/frost-service/Cargo.toml
```
Expected: All 41 tests pass.

## 7. Solidity Tests (If Foundry available)
```bash
forge test
```
Tests VDF, FROST, Integration, Edge cases.

## 8. End-to-End Journey Validation

The full user journey (per User_Journey.md):
1. Owner sets up with trustees via DKG ✓
2. Periodic check-ins ✓ (checkIn in contracts)
3. VDF runs on inactivity ✓ (VDF CLI + contracts)
4. Trustees attest on death ✓ (attest command)
5. Threshold reaches, FROST signs ✓ (sign/selftest)
6. Assets released ✓ (release function in InheritanceVault)
