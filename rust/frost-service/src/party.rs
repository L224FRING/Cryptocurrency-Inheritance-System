use std::collections::{BTreeMap, HashSet};

use frost_core as frost;
use frost_secp256k1 as frostk;
use rand_core::{CryptoRng, RngCore};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::wire::{signing_message, Envelope, MessageKind, SessionId, BROADCAST};

type Id = frostk::Identifier;
type DkgRound1Package = frostk::keys::dkg::round1::Package;
type DkgRound2Package = frostk::keys::dkg::round2::Package;

/// A trustee's own numbers, kept alongside the FROST identifier because
/// `Identifier` has no `Display` impl and error messages need the index.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TrusteeId(pub u16);

impl std::fmt::Display for TrusteeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TrusteeId {
    pub fn new(index: u16) -> Result<Self> {
        if index == 0 {
            return Err(Error::BadTrustee("trustee indices start at 1".to_string()));
        }
        Ok(Self(index))
    }

    pub fn index(self) -> u16 {
        self.0
    }

    pub fn identifier(self) -> Result<Id> {
        Id::try_from(self.0)
            .map_err(|e| Error::BadTrustee(format!("trustee {} rejected: {e}", self.0)))
    }
}

struct DkgState {
    session: SessionId,
    max_signers: u16,
    round1_secret: Option<frostk::keys::dkg::round1::SecretPackage>,
    round1_from: BTreeMap<Id, DkgRound1Package>,
    round2_secret: Option<frostk::keys::dkg::round2::SecretPackage>,
    round2_from: BTreeMap<Id, DkgRound2Package>,
}

struct SigningState {
    nonces: frostk::round1::SigningNonces,
    commitments: frostk::round1::SigningCommitments,
    message: Vec<u8>,
}

/// One trustee, holding only its own secret material.
///
/// A `Party` is a single participant's view of the protocol. It never sees
/// another trustee's secrets, and no group secret is ever assembled here, at
/// setup or at signing, exactly as the FROST security proof requires.
pub struct Party {
    me: TrusteeId,
    id: Id,
    /// Maps every trustee's FROST identifier back to its index. Needed because
    /// the ciphersuite offers no `Identifier -> u16` conversion.
    roster: BTreeMap<Id, u16>,
    dkg: Option<DkgState>,
    key_package: Option<frostk::keys::KeyPackage>,
    public_key_package: Option<frostk::keys::PublicKeyPackage>,
    pending: BTreeMap<SessionId, SigningState>,
    signing_commitments: BTreeMap<SessionId, BTreeMap<Id, frostk::round1::SigningCommitments>>,
    signature_shares: BTreeMap<SessionId, BTreeMap<Id, frostk::round2::SignatureShare>>,
    /// Commitment sets already consumed. FROST nonces must never sign twice:
    /// a repeat leaks the signing share. This is a hard stop, not a warning.
    spent_commitments: HashSet<Vec<u8>>,
}

impl Party {
    /// `committee` is the full roster of trustee indices, 1-based and unique.
    pub fn new(index: u16, committee: &[u16]) -> Result<Self> {
        let me = TrusteeId::new(index)?;
        if !committee.contains(&index) {
            return Err(Error::BadTrustee(format!(
                "trustee {index} is not on the committee"
            )));
        }
        let mut roster = BTreeMap::new();
        for member in committee {
            roster.insert(TrusteeId::new(*member)?.identifier()?, *member);
        }
        Ok(Self {
            id: me.identifier()?,
            me,
            roster,
            dkg: None,
            key_package: None,
            public_key_package: None,
            pending: BTreeMap::new(),
            signing_commitments: BTreeMap::new(),
            signature_shares: BTreeMap::new(),
            spent_commitments: HashSet::new(),
        })
    }

    /// This trustee's index. Identifiers have no `Display`, so error messages
    /// and envelope addressing both go through the index.
    pub fn me(&self) -> u16 {
        self.me.index()
    }

    pub fn trustee_id(&self) -> TrusteeId {
        self.me
    }

