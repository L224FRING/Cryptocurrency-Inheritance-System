# frost-service — design and operation

A detailed walkthrough of the off-chain FROST (secp256k1) threshold signing
service for the inheritance vault: what it does, how the pieces fit together,
what invariants hold, why they hold, and how to drive it.

Crate: `rust/frost-service` (v0.2.0). Ciphersuite: `frost-secp256k1`
(secp256k1, SHA-256), `frost-core` 3.0.0.

---

## 1. What this actually is

Two binaries, one library:

| Target | Purpose |
| --- | --- |
| `lib` (`frost_service`) | the protocol core: each trustee as a `Party`, the message format in `wire`, the coordination layer in `coordinator`, and pluggable `transport`s |
| `bin/frost-service` | CLI: `selftest`, `dkg`, `sign`, `matrix`. Always takes from stdin nothing, writes one JSON document to stdout, exits 0 only on success |
| `bin/frost-relay` | an untrusted message courier. Routes JSON envelopes between trustee mailboxes over blocking HTTP, remembers nothing |

The service implements **two multi-round protocols** end to end:

1. **Setup — DKG** (Distributed Key Generation, `frost::keys::dkg::part1/2/3`).
   All `n` trustees together produce:
   - one long-lived **signing share** per trustee (`KeyPackage`), kept only by
     that trustee, and
   - a shared **group verifying key** (`PublicKeyPackage`), which everyone can
     hold.
   No one ever holds the group secret, before or after.
2. **Signing — two-round FROST** (`frost::round1::commit`,
   `frost::round2::sign`, `frost::aggregate`). Any subset of at least `t`
   trustees produces one Schnorr signature over a message that **every signer
   agreed on and is bound to**.

The design constraint that shaped everything is: **the party logic must not
care how messages travel, and the transport must not care what the messages
mean.** FROST's own per-package verification is the security boundary, so the
relay is deliberately dumb.

---

## 2. Module map

```
src/lib.rs                 crate root, re-exports the public surface
src/error.rs               one Error enum, one Result alias
src/wire.rs                SessionId, Envelope, MessageKind, signed-message builder
src/party.rs               a single trustee's state machine + local crypto
src/coordinator.rs         pumps envelopes until quiet, runs full sessions
src/transport/mod.rs       the Transport trait
src/transport/memory.rs    in-process transport (tests, CLI default)
src/transport/http.rs      relay client + the relay's queue state
src/bin/relay.rs           the frost-relay HTTP server
src/main.rs                the CLI
src/report.rs              the JSON report shapes + stdout writer
tests/protocol.rs          16 end-to-end tests against real protocol runs
```

Dependency direction is one-way: `party` and `coordinator` depend on `wire`
and `error`; `transport` is a separate concern both sides plug into;
`coordinator` sits on top and knows only the `Transport` trait.

---

## 3. The wire format

### 3.1 `SessionId`

```rust
pub type SessionId = [u8; 32];
```

One run of the protocol, DKG *or* signing. It is bound into two very different
places, which together are what make cross-session replay impossible:

- **Every `Envelope`** carries the session it belongs to. Recipients reject an
  envelope whose session does not match their own.
- **Every signed message** is derived from the session (see 3.4), so a
  signature from one session is a signature over different bytes than any
  other session — it cannot verify for a second release attestation.

Invariant: **a session id is single-use for signing.** Commitments received
for a session are retained, duplicates are refused, so a second signing run
with the same id fails. A caller must mint a fresh 32 bytes per run. The CLI
does this automatically; `coordinator::run_signing` does not.

### 3.2 `Envelope`

One protocol message in flight between two trustees:

```rust
pub struct Envelope {
    pub session: SessionId,  // which run this belongs to
    pub kind: MessageKind,   // which FROST message it is
    pub from: u16,           // sender's trustee index
    pub to: u16,             // recipient index, or BROADCAST
    pub body: String,        // the FROST payload, hex-encoded
}
```

`BROADCAST` is `u16::MAX`, meaning "every trustee except the sender". The body
is the ciphersuite's own serialization of a FROST package (`serialize()` /
`deserialize()`), hex-wrapped so the envelope survives as plain JSON over the
relay. Nothing in the envelope is encrypted — see §8 for what that means.

