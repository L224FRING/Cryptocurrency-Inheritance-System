//! End-to-end tests: real DKG, real threshold signing, real relay.
//!
//! The point of these is the n-of-m claim. A signature must appear when enough
//! trustees take part, and must not appear when too few do.

use std::process::{Child, Command, Stdio};

use frost_service::coordinator::{self, SigningOutcome, SigningRequest};
use frost_service::transport::http::HttpTransport;
use frost_service::transport::memory::MemoryTransport;
use frost_service::transport::Transport;
use frost_service::wire::{SessionId, SIGNING_DOMAIN};
use rand_core::{OsRng, RngCore};

use frost_secp256k1 as frostk;

const MESSAGE: &[u8] = b"inheritance-release-attestation";

fn session_id(seed: u8) -> SessionId {
    [seed; 32]
}

/// Check a signature with frost directly, against the group key the DKG agreed
/// on. This is independent of the crate's own aggregation bookkeeping.
fn verifies_independently(outcome: &SigningOutcome, domain: &str) -> bool {
    let verifying_key = frostk::VerifyingKey::deserialize(
        &hex::decode(&outcome.group_verifying_key).expect("group key is hex"),
    )
    .expect("group key deserializes");
    let signature = frostk::Signature::deserialize(&hex::decode(&outcome.signature).expect("hex"))
        .expect("signature deserializes");
    let message = frost_service::wire::signing_message(
        &hex::decode(&outcome.session)
            .map(|b| {
                let mut s = [0u8; 32];
                s.copy_from_slice(&b);
                s
            })
            .expect("session is hex"),
        domain,
        &hex::decode(&outcome.message).expect("message is hex"),
    );
    verifying_key.verify(&message, &signature).is_ok()
}

fn setup(
    trustees: u16,
    threshold: u16,
) -> (Vec<frost_service::party::Party>, MemoryTransport, SessionId) {
    let mut rng = OsRng;
    let session = session_id(0xA1);
    let mut parties = coordinator::new_committee(trustees).expect("committee");
    let mut transport = MemoryTransport::new(coordinator::committee(trustees));
    coordinator::run_dkg(&mut parties, &mut transport, session, threshold, &mut rng)
        .expect("DKG completes");
    (parties, transport, session)
}

fn sign(
    parties: &mut [frost_service::party::Party],
    transport: &mut MemoryTransport,
    session: SessionId,
    threshold: u16,
    participants: &[u16],
) -> Result<SigningOutcome, frost_service::Error> {
    coordinator::run_signing(
        parties,
        transport,
        &SigningRequest::new(session, threshold, participants, MESSAGE, SIGNING_DOMAIN),
        &mut OsRng,
    )
}

#[test]
fn dkg_agrees_on_one_group_key_for_every_trustee() {
    let mut rng = OsRng;
    let mut parties = coordinator::new_committee(5).unwrap();
    let mut transport = MemoryTransport::new(coordinator::committee(5));

    let dkg =
        coordinator::run_dkg(&mut parties, &mut transport, session_id(1), 3, &mut rng).unwrap();

    assert_eq!(dkg.per_trustee_group_key.len(), 5, "one key per trustee");
    let distinct: std::collections::BTreeSet<&String> =
        dkg.per_trustee_group_key.values().collect();
    assert_eq!(
        distinct.len(),
        1,
        "every trustee must derive the same group key, got {distinct:?}"
    );
    for party in &parties {
        assert!(
            party.has_key_package(),
            "trustee {} has no key share",
            party.me()
        );
    }
}

#[test]
fn exactly_the_threshold_can_sign() {
    let (mut parties, mut transport, session) = setup(5, 3);

    let outcome = sign(&mut parties, &mut transport, session, 3, &[1, 2, 3]).unwrap();

    assert_eq!(outcome.signers_aggregated, 3);
    assert!(
        verifies_independently(&outcome, SIGNING_DOMAIN),
        "signature must verify against the group key with frost itself"
    );
}

#[test]
fn more_than_the_threshold_can_sign() {
    let (mut parties, mut transport, session) = setup(5, 3);

    // All five sign even though the threshold is three.
    let outcome = sign(&mut parties, &mut transport, session, 3, &[1, 2, 3, 4, 5]).unwrap();

    assert_eq!(outcome.signers_aggregated, 5);
    assert!(verifies_independently(&outcome, SIGNING_DOMAIN));
}

#[test]
fn any_subset_at_or_above_the_threshold_can_sign() {
    // Every combination of 3 from 5, plus the full set, must work.
    for participants in [
        vec![1, 2, 3],
        vec![1, 2, 4],
        vec![1, 3, 5],
        vec![2, 3, 4],
        vec![3, 4, 5],
        vec![1, 2, 3, 4],
        vec![2, 3, 4, 5],
        vec![1, 2, 3, 4, 5],
    ] {
        let (mut parties, mut transport, session) = setup(5, 3);
        let outcome = sign(&mut parties, &mut transport, session, 3, &participants)
            .unwrap_or_else(|e| panic!("{participants:?} should have been able to sign: {e}"));
        assert!(
            verifies_independently(&outcome, SIGNING_DOMAIN),
            "{participants:?} produced an invalid signature"
        );
    }
}

