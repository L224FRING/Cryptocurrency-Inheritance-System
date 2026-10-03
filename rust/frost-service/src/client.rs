//! Per-trustee drivers for the deployed case.
//!
//! The in-process [`crate::coordinator`] owns every party at once, which is
//! perfect for tests and for a one-shot demo but hides the trust boundary: one
//! process holds every share. This module drives a *single* [`Party`] across an
//! untrusted transport, where the other trustees are separate processes that
//! hold their own shares.
//!
//! The shape is the same either way: post this party's round-1 messages, poll
//! the inbox until enough peers have answered, then post the round-2 messages.
//! A standalone client cannot call `pump_to_quiescence` because it cannot see
//! the other parties' queues, so it polls its own inbox with a timeout and
//! fails loudly rather than hanging if a peer never shows up.

use std::thread::sleep;
use std::time::{Duration, Instant};

use rand_core::{CryptoRng, RngCore};

use crate::coordinator::dispatch;
use crate::error::{Error, Result};
use crate::party::Party;
use crate::transport::Transport;
use crate::wire::SessionId;

/// How often a waiting client re-checks its inbox.
const POLL_INTERVAL: Duration = Duration::from_millis(25);

/// Drain this party's inbox into the party once. Returns how many envelopes
/// were applied, mirroring the coordinator's per-party step.
pub fn pump_once(party: &mut Party, transport: &mut (impl Transport + ?Sized)) -> Result<usize> {
    let mut moved = 0usize;
    for envelope in transport.inbox(party.me())? {
        dispatch(party, envelope)?;
        moved += 1;
    }
    Ok(moved)
}

/// Poll until `ready` holds, draining the inbox on every pass.
///
/// Returns [`Error::Stalled`] on timeout so a missing trustee is an explicit
/// failure, never a silent hang.
pub fn wait_until<F>(
    party: &mut Party,
    transport: &mut (impl Transport + ?Sized),
    timeout: Duration,
    what: &str,
    ready: F,
) -> Result<()>
where
    F: Fn(&Party) -> bool,
{
    let start = Instant::now();
    loop {
        pump_once(party, transport)?;
        if ready(party) {
            return Ok(());
        }
        if start.elapsed() >= timeout {
            return Err(Error::Stalled(format!(
                "timed out after {:?} waiting for {what}",
                timeout
            )));
        }
        sleep(POLL_INTERVAL);
    }
}

/// Run one trustee's leg of the three-part DKG over a relay.
///
/// Every trustee must run this concurrently over the same session; the function
/// returns this trustee's newly minted key share as its serialized group
/// verifying key, ready to be persisted with [`Party::export_share`].
pub fn run_dkg_party<R: RngCore + CryptoRng>(
    party: &mut Party,
    transport: &mut (impl Transport + ?Sized),
    session: SessionId,
    threshold: u16,
    max_signers: u16,
    timeout: Duration,
    rng: &mut R,
) -> Result<Vec<u8>> {
    let expected = max_signers as usize - 1;
    let me = party.me();

    // Round 1: publish a commitment, then collect everyone else's.
    for envelope in party.dkg_round1(session, threshold, max_signers, rng)? {
        transport.send(me, envelope)?;
    }
    wait_until(party, transport, timeout, "dkg round-1 packages", |p| {
        p.dkg_round1_count() == expected
    })?;

    // Round 2: derive a share for each peer and send it to that peer alone.
    for envelope in party.dkg_round2()? {
        transport.send(me, envelope)?;
    }
    wait_until(party, transport, timeout, "dkg round-2 packages", |p| {
        p.dkg_round2_count() == expected
    })?;

    // Part 3: finalize this trustee's share.
    party.dkg_finish()
}

/// Run one trustee's leg of the two-round FROST signing protocol.
///
/// Returns `Some(serialized signature)` for the aggregating trustee and `None`
/// for the others. Exactly one participant should be the aggregator; the rest
/// may exit as soon as they have posted their round-2 share, because the relay
/// has accepted it before returning.
#[allow(clippy::too_many_arguments)]
pub fn run_signing_party<R: RngCore + CryptoRng>(
    party: &mut Party,
    transport: &mut (impl Transport + ?Sized),
    session: SessionId,
    threshold: u16,
    participants: &[u16],
    payload: &[u8],
    domain: &str,
    aggregate: bool,
    timeout: Duration,
    rng: &mut R,
) -> Result<Option<Vec<u8>>> {
    let me = party.me();
    if !participants.contains(&me) {
        return Err(Error::BadTrustee(format!(
            "trustee {me} is not among the signers {participants:?}"
        )));
    }
    let expected = participants.len();

    // Round 1: commit a nonce pair, then collect every other signer's.
    for envelope in party.signing_round1(session, payload, domain, rng)? {
        transport.send(me, envelope)?;
    }
    wait_until(party, transport, timeout, "signing commitments", |p| {
        p.signing_commitment_count(&session) >= expected
    })?;

    // Round 2: publish this trustee's partial signature.
    for envelope in party.signing_round2(session, participants)? {
        transport.send(me, envelope)?;
    }

    if !aggregate {
        return Ok(None);
    }

    // The aggregator is the only party that must stay until every share lands.
    wait_until(party, transport, timeout, "signature shares", |p| {
        p.signature_share_count(&session) >= expected
    })?;

    let signature = party.aggregate(session, participants, threshold, payload, domain)?;
    let bytes = signature
        .serialize()
        .map_err(|e| Error::Rejected(format!("signature serialization failed: {e}")))?;
    Ok(Some(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coordinator;
    use crate::transport::memory::MemoryTransport;

    /// A single party talking to a transport that already holds what it needs
    /// should drain and stop without waiting.
    #[test]
    fn wait_until_returns_as_soon_as_ready() {
        let committee = vec![1, 2, 3];
        let mut party = Party::new(1, &committee).unwrap();
        let mut transport = MemoryTransport::new(committee);
        wait_until(
            &mut party,
            &mut transport,
            Duration::from_secs(1),
            "nothing",
            |_| true,
        )
        .expect("immediately ready");
        // Sanity: the module is usable with the in-process transport too.
        let _ = coordinator::committee(3);
    }
}