`Envelope::check(session, trustee)` validates the three things a recipient can
check without trusting anybody:

- the session matches,
- the envelope is addressed to the trustee (or to broadcast),
- the envelope does not claim to come from the trustee itself.

DKG `accept_*` paths call `check`; the signing `accept_*` paths instead rely on
the duplicate-refusal logic plus per-session buckets (which serves the same
purpose — a foreign-session signing message lands in a bucket for its own
session, never the current one).

### 3.3 `MessageKind`

Four messages, mirroring the four protocol steps that produce wire traffic:

```
dkg_round1      dkg_round2      signing_round1      signing_round2
```

### 3.4 `signing_message` — what trustees actually sign

Signers agree that the message to sign is the canonical JSON of:

```rust
pub struct SignRequest {
    domain: String,        // a domain separator, e.g. "cis/frost/release-attestation/v1"
    session: String,       // hex(session id)
    payload_hash: String,  // hex(SHA-256(payload))
}
```

Serialized with `serde_json::to_vec`, so the bytes are fully determined by the
three fields and stable across runs and machines (`signing_message_is_stable`
locks this in the unit tests).

Why three bindings:

| Field | Stops |
| --- | --- |
| `domain` | a signature minted for one protocol being verified as a signature for another. Change the domain and the bytes change. |
| `session` | this attestation being lifted into a different signing run. |
| `payload_hash` | the signature being replayed while the payload is edited — the payload itself is never signed directly, only its hash, which is what stops a huge payload changing the signed bytes format. |

`SIGNING_DOMAIN = "cis/frost/release-attestation/v1"` is the default; `sign`
accepts `--domain` to override, and tests verify that a signature created under
one domain fails to verify under another.

---

## 4. A trustee: `Party`

`Party` is one trustee's entire view of the protocol. Its fields are carefully
split between what is private, what is public, and what must never exist:

```
me                  TrusteeId        this trustee's index (1-based)
id                  Identifier       this trustee's FROST identifier
roster              BTreeMap<Id,u16> identifier -> index, for addressing
dkg                 Option<DkgState> in-flight DKG secret + received packages
key_package         Option<KeyPackage>      this trustee's signing share (PRIVATE)
public_key_package  Option<PublicKeyPackage> group verifying key (PUBLIC)
pending             BTreeMap<SessionId, SigningState>   unused nonces
signing_commitments BTreeMap<SessionId, Id -> SigningCommitments>
signature_shares    BTreeMap<SessionId, Id -> SignatureShare>
spent_commitments   HashSet<Vec<u8>>   fingerprints of used nonce sets
```

The only thing a `Party` ever holds secret is `key_package` (its own share)
and `SigningState.nonces`. **No `Party` ever holds the group secret or another
trustee's share**, at any point in setup or signing — matching the FROST
security model. (`DkgState` briefly holds `round2_secret`, which is this
trustee's *intermediate* secret on the way to its own share; it never holds the
final group secret.)

Trustee indices start at **1** (`TrusteeId::new(0)` is refused) and are the
addresses the whole wire format uses. The FROST `Identifier` is derived from
the index via `Identifier::try_from(index)`; the inverse mapping is kept in
`roster` because the ciphersuite offers no `Identifier -> u16` conversion.

### 4.1 DKG state machine

**Round 1 — commitment.** `dkg_round1(session, min, max, rng)`:
`part1(identifier, max, min, rng)` returns this trustee's secret polynomial
(`SecretPackage`, stored as `Option` in `DkgState`) and its public commitment
(`Package`), which is broadcast. Refuses to start while another DKG is in
progress, so a party cannot be tricked into running two DKGs whose randomness
could interact.

**Round 2 — shares.** `dkg_round2()`:
- requires exactly `max_signers - 1` round-1 packages (one per *other*
  trustee; a party never accepts packages from itself — `Envelope::check`
  rejects self-addressed mail),
- `part2(secret, packages)` consumes the round-1 `SecretPackage` **by value**,
  which is why it lives in an `Option` and is `take()`n. A second call is then
  structurally impossible — re-running round 2 would reuse round-1 randomness
  and break the security of the resulting shares,