    pub fn identifier(&self) -> Id {
        self.id
    }

    pub fn has_key_package(&self) -> bool {
        self.key_package.is_some()
    }

    pub fn public_key_package(&self) -> Option<&frostk::keys::PublicKeyPackage> {
        self.public_key_package.as_ref()
    }

    pub fn group_verifying_key(&self) -> Result<Vec<u8>> {
        self.public_key_package
            .as_ref()
            .ok_or_else(|| Error::NotReady("no public key package yet".to_string()))?
            .verifying_key()
            .serialize()
            .map_err(|e| Error::Rejected(format!("group key serialization failed: {e}")))
    }

    fn index_of(&self, id: &Id) -> Result<u16> {
        self.roster
            .get(id)
            .copied()
            .ok_or_else(|| Error::BadTrustee(format!("{id:?} is not on the committee")))
    }

    // ---- DKG -------------------------------------------------------------

    /// DKG round 1: draw a secret polynomial and publish its commitment.
    pub fn dkg_round1<R: RngCore + CryptoRng>(
        &mut self,
        session: SessionId,
        min_signers: u16,
        max_signers: u16,
        rng: &mut R,
    ) -> Result<Vec<Envelope>> {
        if min_signers == 0 || min_signers > max_signers {
            return Err(Error::BadTrustee(format!(
                "threshold {min_signers} is not within 1..={max_signers}"
            )));
        }
        if self.dkg.is_some() {
            return Err(Error::NotReady(format!(
                "trustee {} is already running a DKG",
                self.me
            )));
        }

        let (secret, package) = frostk::keys::dkg::part1(self.id, max_signers, min_signers, rng)?;
        let body = package
            .serialize()
            .map_err(|e| Error::Rejected(format!("dkg round 1 serialization failed: {e}")))?;

        self.dkg = Some(DkgState {
            session,
            max_signers,
            round1_secret: Some(secret),
            round1_from: BTreeMap::new(),
            round2_secret: None,
            round2_from: BTreeMap::new(),
        });

        Ok(vec![Envelope::new(
            session,
            MessageKind::DkgRound1,
            self.me(),
            BROADCAST,
            body,
        )])
    }

    pub fn accept_dkg_round1(&mut self, envelope: &Envelope) -> Result<()> {
        let session = self.dkg_session()?;
        envelope.check(session, self.me())?;
        if envelope.kind != MessageKind::DkgRound1 {
            return Err(Error::UnexpectedEnvelope(format!(
                "expected dkg_round1, got {}",
                envelope.kind.as_str()
            )));
        }
        let sender = TrusteeId::new(envelope.from)?.identifier()?;
        let package = DkgRound1Package::deserialize(&envelope.decode_body()?)
            .map_err(|e| Error::Rejected(format!("dkg round 1 package rejected: {e}")))?;

        let state = self.dkg.as_mut().expect("session checked above");
        if state.round1_from.insert(sender, package).is_some() {
            return Err(Error::UnexpectedEnvelope(format!(
                "trustee {} sent two dkg_round1 packages",
                envelope.from
            )));
        }
        Ok(())
    }

    /// DKG round 2: turn each peer's round-1 commitment into a share addressed
    /// to that peer alone. These packages are confidential per recipient, so
    /// the transport must not be able to fan one out.
    pub fn dkg_round2(&mut self) -> Result<Vec<Envelope>> {
        let state = self
            .dkg
            .as_mut()
            .ok_or_else(|| Error::NotReady("no DKG in progress".to_string()))?;

        let expected = state.max_signers as usize - 1;
        if state.round1_from.len() != expected {
            return Err(Error::NotReady(format!(
                "trustee {} holds {} of {expected} dkg_round1 packages",
                self.me,
                state.round1_from.len()
            )));
        }

        // `part2` consumes the round-1 secret, so the state is taken here. That
        // also makes a second call to this function impossible: a re-run would
        // reuse round-1 randomness and break DKG.
        let secret = state
            .round1_secret
            .take()
            .ok_or_else(|| Error::NotReady("dkg round 2 has already run".to_string()))?;
        let (secret, packages) = frostk::keys::dkg::part2(secret, &state.round1_from)
            .map_err(|e| Error::Rejected(format!("dkg round 2 failed: {e}")))?;
        state.round2_secret = Some(secret);
        let session = state.session;

        let mut out = Vec::new();
        for (recipient, package) in packages {
            let to = self.index_of(&recipient)?;
            let body = package
                .serialize()
                .map_err(|e| Error::Rejected(format!("dkg round 2 serialization failed: {e}")))?;
            out.push(Envelope::new(
                session,
                MessageKind::DkgRound2,
                self.me(),
                to,
                body,
            ));
        }
        Ok(out)
    }