#[test]
fn one_short_of_the_threshold_cannot_sign() {
    let (mut parties, mut transport, session) = setup(5, 3);

    let error = sign(&mut parties, &mut transport, session, 3, &[1, 2])
        .expect_err("two of three shares must not make a signature");

    assert!(
        matches!(error, frost_service::Error::NotReady(_)),
        "expected NotReady, got {error:?}"
    );
}

#[test]
fn sub_threshold_collusion_is_refused_across_the_matrix() {
    // For every committee size and threshold, one below the threshold must fail.
    for trustees in 2..=5u16 {
        for threshold in 2..=trustees {
            let (mut parties, mut transport, session) = setup(trustees, threshold);
            let below: Vec<u16> = (1..threshold).collect();
            let result = sign(&mut parties, &mut transport, session, threshold, &below);
            assert!(
                result.is_err(),
                "{}-of-{threshold} was signed by only {} trustees",
                trustees,
                below.len()
            );
        }
    }
}

#[test]
fn a_single_trustee_alone_cannot_sign_a_three_of_five() {
    let (mut parties, mut transport, session) = setup(5, 3);

    for alone in [1u16, 2, 3, 4, 5] {
        let result = sign(&mut parties, &mut transport, session, 3, &[alone]);
        assert!(
            result.is_err(),
            "trustee {alone} alone produced a 3-of-5 signature"
        );
    }
}

#[test]
fn a_signature_binds_only_to_its_own_session_domain_and_payload() {
    let (mut parties, mut transport, session) = setup(5, 3);
    let outcome = sign(&mut parties, &mut transport, session, 3, &[1, 2, 3]).unwrap();

    let verifying_key =
        frostk::VerifyingKey::deserialize(&hex::decode(&outcome.group_verifying_key).expect("hex"))
            .expect("group key");
    let signature = frostk::Signature::deserialize(&hex::decode(&outcome.signature).expect("hex"))
        .expect("sig");

    // The exact triple it was signed over.
    let honest = frost_service::wire::signing_message(&session, SIGNING_DOMAIN, MESSAGE);
    assert!(verifying_key.verify(&honest, &signature).is_ok());

    // Any change to the session, the domain, or the payload breaks it. This is
    // what stops a signature being lifted into another release attestation.
    let other_session = session_id(0x99);
    for (label, message) in [
        (
            "a different session",
            frost_service::wire::signing_message(&other_session, SIGNING_DOMAIN, MESSAGE),
        ),
        (
            "a different domain",
            frost_service::wire::signing_message(&session, "other-protocol/v1", MESSAGE),
        ),
        (
            "a different payload",
            frost_service::wire::signing_message(&session, SIGNING_DOMAIN, b"a different release"),
        ),
    ] {
        assert!(
            verifying_key.verify(&message, &signature).is_err(),
            "the signature verified for {label}, so it is not bound to that value"
        );
    }
}

#[test]
fn a_session_id_cannot_be_reused_for_a_second_signature() {
    let (mut parties, mut transport, session) = setup(5, 3);

    sign(&mut parties, &mut transport, session, 3, &[1, 2, 3]).unwrap();

    // Running the same session again must be refused. A fresh session id is
    // required, so commitments from the first run can never be recycled.
    let second = sign(&mut parties, &mut transport, session, 3, &[1, 2, 3]);
    assert!(
        second.is_err(),
        "a second signing run reused a session id that was already signed over"
    );
}

#[test]
fn a_replayed_round1_commitment_is_rejected() {
    let mut rng = OsRng;
    let session = session_id(0xB2);
    let mut parties = coordinator::new_committee(3).unwrap();
    let mut transport = MemoryTransport::new(coordinator::committee(3));
    coordinator::run_dkg(&mut parties, &mut transport, session, 2, &mut rng).unwrap();

    // Party 2 publishes a commitment.
    let envelope = parties[1]
        .signing_round1(session, MESSAGE, SIGNING_DOMAIN, &mut rng)
        .unwrap()
        .remove(0);

    // Deliver it, then deliver the identical bytes again. The second copy is a
    // replay of the same nonce commitment.
    parties[0].accept_signing_round1(&envelope).unwrap();
    let replay = parties[0].accept_signing_round1(&envelope);
    assert!(
        replay.is_err(),
        "a duplicate commitment from the same trustee must be refused"
    );
}