- emits one `dkg_round2` envelope per *recipient*, addressed point-to-point.
  These packages are **secret per recipient** — the transport must not fan them
  out, which is exactly why they are the only messages that are *not*
  broadcast.

**Part 3 — finalize.** `dkg_finish()`: requires all `max_signers - 1`
round-2 packages, then `part3(...)` produces this trustee's `KeyPackage` and the
shared `PublicKeyPackage`. The DKG state is cleared and the party is now
ready to sign. The group key is returned and every trustee must compute the
*same* bytes; `coordinator::run_dkg` cross-checks all `n` of them and refuses
the run if even one differs.

### 4.2 Signing state machine

**Round 1 — commit.** `signing_round1(session, payload, domain, rng)`:
- refuses if this trustee has no key share (nothing to sign with),
- `round1::commit(signing_share, rng)` draws a fresh nonce pair and its
  commitments,
- refuses if this commitment set's fingerprint was already spent (§3.5),
- records its **own** commitments locally — the transport never echoes a
  broadcast back to its sender, so a party cannot rely on receiving its own
  commitment; the aggregator needs the complete set,
- stores the nonces under `pending[session]` (one-shot: see §5),
- broadcasts the serialized commitments.

**Round 2 — share.** `signing_round2(session, participants)`:
- removes the nonces from `pending` **before doing any work**. If a later step
  fails, the nonces are gone all the same — it is impossible to come back and
  sign a second message with them,
- fingerprints the commitment set into `spent_commitments`,
- assembles the commitments of every participant (its own from local
  state, others' from the per-session bucket; missing one is a hard
  `NotReady`),
- builds the `SigningPackage` from the complete commitment set and the agreed
  `SignRequest` bytes,
- `round2::sign` produces its share of the signature,
- verifies its **own share** immediately against the group key before it goes
  anywhere (a trustee never ships a share it cannot account for),
- records its own share locally and broadcasts it.

**Aggregate.** `aggregate(session, participants, min, payload, domain)`:
- refuses if fewer than `min` participants,
- requires every participant's commitment *and* share,
- `frost::aggregate` combines the shares into one Schnorr signature,
- re-verifies the final signature against the group verifying key before
  returning it. `verified_against_group_key` in the report is therefore not an
  opinion — it is the return value of reaching this line.

### 4.3 The two nonce defenses

FROST nonce reuse is a key-share leak: signing two different messages with the
same nonce pair reveals the signing share. Two independent layers enforce
single-use:

1. **Removal before use.** The nonces live in `pending[session]`, and
   `signing_round2` starts with `pending.remove(&session)`. There is no path
   that leaves them in place while also using them.
2. **A spent-record.** The fingerprint of every consumed commitment set is
   kept in `spent_commitments`. `signing_round1` refuses to hand out a set
   whose fingerprint was already spent — which catches a caller that obtains a
   commitment set and tries to register it twice through the API directly.

The fingerprint is the ciphersuite's canonical serialization of the
commitments, deterministic for a given set.

---

## 5. The coordination layer: `coordinator`

The coordinator's entire job is *delivery*, and it is written to make ordering
irrelevant. Messages are moved in sweeps:

```rust
fn pump(parties, transport) -> usize   // one sweep: drain every party's inbox
fn pump_to_quiescence(parties, ...)    // sweep until nothing moves, max 64 sweeps
```

A session proceeds as a sequence of **collect-then-dispatch** phases:

1. Ask every participant to produce their messages (`dkg_round1` /
   `signing_round1` / ...).
2. Hand every party's outbox to the transport (`deliver_all`).
3. Pump to quiescence, so every envelope reaches its recipient and is
   dispatched, regardless of the order the parties produced them in.

Because `pump` drains each party's queue and immediately dispatches, a party
that receives messages before it has sent its own is handled exactly the same
as one that sends first. The 64-sweep bound turns a relay that keeps echoing
into a hard `Stalled` error instead of a hang.