    pub fn accept_dkg_round2(&mut self, envelope: &Envelope) -> Result<()> {
        let session = self.dkg_session()?;
        envelope.check(session, self.me())?;
        if envelope.kind != MessageKind::DkgRound2 {
            return Err(Error::UnexpectedEnvelope(format!(
                "expected dkg_round2, got {}",
                envelope.kind.as_str()
            )));
        }
        let sender = TrusteeId::new(envelope.from)?.identifier()?;
        let package = DkgRound2Package::deserialize(&envelope.decode_body()?)
            .map_err(|e| Error::Rejected(format!("dkg round 2 package rejected: {e}")))?;

        let state = self.dkg.as_mut().expect("session checked above");
        if state.round2_from.insert(sender, package).is_some() {
            return Err(Error::UnexpectedEnvelope(format!(
                "trustee {} sent two dkg_round2 packages",
                envelope.from
            )));
        }
        Ok(())
    }

    /// DKG part 3: combine everyone's shares into this trustee's long-lived key
    /// share. Every trustee must independently arrive at the same group key.
    pub fn dkg_finish(&mut self) -> Result<Vec<u8>> {
        let expected = {
            let state = self
                .dkg
                .as_ref()
                .ok_or_else(|| Error::NotReady("no DKG in progress".to_string()))?;
            state.max_signers as usize - 1
        };

        let (round2_secret, round1_from, round2_from) = {
            let state = self.dkg.as_ref().expect("checked above");
            if state.round2_from.len() != expected {
                return Err(Error::NotReady(format!(
                    "trustee {} holds {} of {expected} dkg_round2 packages",
                    self.me,
                    state.round2_from.len()
                )));
            }
            let secret = state
                .round2_secret
                .clone()
                .ok_or_else(|| Error::NotReady("dkg round 2 has not run".to_string()))?;
            (secret, state.round1_from.clone(), state.round2_from.clone())
        };

        let (key_package, public_key_package) =
            frostk::keys::dkg::part3(&round2_secret, &round1_from, &round2_from)
                .map_err(|e| Error::Rejected(format!("dkg part 3 failed: {e}")))?;

        self.group_verifying_key_of(&public_key_package)?;
        self.key_package = Some(key_package);
        self.public_key_package = Some(public_key_package);
        self.dkg = None;
        self.group_verifying_key()
    }

    fn group_verifying_key_of(&self, package: &frostk::keys::PublicKeyPackage) -> Result<Vec<u8>> {
        package
            .verifying_key()
            .serialize()
            .map_err(|e| Error::Rejected(format!("group key serialization failed: {e}")))
    }

    fn dkg_session(&self) -> Result<SessionId> {
        self.dkg
            .as_ref()
            .map(|s| s.session)
            .ok_or_else(|| Error::NotReady("no DKG in progress".to_string()))
    }

    // ---- Signing ---------------------------------------------------------