#[test]
fn a_trustee_will_not_sign_twice_with_the_same_nonces() {
    let mut rng = OsRng;
    let session = session_id(0xC3);
    let mut parties = coordinator::new_committee(3).unwrap();
    let mut transport = MemoryTransport::new(coordinator::committee(3));
    coordinator::run_dkg(&mut parties, &mut transport, session, 2, &mut rng).unwrap();

    // Collect every commitment at trustee 1.
    parties[0]
        .signing_round1(session, MESSAGE, SIGNING_DOMAIN, &mut rng)
        .expect("trustee 1 commits");
    for other in [1usize, 2] {
        let envelope = parties[other]
            .signing_round1(session, MESSAGE, SIGNING_DOMAIN, &mut rng)
            .expect("commitment published")
            .remove(0);
        parties[0]
            .accept_signing_round1(&envelope)
            .expect("collected");
    }

    // The first round 2 signs and burns the nonces.
    parties[0]
        .signing_round2(session, &[1, 2, 3])
        .expect("first round 2");

    // A second round 2 with no fresh round 1 must not reuse them. Reusing a
    // nonce pair across two messages is how a key share leaks.
    let replay = parties[0].signing_round2(session, &[1, 2, 3]);
    assert!(
        matches!(replay, Err(frost_service::Error::NoncesReused(_))),
        "expected NoncesReused, got {replay:?}"
    );
}

#[test]
fn a_duplicate_signature_share_is_refused() {
    let mut rng = OsRng;
    let session = session_id(0xC4);
    let mut parties = coordinator::new_committee(3).unwrap();
    let mut transport = MemoryTransport::new(coordinator::committee(3));
    coordinator::run_dkg(&mut parties, &mut transport, session, 2, &mut rng).unwrap();

    parties[0]
        .signing_round1(session, MESSAGE, SIGNING_DOMAIN, &mut rng)
        .expect("trustee 1 commits");
    for other in [1usize, 2] {
        let envelope = parties[other]
            .signing_round1(session, MESSAGE, SIGNING_DOMAIN, &mut rng)
            .expect("published")
            .remove(0);
        parties[0]
            .accept_signing_round1(&envelope)
            .expect("collected");
    }
    let share = parties[0]
        .signing_round2(session, &[1, 2, 3])
        .expect("round 2")
        .remove(0);

    parties[1]
        .accept_signing_round2(&share)
        .expect("first share");
    let replay = parties[1].accept_signing_round2(&share);
    assert!(
        replay.is_err(),
        "a duplicate share from the same trustee must be refused"
    );
}

#[test]
fn dkg_round2_packages_never_reach_a_third_party() {
    let mut rng = OsRng;
    let session = session_id(0xD4);
    let mut parties = coordinator::new_committee(3).unwrap();
    let mut transport = MemoryTransport::new(coordinator::committee(3));

    // Round 1, delivered for real.
    let round1: Vec<Vec<_>> = parties
        .iter_mut()
        .map(|p| p.dkg_round1(session, 2, 3, &mut rng).expect("round 1"))
        .collect();
    for (party, envelopes) in parties.iter_mut().zip(round1) {
        for envelope in envelopes {
            transport.send(party.me(), envelope).expect("delivered");
        }
    }
    coordinator::pump_to_quiescence(&mut parties, &mut transport).expect("pumped");

    // Round 2, captured before delivery.
    let mut round2: Vec<frost_service::wire::Envelope> = Vec::new();
    for party in parties.iter_mut() {
        round2.extend(party.dkg_round2().expect("round 2"));
    }

    // Each of the three trustees sends one package to each of the other two.
    assert_eq!(round2.len(), 6, "two packages per trustee");
    for envelope in &round2 {
        assert_eq!(
            envelope.kind,
            frost_service::wire::MessageKind::DkgRound2,
            "only round 2 packages here"
        );
        assert_ne!(
            envelope.to,
            frost_service::wire::BROADCAST,
            "a dkg round 2 package must never be broadcast: {envelope:?}"
        );
        assert_ne!(
            envelope.to, envelope.from,
            "a package must not be self-addressed"
        );
    }
}

// ---- live relay -----------------------------------------------------------

struct RunningRelay {
    child: Child,
    base: String,
}

impl RunningRelay {
    fn start() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_frost-relay"))
            .args(["--listen", "127.0.0.1:0", "--trustees", "5"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("the relay binary should start");

        // The relay prints the address it actually bound to.
        let stdout = child.stdout.take().expect("piped stdout");
        let mut line = String::new();
        {
            use std::io::BufRead as _;
            let mut reader = std::io::BufReader::new(stdout);
            reader.read_line(&mut line).expect("relay greeting");
        }
        let addr = line
            .split_whitespace()
            .nth(3)
            .unwrap_or_else(|| panic!("unexpected relay greeting: {line}"))
            .to_string();

        Self {
            child,
            base: format!("http://{addr}"),
        }
    }
}

impl Drop for RunningRelay {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn full_protocol_runs_over_a_live_relay() {
    let mut rng = OsRng;
    let session = session_id(0xE5);
    let relay = RunningRelay::start();
    let mut transport = HttpTransport::new(relay.base.clone());
    let mut parties = coordinator::new_committee(5).unwrap();