`coordinator::run_dkg` and `coordinator::run_signing` are the two complete
orchestrations on top of this, and both take `&mut (impl Transport + ?Sized)`
so the same driver runs over `MemoryTransport` or a boxed `HttpTransport`.

`SigningRequest` bundles everything a signing run needs (session,
min_signers, participants, payload, domain). It exists partly because the
unbundled version was eight positional arguments, and partly to make
single-use sessions awkward: to sign twice you must *build a second request*,
not tweak an argument.

---

## 6. Transport layer

### 6.1 The trait

```rust
pub trait Transport {
    fn send(&mut self, from: u16, envelope: Envelope) -> Result<()>;
    fn inbox(&mut self, trustee: u16) -> Result<Vec<Envelope>>; // drains
    fn kind(&self) -> &'static str;   // "memory" | "relay"
    fn sent(&self) -> usize;          // counters, for the reports
    fn delivered(&self) -> usize;
}
```

Two properties every transport implements identically:

- **A sender cannot forge identity through the transport.** `send(from, env)`
  refuses if `env.from != from`. In `MemoryTransport` this is enforced locally;
  in `HttpTransport` it is enforced by the relay, which rejects
  `envelope.from != trustee` on `POST /send`.
- **A broadcast is expanded by the transport**, using the roster, to every
  member except the sender. The *sender's* own mailbox never receives its
  broadcast back — both transports exclude it, which is the root of the bug
  documented in `Bugs_Log.md` §1/§2 and why parties now record their own
  commitments and shares locally.

### 6.2 `MemoryTransport`

Process-local queues (`BTreeMap<u16, VecDeque<Envelope>>`). Used by the CLI
default and by every in-process test. Reads drain the queue — an envelope is
delivered exactly once.

### 6.3 The relay protocol (`HttpTransport` + `frost-relay`)

`frost-relay` is a single-threaded, blocking HTTP server over `std::net` (no
async runtime, no TLS in this build). One connection at a time, one request per
connection (`Connection: close`); envelope ordering within a trustee's queue is
FIFO, which makes the coordinator's delivery rules deterministic.

Endpoints:

| Endpoint | Request body | Response | Effect |
| --- | --- | --- | --- |
| `POST /send` | `{"trustee": N, "envelopes": [...]}` | `{"envelopes": []}` | Queue the envelopes. Broadcasts are expanded here; per-recipient envelopes are queued to exactly one mailbox. Rejects `envelope.from != trustee` with 400. |
| `GET /inbox/{n}` | — | `{"envelopes": [...]}` | Atomically drain trustee n's queue. |
| `GET /stats` | — | `{"queued": N}` | Total envelopes sitting in all queues. |
| `POST /reset` | `{}` | `{"envelopes": []}` | Clear every queue. Used between sessions in tests. |
| `GET /health` | — | `{"ok": true}` | Liveness. |

All bodies are limited to 8 MiB (413 beyond). `HttpTransport` is the client on
the other side: it keeps the same `sent`/`delivered` counters and, crucially,
**re-enforces the send-side identity check** even though the relay also does,
so the client cannot be pointed at a broken relay and silently emit forged
mail.

### 6.4 What the relay is trusted not to do

Trust boundary, stated precisely:

- **Cannot forge a sender.** The `POST /send` handler rejects any envelope whose
  `from` differs from the authenticated `trustee` field. A malicious client
  cannot make the relay attribute its messages to another trustee.
- **Cannot learn a DKG round-2 package meant for someone else.** Those are the
  only envelopes never broadcast; the relay fans out *only* `to == BROADCAST`
  and queues everything else to exactly one mailbox.
- **Gains nothing beyond delivery.** The relay is a worse courier, not a
  trusted participant. FROST package verification happens in the parties; a
  relay that drops, delays, or reorders messages fails the session but cannot
  corrupt the cryptography.

What the relay **is** trusted to do (and must be, by construction, because it
sits on the path): the relay decides queue contents and expands broadcasts, so
it can *mount* a denial of service, and with plaintext HTTP in this build an
eavesdropper on the wire can *read* DKG round-2 packages. For a live
deployment the point-to-point secret packages should travel over TLS or a
second authenticated channel; the protocol code does not depend on that, the
*deployment* does. This limitation is deliberate for the dev harness and is
documented in §8.

