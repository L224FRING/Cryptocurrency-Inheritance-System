# Bugs Found and Fixed

A running log of real defects hit while building the multi-party FROST service,
most of them only surfaced by end-to-end tests rather than by the compiler.

Ordered roughly by severity. Every entry names the test that now prevents a
regression.

---

## 1. The aggregator never saw its own signing commitment

**Symptom:** `not ready: missing commitment from trustee 1` on the very first
end-to-end run. DKG succeeded; signing could not start.

**Root cause:** `signing_round1` emits a `BROADCAST` envelope. Both transports
expand a broadcast to every committee member *except the sender* — correctly, so
a party cannot receive its own traffic back. `accept_signing_round1` therefore
never saw the local trustee's own commitment, and `aggregate` builds its
`SigningCommitments` map entirely from `self.signing_commitments`, which was
missing one entry.

The compiler was happy: this was purely a protocol-shape mismatch between "what
the network delivers" and "what aggregation assumes it holds".

**Fix:** `party.rs` records its own commitment into `signing_commitments[session]`
at the moment it generates it, rather than relying on the network to return it.

**Locked in by:** `exactly_the_threshold_can_sign`.

## 2. The aggregator never saw its own signature share

**Symptom:** `not ready: missing share from trustee 1`, immediately after
fixing #1.

**Root cause:** identical to #1, one round later. `signing_round2` broadcasts its
share; the relay drops it for the sender; `aggregate` could not find its own share.

**Fix:** `party.rs` records its own share into `signature_shares[session]` after
verifying it.

**Locked in by:** `exactly_the_threshold_can_sign` (the test cannot pass without
both fixes).

> #1 and #2 are the same bug wearing two hats. The underlying lesson: a
> coordinator must never infer local state from message delivery. Anything a
> party needs for aggregation has to be recorded when it is produced, because the
> transport is entitled never to send a party its own output.

## 3. A duplicate signing commitment was silently overwritten

**Symptom:** `a_replayed_round1_commitment_is_rejected` failed — replaying the
identical `SigningRound1` envelope twice was accepted.

**Root cause:** `accept_signing_round1` used `BTreeMap::insert`, which
overwrites. A second commitment from the same trustee replaced the first, so an
attacker (or a buggy coordinator re-sending after a timeout) could swap the nonce
commitment set for a session *after* other parties had already committed to
signing under the original one.

**Fix:** refuse a second commitment from the same trustee for the same session.

**Locked in by:** `a_replayed_round1_commitment_is_rejected`.

## 4. A duplicate signature share was silently overwritten

**Symptom:** found while fixing #3, which prompted the same audit on round 2.

**Root cause:** `accept_signing_round2` had the same `insert`-overwrites
behaviour.

**Fix:** refuse duplicates, same as round 1.

**Locked in by:** `a_duplicate_signature_share_is_refused`.

## 5. DKG round-1 secret was not one-shot

**Symptom:** a compile error at first — `cannot move out of state.round1_secret
which is behind a mutable reference` — with the compiler offering `.clone()`.

**Root cause:** `frost::keys::dkg::part2` consumes the round-1 `SecretPackage` by
value. The suggested `.clone()` would have compiled, and would have been a
latent disaster: it would leave the round-1 secret sitting in the struct, letting
`dkg_round2` run twice and reuse round-1 randomness. Reusing DKG round-1
randomness across two runs breaks the security of the resulting shares.

**Fix:** hold `round1_secret` as `Option<..>` and `take()` it. The compiler error
was pointing at a security bug, not a syntax problem.

**Locked in by:** the `Option` plus the `NotReady("dkg round 2 has already run")`
branch; a second call is now impossible by construction.

## 6. Signing nonces could be consumed twice

**Symptom:** a test written to prove nonce reuse was impossible instead *passed*
— the second signing round succeeded.

**Root cause:** the test was wrong, not the code. It called `signing_round1` a
second time, which mints a **fresh** nonce pair, so there was no reuse to detect.
`pending.remove(&session)` was already doing the right thing.

**Fix:** rewrote the test to assert the actual invariant — a second
`signing_round2` with no intervening `signing_round1` must fail with
`NoncesReused`, because the nonces are removed *before* any work is attempted.

**Locked in by:** `a_trustee_will_not_sign_twice_with_the_same_nonces`.

## 7. Relay advertised the address it was asked for, not the one it bound

**Symptom:** `full_protocol_runs_over_a_live_relay` failed with
`Connection refused`, intermittently, and the *other* relay test passed. It
looked like a flaky test and was almost dismissed as one.

**Root cause:** `frost-relay --listen 127.0.0.1:0` binds an OS-assigned port but
printed the literal string `127.0.0.1:0`. The integration test parsed the greeting
and dialled port 0, hence `ECONNREFUSED`. It only appeared intermittent because
the passing test was reading a greeting from an already-rebuilt binary while the
failing one raced a rebuild.

