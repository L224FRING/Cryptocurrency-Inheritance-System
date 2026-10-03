use std::collections::BTreeMap;

use rand_core::{CryptoRng, RngCore};
use serde::Serialize;

use crate::error::{Error, Result};
use crate::party::Party;
use crate::transport::Transport;
use crate::wire::{Envelope, MessageKind, SessionId};

const MAX_PUMP_ROUNDS: usize = 64;

#[derive(Debug, Clone, Serialize)]
pub struct DkgOutcome {
    pub session: String,
    pub trustees: u16,
    pub threshold: u16,
    pub group_verifying_key: String,
    /// One entry per trustee. Every value must match; a mismatch means the DKG
    /// did not converge and the resulting shares are unusable.
    pub per_trustee_group_key: BTreeMap<String, String>,
    pub envelopes_sent: usize,
    pub envelopes_delivered: usize,
    pub transport: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SigningOutcome {
    pub session: String,
    pub message: String,
    pub participants: Vec<u16>,
    pub threshold: u16,
    pub group_verifying_key: String,
    pub signature: String,
    pub verified_against_group_key: bool,
    pub signers_aggregated: u16,
    pub envelopes_sent: usize,
    pub envelopes_delivered: usize,
    pub transport: String,
}

pub fn committee(max_signers: u16) -> Vec<u16> {
    (1..=max_signers).collect()
}

pub fn new_committee(max_signers: u16) -> Result<Vec<Party>> {
    let roster = committee(max_signers);
    roster
        .iter()
        .map(|index| Party::new(*index, &roster))
        .collect()
}

/// Move every queued envelope to its recipient until the system is quiet.
///
/// This is the whole coordination layer: it forwards opaque bytes and knows
/// nothing about FROST. Two parties talking through it learn no more than two
/// parties talking directly.
fn pump(parties: &mut [Party], transport: &mut (impl Transport + ?Sized)) -> Result<usize> {
    let mut moved = 0usize;
    for party in parties.iter_mut() {
        for envelope in transport.inbox(party.me())? {
            moved += 1;
            dispatch(party, envelope)?;
        }
    }
    Ok(moved)
}

/// Apply one envelope to a party. Public so the per-trustee client can feed a
/// standalone party from its own relay inbox using the same rules the
/// in-process coordinator uses.
pub fn dispatch(party: &mut Party, envelope: Envelope) -> Result<()> {
    match envelope.kind {
        MessageKind::DkgRound1 => party.accept_dkg_round1(&envelope),
        MessageKind::DkgRound2 => party.accept_dkg_round2(&envelope),
        MessageKind::SigningRound1 => party.accept_signing_round1(&envelope),
        MessageKind::SigningRound2 => party.accept_signing_round2(&envelope),
    }
}

/// Pump until nothing more moves, so ordering differences between parties do
/// not matter. Bounded so a relay that keeps echoing cannot hang the process.
///
/// Public because a caller that drives the rounds itself needs the same
/// delivery rule the coordinators use.
pub fn pump_to_quiescence(
    parties: &mut [Party],
    transport: &mut (impl Transport + ?Sized),
) -> Result<()> {
    for round in 0..MAX_PUMP_ROUNDS {
        let moved = pump(parties, transport)?;
        if moved == 0 {
            return Ok(());
        }
        if round == MAX_PUMP_ROUNDS - 1 {
            return Err(Error::Stalled(format!(
                "envelopes were still moving after {MAX_PUMP_ROUNDS} sweeps"
            )));
        }
    }
    Ok(())
}

fn deliver_all(
    parties: &mut [Party],
    transport: &mut (impl Transport + ?Sized),
    outbox: Vec<Vec<Envelope>>,
) -> Result<()> {
    for (party, envelopes) in parties.iter_mut().zip(outbox) {
        for envelope in envelopes {
            transport.send(party.me(), envelope)?;
        }
    }
    Ok(())
}

/// Run the full three-part DKG across the committee.
pub fn run_dkg<R: RngCore + CryptoRng>(
    parties: &mut [Party],
    transport: &mut (impl Transport + ?Sized),
    session: SessionId,
    min_signers: u16,
    rng: &mut R,
) -> Result<DkgOutcome> {
    let max_signers = parties.len() as u16;
    if min_signers == 0 || min_signers > max_signers {
        return Err(Error::BadTrustee(format!(
            "threshold {min_signers} is not within 1..={max_signers}"
        )));
    }

    // Round 1: everyone publishes a commitment.
    let mut outbox = Vec::new();
    for party in parties.iter_mut() {
        outbox.push(party.dkg_round1(session, min_signers, max_signers, rng)?);
    }
    deliver_all(parties, transport, outbox)?;
    pump_to_quiescence(parties, transport)?;

    // Round 2: everyone derives a per-recipient share.
    let mut outbox = Vec::new();
    for party in parties.iter_mut() {
        outbox.push(party.dkg_round2()?);
    }
    deliver_all(parties, transport, outbox)?;
    pump_to_quiescence(parties, transport)?;

    // Part 3: everyone finalizes their own key share.
    let mut per_trustee_group_key = BTreeMap::new();
    for party in parties.iter_mut() {
        let key = party.dkg_finish()?;
        per_trustee_group_key.insert(party.me().to_string(), hex::encode(key));
    }

    let distinct: std::collections::BTreeSet<String> =
        per_trustee_group_key.values().cloned().collect();
    if distinct.len() != 1 {
        return Err(Error::Rejected(format!(
            "DKG did not converge: {} distinct group keys",
            distinct.len()
        )));
    }
    let group_verifying_key = distinct.into_iter().next().expect("checked len == 1");

    Ok(DkgOutcome {
        session: hex::encode(session),
        trustees: max_signers,
        threshold: min_signers,
        group_verifying_key,
        per_trustee_group_key,
        envelopes_sent: transport_sent(transport),
        envelopes_delivered: transport_delivered(transport),
        transport: transport.kind().to_string(),
    })
}

/// One signing run: which trustees take part, over what, under which session.
///
/// Grouped into a struct because a session id is single-use: a caller that
/// signs twice has to build a second request rather than tweak an argument, and
/// that is exactly the mistake worth making awkward.
#[derive(Clone, Debug)]
pub struct SigningRequest<'a> {
    pub session: SessionId,
    pub min_signers: u16,
    pub participants: &'a [u16],
    pub payload: &'a [u8],
    pub domain: &'a str,
}

