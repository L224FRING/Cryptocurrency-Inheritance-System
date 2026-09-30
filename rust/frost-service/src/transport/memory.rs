use std::collections::{BTreeMap, VecDeque};

use crate::error::{Error, Result};
use crate::transport::Transport;
use crate::wire::{Envelope, BROADCAST};

/// In-process transport. Used by the CLI's default mode and by tests; the relay
/// server speaks the same protocol, so both exercise identical party logic.
#[derive(Debug, Default)]
pub struct MemoryTransport {
    queues: BTreeMap<u16, VecDeque<Envelope>>,
    committee: Vec<u16>,
    sent: usize,
    delivered: usize,
}

impl MemoryTransport {
    /// `committee` is the full trustee roster, needed to expand broadcasts.
    pub fn new(committee: Vec<u16>) -> Self {
        Self {
            queues: BTreeMap::new(),
            committee,
            sent: 0,
            delivered: 0,
        }
    }

    pub fn sent(&self) -> usize {
        self.sent
    }

    pub fn delivered(&self) -> usize {
        self.delivered
    }

    fn enqueue(&mut self, to: u16, envelope: Envelope) {
        self.queues.entry(to).or_default().push_back(envelope);
    }
}

impl Transport for MemoryTransport {
    fn send(&mut self, from: u16, envelope: Envelope) -> Result<()> {
        if envelope.from != from {
            return Err(Error::UnexpectedEnvelope(format!(
                "trustee {from} tried to send an envelope claiming to be from {}",
                envelope.from
            )));
        }
        if envelope.to == BROADCAST {
            for trustee in self.committee.clone() {
                if trustee == from {
                    continue;
                }
                self.enqueue(trustee, envelope.clone());
                self.sent += 1;
            }
        } else {
            self.enqueue(envelope.to, envelope);
            self.sent += 1;
        }
        Ok(())
    }

    fn inbox(&mut self, trustee: u16) -> Result<Vec<Envelope>> {
        let taken: Vec<Envelope> = self
            .queues
            .get_mut(&trustee)
            .map(|q| q.drain(..).collect())
            .unwrap_or_default();
        self.delivered += taken.len();
        Ok(taken)
    }

    fn kind(&self) -> &'static str {
        "memory"
    }

    fn sent(&self) -> usize {
        self.sent
    }

    fn delivered(&self) -> usize {
        self.delivered
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::{MessageKind, SessionId};

    fn env(from: u16, to: u16) -> Envelope {
        Envelope::new(
            [1u8; 32],
            MessageKind::DkgRound1,
            from,
            to,
            vec![from as u8],
        )
    }

    #[test]
    fn broadcast_reaches_everyone_but_the_sender() {
        let mut t = MemoryTransport::new(vec![1, 2, 3]);
        t.send(1, env(1, BROADCAST)).unwrap();

        assert_eq!(t.inbox(1).unwrap().len(), 0);
        assert_eq!(t.inbox(2).unwrap().len(), 1);
        assert_eq!(t.inbox(3).unwrap().len(), 1);
    }

    #[test]
    fn direct_send_is_private() {
        let mut t = MemoryTransport::new(vec![1, 2, 3]);
        t.send(2, env(2, 3)).unwrap();

        assert_eq!(t.inbox(1).unwrap().len(), 0);
        assert_eq!(t.inbox(2).unwrap().len(), 0);
        assert_eq!(t.inbox(3).unwrap().len(), 1);
    }

    #[test]
    fn inbox_is_drained_on_read() {
        let mut t = MemoryTransport::new(vec![1, 2]);
        t.send(1, env(1, 2)).unwrap();

        assert_eq!(t.inbox(2).unwrap().len(), 1);
        assert_eq!(
            t.inbox(2).unwrap().len(),
            0,
            "envelopes must not be redelivered"
        );
    }

    #[test]
    fn sender_cannot_forge_its_identity() {
        let mut t = MemoryTransport::new(vec![1, 2]);
        assert!(t.send(1, env(2, 1)).is_err());
    }

    #[test]
    fn round2_packages_stay_confidential() {
        // Trustee 2 sends a DKG round-2 package meant only for trustee 3.
        let mut t = MemoryTransport::new(vec![1, 2, 3]);
        let pkg = Envelope::new([2u8; 32], MessageKind::DkgRound2, 2, 3, vec![0xde, 0xad]);
        t.send(2, pkg).unwrap();

        assert_eq!(t.inbox(1).unwrap().len(), 0, "trustee 1 must not see it");
        assert_eq!(t.inbox(3).unwrap().len(), 1);
    }

    #[test]
    fn unknown_trustee_gets_empty_inbox() {
        let mut t = MemoryTransport::new(vec![1, 2]);
        let got: Vec<Envelope> = t.inbox(99).unwrap();
        assert!(got.is_empty());
    }

    #[test]
    fn session_id_survives_transport() {
        let session: SessionId = [0xab; 32];
        let mut t = MemoryTransport::new(vec![1, 2]);
        t.send(
            1,
            Envelope::new(session, MessageKind::SigningRound1, 1, BROADCAST, vec![1]),
        )
        .unwrap();
        let got = t.inbox(2).unwrap();
        assert_eq!(got[0].session, session);
    }
}