    /// Signing round 1: commit to a nonce pair.
    pub fn signing_round1<R: RngCore + CryptoRng>(
        &mut self,
        session: SessionId,
        payload: &[u8],
        domain: &str,
        rng: &mut R,
    ) -> Result<Vec<Envelope>> {
        if self.key_package.is_none() {
            return Err(Error::NotReady(format!(
                "trustee {} has no key share; run a DKG first",
                self.me
            )));
        }

        let key_package = self.key_package.as_ref().expect("checked above");
        let (nonces, commitments) = frostk::round1::commit(key_package.signing_share(), rng);

        let fingerprint = commitments_fingerprint(&commitments);
        if self.spent_commitments.contains(&fingerprint) {
            return Err(Error::NoncesReused(format!(
                "trustee {} was handed an already-spent commitment set",
                self.me
            )));
        }

        let message = signing_message(&session, domain, payload);
        let body = commitments.serialize().map_err(|e| {
            Error::Rejected(format!("signing commitment serialization failed: {e}"))
        })?;

        // The relay never echoes a broadcast back to its sender, so record our
        // own commitment here. The aggregator needs the complete commitment
        // set to verify what it is about to sign.
        self.signing_commitments
            .entry(session)
            .or_default()
            .insert(self.id, commitments);

        self.pending.insert(
            session,
            SigningState {
                nonces,
                commitments,
                message,
            },
        );

        Ok(vec![Envelope::new(
            session,
            MessageKind::SigningRound1,
            self.me(),
            BROADCAST,
            body,
        )])
    }

    pub fn accept_signing_round1(&mut self, envelope: &Envelope) -> Result<()> {
        if envelope.kind != MessageKind::SigningRound1 {
            return Err(Error::UnexpectedEnvelope(format!(
                "expected signing_round1, got {}",
                envelope.kind.as_str()
            )));
        }
        let sender = TrusteeId::new(envelope.from)?.identifier()?;
        let commitments = frostk::round1::SigningCommitments::deserialize(&envelope.decode_body()?)
            .map_err(|e| Error::Rejected(format!("signing commitment rejected: {e}")))?;

        // A second commitment from the same trustee is either a replay or an
        // attempt to swap the nonce set mid-session. Either way we stop, rather
        // than letting the later one silently replace the first.
        let slot = self
            .signing_commitments
            .entry(envelope.session)
            .or_default();
        if slot.contains_key(&sender) {
            return Err(Error::UnexpectedEnvelope(format!(
                "trustee {} sent two signing commitments for this session",
                envelope.from
            )));
        }
        slot.insert(sender, commitments);
        Ok(())
    }

    /// Signing round 2: produce this trustee's signature share.
    pub fn signing_round2(
        &mut self,
        session: SessionId,
        participants: &[u16],
    ) -> Result<Vec<Envelope>> {
        // Take the nonces out before doing any work. If a later step fails we
        // must not be able to come back and sign a second time with them.
        let state = self.pending.remove(&session).ok_or_else(|| {
            Error::NoncesReused(format!(
                "trustee {} has no unused nonces for this session",
                self.me
            ))
        })?;
        self.spent_commitments
            .insert(commitments_fingerprint(&state.commitments));

        let key_package = self
            .key_package
            .as_ref()
            .ok_or_else(|| Error::NotReady("no key share".to_string()))?;
        let public_key_package = self
            .public_key_package
            .as_ref()
            .ok_or_else(|| Error::NotReady("no public key package".to_string()))?;

        let mut commitments = BTreeMap::new();
        for index in participants {
            let id = TrusteeId::new(*index)?.identifier()?;
            if *index == self.me() {
                commitments.insert(id, state.commitments);
                continue;
            }
            let received = self
                .signing_commitments
                .get(&session)
                .and_then(|m| m.get(&id))
                .ok_or_else(|| {
                    Error::NotReady(format!(
                        "trustee {} is missing the commitment from trustee {index}",
                        self.me
                    ))
                })?;
            commitments.insert(id, *received);
        }

        let package = frostk::SigningPackage::new(commitments, &state.message);
        let share = frostk::round2::sign(&package, &state.nonces, key_package)
            .map_err(|e| Error::Rejected(format!("signing failed: {e}")))?;

        frost::verify_signature_share(
            self.id,
            key_package.verifying_share(),
            &share,
            &package,
            public_key_package.verifying_key(),
        )
        .map_err(|e| Error::Rejected(format!("own share failed verification: {e}")))?;

        // As with the commitment, keep our own share: the relay will not echo
        // this broadcast back, and the aggregator is usually this trustee.
        self.signature_shares
            .entry(session)
            .or_default()
            .insert(self.id, share);

        Ok(vec![Envelope::new(
            session,
            MessageKind::SigningRound2,
            self.me(),
            BROADCAST,
            share.serialize(),
        )])
    }