---

## 7. End-to-end flows

### 7.1 DKG, `n = 5, t = 3`

```
trustees 1..5          relay / memory          trustee m
──────── NOTHING SECRET IS EVER BROADCAST EXCEPT COMMITMENTS ────────
round 1:
  each 1..5  part1(rng) -> SecretPackage + Package(commitment)
             Envelope(kind=dkg_round1, to=BROADCAST, body=commitment)
             broadcasts; relay expands to the other 4
  each holds 4 commitments, then part2(secret, pkgs)
round 2:
  each 1..5 computes 4 per-recipient SharePackages
             Envelope(kind=dkg_round2, to=r, body=package_for_r)   // point-to-point
  each receives exactly 4, one from every other trustee
part 3:
  each 1..5  part3(...) -> KeyPackage + PublicKeyPackage
  coordinator compares all 5 group keys; equal, or the run is refused
```

Secret material at every point: part1's `SecretPackage` and part2's
`SecretPackage` exist only locally and are consumed; the per-recipient
`SharePackage`s exist on the wire addressed singly, and each party's
`KeyPackage` (its share of the eventual group secret) is created in part 3 and
stays with it.

### 7.2 Signing, participants `[1,2,4]`, `t = 3`

```
signers 1,2,4                  relay            each signer
round 1:
  each   commit(rng) -> Nonces + Commitments
         records OWN commitments locally          <- critical, see Bugs_Log §1
         Envelope(kind=signing_round1, to=BROADCAST, body=commitments)
  each now holds the commitments of the other two
round 2:
  each   remove(nonces) from pending              <- one-shot, see §4.3
         SigningPackage: own commitments + the two remote ones
         round2::sign -> SignatureShare, verify own share vs group key
         record OWN share locally                 <- critical, see Bugs_Log §2
         Envelope(kind=signing_round2, to=BROADCAST, body=share)
aggregate (at participant 1):
  require 3 commitments and 3 shares
  frost::aggregate -> Signature
  verify signature against group verifying key
```

Trustees **3 and 5** (not in the subset) receive the broadcasts too — the
relay fans a broadcast to every roster member. They ignore them: they were not
asked to participate, and `aggregate` only ever looks at the participants
listed in the request. Non-participants hold no key material of value beyond
what they already had.

---

## 8. Threat model and honest limitations

Design goals that hold in the code:

| Claim | Where it is enforced |
| --- | --- |
| No group secret is ever assembled | `Party` never holds it; DKG secrets are consumed; signing is threshold-only with no reconstruction step |
| Nobody signs under the threshold | `aggregate` and `run_signing` both require `participants >= min_signers`; the matrix test sweeps every sub-threshold subset |
| Replay of a whole session | session bound into every envelope and every signed message |
| Signature lifting into another context | session + domain + payload-hash all bound into the signed bytes |
| Nonce reuse | removal-before-use, spent-commitment fingerprints |
| Commitment/share substitution mid-session | duplicates are refused, per sender per session |
| Relay forgery of sender identity | relay rejects `from != trustee`; client re-checks |
| Round-2 DKG confidentiality against the relay | those envelopes are never broadcast |

Limitations to be honest about (all acceptable for the current harness, all
deployment concerns rather than protocol flaws):

1. **Plaintext HTTP.** The relay is untrusted *in what it learns accidentally*,
   but this build does not encrypt point-to-point traffic. A wire sniffer could
   capture DKG round-2 packages. TLS (or a separately authenticated channel for
   the secret packages) is a deployment requirement, not a code one.
2. **No persistence.** Key packages and group keys live in memory only. There
   is no at-rest encryption, no backup of shares, no "lost a trustee" recovery
   beyond running a fresh DKG. Reality needs each trustee's share to survive a
   reboot.
3. **One coordinator process.** The CLI and tests run every `Party` in one
   process. The protocol treats them as independent actors and the relay makes
   the transport genuinely shared, so the seams are real — but this is not yet
   five processes on five machines.
