use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// Identifies one run of the protocol. Bound into every envelope and into the
/// bytes that get signed, so nothing from a previous session is ever accepted.
pub type SessionId = [u8; 32];

/// Recipient meaning "every trustee except the sender".
pub const BROADCAST: u16 = u16::MAX;

pub const SIGNING_DOMAIN: &str = "cis/frost/release-attestation/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageKind {
    DkgRound1,
    DkgRound2,
    SigningRound1,
    SigningRound2,
}

impl MessageKind {
    pub fn as_str(self) -> &'static str {
        match self {
            MessageKind::DkgRound1 => "dkg_round1",
            MessageKind::DkgRound2 => "dkg_round2",
            MessageKind::SigningRound1 => "signing_round1",
            MessageKind::SigningRound2 => "signing_round2",
        }
    }
}

/// One protocol message in flight between two trustees.
///
/// `body` is the FROST payload serialized with the ciphersuite's own encoding
/// and then hex-wrapped so the envelope stays valid JSON over the relay.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Envelope {
    pub session: SessionId,
    pub kind: MessageKind,
    pub from: u16,
    pub to: u16,
    pub body: String,
}

impl Envelope {
    pub fn new(session: SessionId, kind: MessageKind, from: u16, to: u16, body: Vec<u8>) -> Self {
        Self {
            session,
            kind,
            from,
            to,
            body: hex::encode(body),
        }
    }

    pub fn decode_body(&self) -> Result<Vec<u8>> {
        hex::decode(&self.body)
            .map_err(|e| Error::Malformed(format!("envelope body is not hex: {e}")))
    }

    /// True when this envelope is addressed to `trustee`.
    pub fn addressed_to(&self, trustee: u16) -> bool {
        self.to == trustee || self.to == BROADCAST
    }

    /// Every field a recipient can check without trusting the relay.
    pub fn check(&self, session: SessionId, trustee: u16) -> Result<()> {
        if self.session != session {
            return Err(Error::UnexpectedEnvelope(format!(
                "envelope is for session {} but this session is {}",
                hex::encode(self.session),
                hex::encode(session)
            )));
        }
        if self.to != trustee && self.to != BROADCAST {
            return Err(Error::UnexpectedEnvelope(format!(
                "envelope addressed to trustee {} landed at trustee {trustee}",
                self.to
            )));
        }
        if self.from == trustee {
            return Err(Error::UnexpectedEnvelope(format!(
                "trustee {trustee} received an envelope claiming to be from itself"
            )));
        }
        Ok(())
    }
}

/// What trustees actually sign. Serialized canonically, so the bytes a
/// signature commits to are fully determined by these fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignRequest {
    pub domain: String,
    pub session: String,
    pub payload_hash: String,
}

/// Build the message handed to FROST for signing.
///
/// Binding the session and a hash of the payload is what stops a signature
/// obtained in one session from being replayed into another.
pub fn signing_message(session: &SessionId, domain: &str, payload: &[u8]) -> Vec<u8> {
    use sha2::{Digest, Sha256};

    let request = SignRequest {
        domain: domain.to_string(),
        session: hex::encode(session),
        payload_hash: hex::encode(Sha256::digest(payload)),
    };
    serde_json::to_vec(&request).expect("SignRequest is always serializable")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(byte: u8) -> SessionId {
        [byte; 32]
    }

    #[test]
    fn signing_message_changes_with_session() {
        let a = signing_message(&session(1), SIGNING_DOMAIN, b"payload");
        let b = signing_message(&session(2), SIGNING_DOMAIN, b"payload");
        assert_ne!(a, b, "session must be bound into the signed bytes");
    }

    #[test]
    fn signing_message_changes_with_payload() {
        let a = signing_message(&session(1), SIGNING_DOMAIN, b"one");
        let b = signing_message(&session(1), SIGNING_DOMAIN, b"two");
        assert_ne!(a, b, "payload must be bound into the signed bytes");
    }

    #[test]
    fn signing_message_changes_with_domain() {
        let a = signing_message(&session(1), "domain/a", b"payload");
        let b = signing_message(&session(1), "domain/b", b"payload");
        assert_ne!(a, b, "domain must be bound into the signed bytes");
    }

    #[test]
    fn signing_message_is_stable() {
        let a = signing_message(&session(7), SIGNING_DOMAIN, b"payload");
        let b = signing_message(&session(7), SIGNING_DOMAIN, b"payload");
        assert_eq!(a, b);
    }

    #[test]
    fn envelope_rejects_foreign_session() {
        let env = Envelope::new(session(1), MessageKind::DkgRound1, 2, 1, vec![0xaa]);
        assert!(env.check(session(1), 1).is_ok());
        assert!(env.check(session(2), 1).is_err());
    }

    #[test]
    fn envelope_rejects_self_addressed() {
        let env = Envelope::new(session(1), MessageKind::DkgRound1, 1, 1, vec![0xaa]);
        assert!(env.check(session(1), 1).is_err());
    }

    #[test]
    fn envelope_rejects_wrong_recipient() {
        let env = Envelope::new(session(1), MessageKind::DkgRound1, 2, 3, vec![0xaa]);
        assert!(env.check(session(1), 3).is_ok());
        assert!(env.check(session(1), 1).is_err());
    }

    #[test]
    fn envelope_round_trips_through_json() {
        let env = Envelope::new(
            session(9),
            MessageKind::SigningRound2,
            4,
            1,
            vec![1, 2, 3, 255],
        );
        let json = serde_json::to_string(&env).unwrap();
        let back: Envelope = serde_json::from_str(&json).unwrap();
        assert_eq!(env, back);
        assert_eq!(back.decode_body().unwrap(), vec![1, 2, 3, 255]);
    }
}