**Fix:** print `listener.local_addr()` and flush stdout, so a caller can learn the
real port.

**Locked in by:** `RunningRelay::start` in `tests/protocol.rs` cannot pass against
the old behaviour.

## 8. `run_signing` took eight positional arguments

**Symptom:** `clippy::too_many_arguments` (8/7).

**Root cause:** a design smell rather than a fault. The argument list included
`session`, which is **single-use by design** — and a positional call site makes
"reuse this session for a second run" look completely natural. It is exactly the
mistake worth making awkward.

**Fix:** grouped into a `coordinator::SigningRequest`. Building a second request
is now the obvious way to sign twice, and the compiler enforces that `run_dkg`
and `run_signing` are called the same way.

**Locked in by:** every call site in `tests/protocol.rs` and `src/main.rs`.

## 9. CLI argument errors were reported as `bad trustee`

**Symptom:** `bad trustee: unknown flag '--nope'`,
`bad trustee: threshold 1 is not within 2..=5`.

**Root cause:** `main.rs` reused `Error::BadTrustee` for every input problem,
because it was the only "caller got it wrong" variant available.

**Fix:** added `Error::BadArgument`. `BadTrustee` is now reserved for genuine
identifier problems.

## 10. The relay could not be driven through a trait object

**Symptom:** `the size for values of type dyn Transport cannot be known at
compilation time`.

**Root cause:** `impl Trait` in argument position implies `Sized`, so the
coordinator could not accept the `Box<dyn Transport>` the CLI builds to choose
between the in-memory and relay transports at runtime.

**Fix:** `&mut (impl Transport + ?Sized)` throughout the coordinator, and the
same for the `transport_sent` / `transport_delivered` helpers.

## 11. ureq 3 API used as if it were ureq 2

**Symptom:** `no method named into_json found on struct Response<Body>`, plus
`send_json` missing entirely.

**Root cause:** `into_json` is ureq 2. In ureq 3 the JSON helpers moved onto the
body (`response.body_mut().read_json::<T>()`) and sit behind a `json` feature.
The dependency was declared `default-features = false` to keep the TLS stack out,
which also dropped `json`.

**Fix:** use `body_mut().read_json()`, and declare
`ureq = { version = "3", default-features = false, features = ["json"] }`.

## 12. `Display for TrusteeId` missing, and `me()` returning the wrong type

**Symptom:** a cluster of `TrusteeId doesn't implement Display` and
`expected u16, found TrusteeId` errors.

**Root cause:** `me()` returned `TrusteeId`, but essentially every caller wanted
the index — for envelope addressing, for error text, for map keys.

**Fix:** implemented `Display` (the error messages genuinely need it) and changed
`me()` to return `u16`, with `trustee_id()` available for the rare caller that
wants the newtype.

---

## Two test failures that turned out to be the code being right

Worth recording separately, because the tempting move in both cases was to make
the test pass by weakening it.

### A. `FfiBridge.t.sol` expected two below-threshold rejections; there is one

The Solidity test asserted `.below_threshold.round2_rejections` had length 2,
implying one rejection per non-signing trustee. The actual behaviour is a single
up-front refusal: the coordinator checks `participants < min_signers` before any
round runs, so no round-2 message ever exists.

The old field name was also wrong — "round2_rejections" described rounds that do
not happen.

**Fix:** renamed the field to `rejections` with a comment explaining it holds one
explanation rather than one line per trustee, and strengthened the Solidity
assertion to check the error *names the threshold* (`_countOf(err, "threshold")`),
which is a stronger claim than counting array entries. Both the Rust report and
`FfiBridge.t.sol` were updated together; neither was bent to match the other.

### B. `matrix` reported four failures on above-threshold subsets

`frost-service matrix` initially reported `passed: 14, failed: 4` — every
"more signers than the threshold" case.

**The code was correct.** The new duplicate-commitment guard (fix #3) refuses a
second signing run under a session id that has already been signed over. The
matrix was minting one session id per committee and reusing it for the
threshold-sized run, the above-threshold run, and the below-threshold run.

**Fix:** mint a fresh session id per signing run in the matrix. The sweep now
reports 18/18. Session single-use is a deliberate property, not an inconvenience to
work around — the README documents it as an API constraint on callers.

---

## Non-bug worth recording

**FROST's DKG refuses a threshold below 2** (`min_signers must be at least 2`), so
a 1-of-n key is not expressible via DKG at all. The matrix sweep was written for
`1..=trustees` and had to start at 2; `parse_options` now rejects `--threshold 1`
up front with `threshold 1 is not within 2..=5` rather than surfacing a
ciphersuite error from three layers down.