4. **Two-thirds honest assumption is not modeled.** FROST's security assumes a
   *threshold number of honest participants*. Nothing here defends against a
   dishonest participant refusing to sign or broadcasting garbage — that is a
   duty-of-operations concern for the vault logic (e.g. quorum tracking and
   successor election), not this crate.
5. **`commitments_fingerprint` degrades to empty on an impossible
   serialization failure**, which would in theory let a failed fingerprint
   collide two sets. In practice `SigningCommitments::serialize` does not fail;
   the defensive `unwrap_or_default` trades an impossible edge for not panicking.
6. **FROST itself refuses `min_signers < 2`** — a 1-of-n key is not
   expressible via DKG at all. The CLI surfaces this as
   `threshold 2 is not within 2..=5` rather than a ciphersuite error.

---

## 9. The CLI

```
frost-service selftest              in-process DKG + signing check
frost-service dkg    [options]      run the 3-part DKG across the committee
frost-service sign   [options]      run two-round threshold signing
frost-service matrix [options]      sweep n-of-m and sub-threshold subsets
frost-service --help

--trustees N        committee size                     (default 5)
--threshold T       signing threshold                  (default 3)
--session HEX       32-byte session id                 (default: random)
--domain STR        signing domain separator           (default cis/frost/release-attestation/v1)
--relay URL         coordinate through a relay         (default: in-process)
--message HEX       payload to sign                    (default: the attestation string)
--participants L    comma-separated trustee indices    (default: 1..T)
--expect-failure    require rejection, report why
```

Every subcommand prints **one** JSON document on stdout — nothing else — and
exits non-zero on failure, so a caller can never parse a half-written report as
a success. A rejected `dkg` run that was *expected* to be rejected (`dkg
--expect-failure`) is a valid JSON document with exit 0 whose `status` is
`rejected_as_expected`; `sign --expect-failure` is refused up front, because
`sign` already ran a DKG and there is nothing to "expect to reject" from a
standalone viewpoint.

- **`selftest`** — the FFI contract exercised by `test/FfiBridge.t.sol`: full
  3-of-5 DKG, a 3-signer signature, a 2-signer refusal, all in one report with
  a `status` field. Failure replaces the whole document with
  `{"status":"error","error":...}`.
- **`dkg`** — prints `DkgOutcome`: the session, the group key, and all `n`
  per-trustee group keys (a report where they disagree is a failed DKG).
- **`sign`** — runs a fresh DKG first (no persistence), then signs with
  `--participants`, embedding both the DKG summary and `SigningOutcome` in the
  JSON. The signature is a 65-byte secp256k1 compact signature (130 hex chars),
  and `verified_against_group_key` is set by actually verifying it.
- **`matrix`** — the proof sweep. For committees 2, 3, and N, and every
  threshold `2..=m`, three cases each: at-threshold signs, above-threshold
  signs, below-threshold is rejected. Each signing run gets a fresh session
  (`SignRequest` is single-use), and the run prints `passed`/`failed` plus the
  per-case verdicts.

Full relay run:

```sh
frost-relay --listen 127.0.0.1:8477 --trustees 5     # terminal 1
frost-service sign --relay http://127.0.0.1:8477 \
                   --participants 2,3,5 --message 0xdeadbeef
```

---

## 10. Reports

Report structs live in `src/report.rs` / `src/coordinator.rs` and are plain
`serde::Serialize` — no printing library, just one `report::emit`. The shape of
`selftest` output (which is the FFI contract) is locked by `FfiBridge.t.sol`,
which reads it with `vm.parseJson*`: rename a field and the Solidity test fails
with a path error instead of quietly scraping.

A below-threshold attempt yields **one** refusal, not one per trustee: the
coordinator checks the participant count before *any* round runs, so no round-2
message ever exists. The report field is named `rejections` (not
`round2_rejections`) because calling it that would describe rounds that do not
happen.

---

## 11. Tests

41 tests, two layers.

**25 unit tests** in the modules they exercise:

- `wire`: each session/domain/payload binding changes the signed bytes; the
  signing message is stable; envelopes reject foreign sessions, wrong
  recipients, and self-addressing; envelopes round-trip JSON.
