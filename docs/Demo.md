# FROST Service — Live Demo Runbook

A 5-minute walkthrough of the multi-party FROST threshold-signing service,
suitable for a live screen-share demo. Every command below produces the exact
output shown; nothing is mocked.

## Prep (once, ~30s)

```sh
cd /home/punav/Crypto_Inheritance_System
cargo build --manifest-path rust/frost-service/Cargo.toml
```

> Stale-binary footgun: `forge test` and `frost-service` run the binary on
> disk. Re-run this build before every demo so no one sees an old binary.

When the four terminal windows below are open, the sequence is six steps and
ends in a clean close ("what we have not done yet").

---

## 1. It's a real protocol, with real tests (~40s)

```sh
cargo test --manifest-path rust/frost-service/Cargo.toml
```

41 tests pass. Point at the integration test names:

- `exactly_the_threshold_can_sign` / `every_3_of_5_combination_signs` —
  *"any 3 of 5 trustees signs"*
- `below_threshold_subsets_are_all_refused` — *"every 2-of-5 collusion fails"*
- `duplicate_commitments_are_refused`, `duplicate_shares_are_refused`,
  `a_session_id_cannot_be_reused_for_a_second_signature` — *"the replay guards"*

Talking point: *"The signatures are verified independently against the group
key using `frost` directly — not against the crate's own bookkeeping."*

## 2. Live 3-of-5 signature, plus the failure, in one doc (~1m)

```sh
rust/frost-service/target/debug/frost-service selftest
```

Walk the JSON:

- `status: "ok"`, `scheme: "frost-secp256k1"`
- `dkg: {label: "3-of-5", threshold: 3, trustees: 5}`
- one `group_verifying_key`
- `signature`:
  - `signers: [1, 2, 3]`, `shares_aggregated: 3`
  - `value` — 130 hex chars = 65-byte secp256k1 signature
  - `verified_against_group_key: true`
- the punchline — `below_threshold`:
  - `signers: [1, 2]`, `rejections: ["session refused: not ready: 2 participants cannot meet a threshold of 3"]`
  - `aggregate_rejected: true`, `aggregate_error: "aggregate_rejected"`

Talking point: *"One run, both outcomes: 3 of 5 signs and verifies; 2 of 5 is
refused before it gets anywhere."*

## 3. Two trustees cannot forge a release (~30s)

```sh
rust/frost-service/target/debug/frost-service sign --participants 1,2
echo "exit=$?"
```

Output is `status: "error"`, then `exit=1`. Talking point: *"The exit code is
the trust boundary — Solidity invokes this binary via FFI and never parses a
half-written report as success."*

## 4. The relay: untrusted, but five machines would use it (~1m)

Terminal A — start the relay:

```sh
rust/frost-service/target/debug/frost-relay --listen 127.0.0.1:8477 --trustees 5
```

Terminal B — sign over it:

```sh
rust/frost-service/target/debug/frost-service sign \
  --relay http://127.0.0.1:8477 --participants 2,3,5
```

The `signing.transport` field reads `"relay"` and `envelopes_sent` /
`envelopes_delivered` are non-zero. Talking point: *"The relay routes opaque
envelopes, enforces no policy, and cannot read DKG round-2 packages addressed
to a specific trustee. DKG round-1 commitments are broadcast; shares are always
point-to-point."*

## 5. The chain sees it (~1m) — the FFI seam

```sh
forge test -vv
```

8 tests pass. Open `test/FfiBridge.t.sol` in the editor and show a test
actually invoking the Rust binary and parsing its JSON with `vm.parseJson*`
(no contract code on this side yet — this is proof the boundary works).
Talking point: *"This is the seam the contracts will sit on."*

## 6. And it's all documented (~20s)

- `docs/FrostService.md` — design walkthrough of the whole service
- `docs/Bugs_Log.md` — three bugs compilers could not catch (own
  commitment/share not recorded, DKG round-1 secret not one-shot, relay
  printing the requested address instead of the bound one)
- `docs/Formal_Spec_Threat_Model.md` — the spec


## 7. VDF computation and verification (Rust) (~30s)

The Verifiable Delay Function (Pietrzak's construction) provides sequential
time delay that cannot be parallelized. The Rust implementation computes
y = x^(2^t) mod N and generates proof points.

```sh
rust/frost-service/target/debug/frost-service vdf
```

Walk the JSON:
- `x` - input value (hex)
- `y` - computed output after 2^t sequential squarings (hex)  
- `t` - number of squaring steps
- `n` - RSA modulus (hex)
- `proof_points` - Pietrzak proof halfway values for fast verification
- `status: "ok"`

Try with custom parameters:

```sh
rust/frost-service/target/debug/frost-service vdf --t 20 --input 0x1234abcd
```

Talking point: *"The computation is intentionally sequential - parallelization doesn't help. The proof allows verification in O(log t) steps rather than recomputing O(t) squarings."*
## Close

*"What we have not done yet: the VDF + inactivity clock. That is the next
phase — a delay function hashed into blocks that no amount of parallel hash
power can fast-forward, with the on-chain verifier replacing the raw timer.
Everything you saw becomes the proof that the threshold side is sound before we
gate it behind time."*