impl<'a> SigningRequest<'a> {
    pub fn new(
        session: SessionId,
        min_signers: u16,
        participants: &'a [u16],
        payload: &'a [u8],
        domain: &'a str,
    ) -> Self {
        Self {
            session,
            min_signers,
            participants,
            payload,
            domain,
        }
    }
}

/// Run the two-round signing protocol over `request.participants`.
pub fn run_signing<R: RngCore + CryptoRng>(
    parties: &mut [Party],
    transport: &mut (impl Transport + ?Sized),
    request: &SigningRequest<'_>,
    rng: &mut R,
) -> Result<SigningOutcome> {
    let SigningRequest {
        session,
        min_signers,
        participants,
        payload,
        domain,
    } = *request;

    if (participants.len() as u16) < min_signers {
        return Err(Error::NotReady(format!(
            "{} participants cannot meet a threshold of {min_signers}",
            participants.len()
        )));
    }
    for index in participants {
        let party = parties
            .iter()
            .find(|p| p.me() == *index)
            .ok_or_else(|| Error::BadTrustee(format!("trustee {index} has no party")))?;
        if !party.has_key_package() {
            return Err(Error::NotReady(format!(
                "trustee {index} has no key share; run a DKG first"
            )));
        }
    }

    let group_verifying_key = parties
        .iter()
        .find(|p| p.me() == participants[0])
        .expect("checked above")
        .group_verifying_key()?;

    // Round 1: commitments from the participating subset only.
    let mut outbox = Vec::new();
    for party in parties.iter_mut() {
        if participants.contains(&party.me()) {
            outbox.push(party.signing_round1(session, payload, domain, rng)?);
        } else {
            outbox.push(Vec::new());
        }
    }
    deliver_all(parties, transport, outbox)?;
    pump_to_quiescence(parties, transport)?;

    // Round 2: one share per participant, then aggregate and verify.
    let mut outbox = Vec::new();
    for party in parties.iter_mut() {
        if participants.contains(&party.me()) {
            outbox.push(party.signing_round2(session, participants)?);
        } else {
            outbox.push(Vec::new());
        }
    }
    deliver_all(parties, transport, outbox)?;
    pump_to_quiescence(parties, transport)?;

    let aggregator = parties
        .iter()
        .find(|p| p.me() == participants[0])
        .expect("checked above");
    let signature = aggregator.aggregate(session, participants, min_signers, payload, domain)?;
    let signature_bytes = signature
        .serialize()
        .map_err(|e| Error::Rejected(format!("signature serialization failed: {e}")))?;

    Ok(SigningOutcome {
        session: hex::encode(session),
        message: hex::encode(payload),
        participants: participants.to_vec(),
        threshold: min_signers,
        group_verifying_key: hex::encode(group_verifying_key),
        signature: hex::encode(signature_bytes),
        verified_against_group_key: true,
        signers_aggregated: participants.len() as u16,
        envelopes_sent: transport_sent(transport),
        envelopes_delivered: transport_delivered(transport),
        transport: transport.kind().to_string(),
    })
}

fn transport_sent(transport: &(impl Transport + ?Sized)) -> usize {
    transport.sent()
}

fn transport_delivered(transport: &(impl Transport + ?Sized)) -> usize {
    transport.delivered()
}