- `memory` / `http` transports: broadcast reaches everyone but the sender;
  point-to-point stays private (round-2 packages especially);
  sender-identity forgery is refused; inboxes drain; reset clears; session ids
  survive the trip.
- `party`: index 0 refused, off-committee trustees refused, signing before a
  DKG is refused.

**16 integration tests** in `tests/protocol.rs`, which actually run complete
multi-party sessions and check the *claims*, not just the happy path:

| Test | What it proves |
| --- | --- |
| `dkg_agrees_on_one_group_key_for_every_trustee` | convergence |
| `exactly_the_threshold_can_sign` | 3 of 5 signs, **independently verified** with `frost` against the group key — not trusted from the crate's own bookkeeping |
| `more_than_the_threshold_can_sign` | 5 of 5 signs, verified |
| `any_subset_at_or_above_the_threshold_can_sign` | all combinations of 3 of 5 + larger subsets |
| `one_short_of_the_threshold_cannot_sign` | 2 of 3 refused |
| `sub_threshold_collusion_is_refused_across_the_matrix` | below-threshold refusal at every threshold from 2..5 |
| `a_single_trustee_alone_cannot_sign_a_three_of_five` | each of 1..5 alone fails |
| `a_signature_binds_only_to_its_own_session_domain_and_payload` | tamper with any of the three, verification fails |
| `a_session_id_cannot_be_reused_for_a_second_signature` | the single-use rule |
| `a_replayed_round1_commitment_is_rejected` | duplicate commitment |
| `a_duplicate_signature_share_is_refused` | duplicate share |
| `a_trustee_will_not_sign_twice_with_the_same_nonces` | one-shot nonces |
| `dkg_round2_packages_never_reach_a_third_party` | 6 point-to-point packages, zero broadcasts |
| `full_protocol_runs_over_a_live_relay` | spawns the real `frost-relay`, signs over it |
| `a_relay_can_be_reset_between_sessions` | two independent sessions, reset in between |
| `in_process_and_relay_transports_agree_on_the_outcome` | same protocol over memory and relay |

`frost-service matrix` re-runs the n-of-m sweep from the CLI and reports the
same verdicts, so the proof is reproducible by hand.

---

## 12. Building and verifying

```sh
# Rust half: 41 tests, clippy, format
cargo test     --manifest-path rust/frost-service/Cargo.toml
cargo clippy   --manifest-path rust/frost-service/Cargo.toml --all-targets
cargo fmt      --manifest-path rust/frost-service/Cargo.toml --check

# Solidity half — this shells out to the DEBUG binary, so build it first
cargo build    --manifest-path rust/frost-service/Cargo.toml
forge test
forge fmt --check

# The proof sweep, by hand
cargo run --manifest-path rust/frost-service/Cargo.toml -- matrix
```

> **Stale-binary footgun.** `forge test` does not build the Rust crate; it runs
> `rust/frost-service/target/debug/frost-service` as it exists on disk. Edit
> Rust, skip the `cargo build`, run `forge test` and one of the
> schema-dependent tests will silently pass against the *previous* binary
> unless the mismatch happens to break another assertion. Always
> `cargo build && forge test`.

---

## 13. Where this slots into the vault

The service signs one thing today: a release attestation message, by default
the UTF-8 string `inheritance-release-attestation`, hashed with SHA-256 under a
domain separator. The message bytes are derived from `(domain, session,
payload_hash)`.

The three integration points a consumer of a final `Signature` cares about:

1. **The group verifying key** (`DkgOutcome.group_verifying_key`) — the
   identity of the whole committee. If a Solidity contract should authenticate
   deliveries, this is the value to publish on-chain.
2. **The signature** (`SigningOutcome.signature`, 65 bytes compact) and **the
   message** it covers (`signing_message(session, domain, payload)`) — the
   signature only verifies for exactly that `(session, domain, payload)` triple;
   a verifier must reproduce the same `SignRequest` bytes.
3. **The session id** — the vault's own nonce story (which release, which
   sequence number, which inactivity claim) becomes the `SessionId`. Because
   sessions are single-use, the vault picking session ids is what prevents
   repeat attestations from being interchangeable.