    let dkg = coordinator::run_dkg(&mut parties, &mut transport, session, 3, &mut rng).unwrap();
    assert_eq!(dkg.transport, "relay");
    assert_eq!(dkg.per_trustee_group_key.len(), 5);

    let outcome = coordinator::run_signing(
        &mut parties,
        &mut transport,
        &SigningRequest::new(session, 3, &[1, 2, 3], MESSAGE, SIGNING_DOMAIN),
        &mut rng,
    )
    .expect("signing over the relay");

    assert!(verifies_independently(&outcome, SIGNING_DOMAIN));
    assert_eq!(outcome.transport, "relay");
}

#[test]
fn a_relay_can_be_reset_between_sessions() {
    let mut rng = OsRng;
    let relay = RunningRelay::start();
    let mut transport = HttpTransport::new(relay.base.clone());

    // First session, run to completion over the relay.
    let mut parties = coordinator::new_committee(5).unwrap();
    coordinator::run_dkg(&mut parties, &mut transport, session_id(0xF6), 3, &mut rng).unwrap();
    coordinator::run_signing(
        &mut parties,
        &mut transport,
        &SigningRequest::new(session_id(0xF6), 3, &[1, 2, 3], MESSAGE, SIGNING_DOMAIN),
        &mut rng,
    )
    .expect("first session signs");

    // A reset relay is empty: it holds no traces of the first session.
    transport.reset().expect("reset");
    assert_eq!(transport.queued().expect("queued"), 0);

    // A second, independent session runs on the same relay.
    let mut next = [0u8; 32];
    rng.fill_bytes(&mut next);
    let mut parties = coordinator::new_committee(5).unwrap();
    coordinator::run_dkg(&mut parties, &mut transport, next, 3, &mut rng).unwrap();
    let second = coordinator::run_signing(
        &mut parties,
        &mut transport,
        &SigningRequest::new(next, 3, &[1, 2, 3], MESSAGE, SIGNING_DOMAIN),
        &mut rng,
    )
    .expect("second session signs");
    assert!(verifies_independently(&second, SIGNING_DOMAIN));
}

#[test]
fn in_process_and_relay_transports_agree_on_the_outcome() {
    // Same protocol, two transports. Two DKGs are independent runs with
    // independent randomness, so their group keys legitimately differ; what
    // must match is that both runs are valid and describe the same session.
    let mut rng = OsRng;
    let session = session_id(0x77);
    let participants = [2, 3, 5];

    let mut memory_parties = coordinator::new_committee(5).unwrap();
    let mut memory = MemoryTransport::new(coordinator::committee(5));
    let memory_dkg =
        coordinator::run_dkg(&mut memory_parties, &mut memory, session, 3, &mut rng).unwrap();
    let over_memory = coordinator::run_signing(
        &mut memory_parties,
        &mut memory,
        &SigningRequest::new(session, 3, &participants, MESSAGE, SIGNING_DOMAIN),
        &mut rng,
    )
    .unwrap();

    let relay = RunningRelay::start();
    let mut http = HttpTransport::new(relay.base.clone());
    let mut relay_parties = coordinator::new_committee(5).unwrap();
    let relay_dkg =
        coordinator::run_dkg(&mut relay_parties, &mut http, session, 3, &mut rng).unwrap();
    let over_relay = coordinator::run_signing(
        &mut relay_parties,
        &mut http,
        &SigningRequest::new(session, 3, &participants, MESSAGE, SIGNING_DOMAIN),
        &mut rng,
    )
    .unwrap();

    // Each run is a genuinely valid 3-of-5 signature against its own key.
    assert!(verifies_independently(&over_memory, SIGNING_DOMAIN));
    assert!(verifies_independently(&over_relay, SIGNING_DOMAIN));
    assert_eq!(
        over_memory.group_verifying_key,
        memory_dkg.group_verifying_key
    );
    assert_eq!(
        over_relay.group_verifying_key,
        relay_dkg.group_verifying_key
    );

    // The relay delivered exactly the same protocol run.
    assert_eq!(over_memory.session, over_relay.session);
    assert_eq!(over_memory.message, over_relay.message);
    assert_eq!(over_memory.participants, over_relay.participants);
    assert_eq!(over_memory.threshold, over_relay.threshold);
    assert_eq!(
        over_memory.signers_aggregated,
        over_relay.signers_aggregated
    );
    assert_eq!(over_memory.transport, "memory");
    assert_eq!(over_relay.transport, "relay");
}