    pub fn accept_signing_round2(&mut self, envelope: &Envelope) -> Result<()> {
        if envelope.kind != MessageKind::SigningRound2 {
            return Err(Error::UnexpectedEnvelope(format!(
                "expected signing_round2, got {}",
                envelope.kind.as_str()
            )));
        }
        let sender = TrusteeId::new(envelope.from)?.identifier()?;
        let share = frostk::round2::SignatureShare::deserialize(&envelope.decode_body()?)
            .map_err(|e| Error::Rejected(format!("signature share rejected: {e}")))?;
        let slot = self.signature_shares.entry(envelope.session).or_default();
        if slot.contains_key(&sender) {
            return Err(Error::UnexpectedEnvelope(format!(
                "trustee {} sent two signature shares for this session",
                envelope.from
            )));
        }
        slot.insert(sender, share);
        Ok(())
    }

    /// Combine shares from at least `min_signers` trustees into one signature
    /// and check it against the group key.
    pub fn aggregate(
        &self,
        session: SessionId,
        participants: &[u16],
        min_signers: u16,
        payload: &[u8],
        domain: &str,
    ) -> Result<frostk::Signature> {
        let public_key_package = self
            .public_key_package
            .as_ref()
            .ok_or_else(|| Error::NotReady("no public key package".to_string()))?;

        if (participants.len() as u16) < min_signers {
            return Err(Error::NotReady(format!(
                "{} participants cannot meet a threshold of {min_signers}",
                participants.len()
            )));
        }

        let mut commitments = BTreeMap::new();
        let mut shares = BTreeMap::new();
        for index in participants {
            let id = TrusteeId::new(*index)?.identifier()?;
            let commitment = *self
                .signing_commitments
                .get(&session)
                .and_then(|m| m.get(&id))
                .ok_or_else(|| {
                    Error::NotReady(format!("missing commitment from trustee {index}"))
                })?;
            let share = *self
                .signature_shares
                .get(&session)
                .and_then(|m| m.get(&id))
                .ok_or_else(|| Error::NotReady(format!("missing share from trustee {index}")))?;
            commitments.insert(id, commitment);
            shares.insert(id, share);
        }

        let message = signing_message(&session, domain, payload);
        let package = frostk::SigningPackage::new(commitments, &message);
        let signature = frostk::aggregate(&package, &shares, public_key_package)
            .map_err(|e| Error::Rejected(format!("aggregation failed: {e}")))?;

        public_key_package
            .verifying_key()
            .verify(&message, &signature)
            .map_err(|e| {
                Error::Rejected(format!(
                    "signature failed verification against group key: {e}"
                ))
            })?;

        Ok(signature)
    }
}

/// Stable identifier for a commitment set, used to enforce one-shot nonces.
fn commitments_fingerprint(commitments: &frostk::round1::SigningCommitments) -> Vec<u8> {
    commitments.serialize().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trustee_index_zero_is_refused() {
        assert!(TrusteeId::new(0).is_err());
        assert!(TrusteeId::new(1).is_ok());
    }

    #[test]
    fn party_must_be_on_the_committee() {
        assert!(Party::new(4, &[1, 2, 3]).is_err());
        assert!(Party::new(2, &[1, 2, 3]).is_ok());
    }

    #[test]
    fn signing_before_dkg_is_refused() {
        let mut p = Party::new(1, &[1, 2, 3]).unwrap();
        let mut rng = rand_core::OsRng;
        let err = p
            .signing_round1([1u8; 32], b"payload", "domain", &mut rng)
            .unwrap_err();
        assert!(matches!(err, Error::NotReady(_)), "got {err:?}");
    }
}
