use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use crate::error::{Error, Result};
use crate::transport::Transport;
use crate::wire::{Envelope, BROADCAST};

/// Body of `POST /send`.
#[derive(Debug, Serialize, Deserialize)]
pub struct SendRequest {
    pub trustee: u16,
    pub envelopes: Vec<Envelope>,
}

/// Body of `GET /inbox/{n}` and `POST /reset`.
#[derive(Debug, Serialize, Deserialize)]
pub struct InboxResponse {
    pub envelopes: Vec<Envelope>,
}

/// Server-side queue state. Shared between the listener and the request loop.
#[derive(Debug, Default)]
pub struct RelayState {
    queues: BTreeMap<u16, Vec<Envelope>>,
    roster: Vec<u16>,
}

impl RelayState {
    pub fn new(roster: Vec<u16>) -> Self {
        Self {
            queues: BTreeMap::new(),
            roster,
        }
    }

    fn enqueue(&mut self, trustee: u16, envelope: Envelope) {
        self.queues.entry(trustee).or_default().push(envelope);
    }

    /// Accept everything one trustee sent. Broadcasts are expanded here, using
    /// the roster the relay was started with, so clients can send one message.
    pub fn accept(&mut self, trustee: u16, envelopes: Vec<Envelope>) -> Result<()> {
        for envelope in envelopes {
            if envelope.from != trustee {
                return Err(Error::BadTrustee(format!(
                    "trustee {trustee} sent an envelope claiming to be from {}",
                    envelope.from
                )));
            }
            if envelope.to == BROADCAST {
                let recipients: Vec<u16> = self
                    .roster
                    .iter()
                    .copied()
                    .filter(|r| *r != trustee)
                    .collect();
                for recipient in recipients {
                    self.enqueue(recipient, envelope.clone());
                }
            } else {
                self.enqueue(envelope.to, envelope);
            }
        }
        Ok(())
    }

    pub fn drain(&mut self, trustee: u16) -> Vec<Envelope> {
        self.queues
            .get_mut(&trustee)
            .map(std::mem::take)
            .unwrap_or_default()
    }

    pub fn reset(&mut self) {
        for queue in self.queues.values_mut() {
            queue.clear();
        }
    }

    pub fn queued(&self) -> usize {
        self.queues.values().map(|q| q.len()).sum()
    }
}

pub type SharedRelay = Arc<Mutex<RelayState>>;

/// Client half of the coordination layer. Talks to a relay it does not trust.
pub struct HttpTransport {
    base: String,
    sent: usize,
    delivered: usize,
}

impl HttpTransport {
    pub fn new(base_url: impl Into<String>) -> Self {
        let mut base = base_url.into();
        while base.ends_with('/') {
            base.pop();
        }
        Self {
            base,
            sent: 0,
            delivered: 0,
        }
    }

    pub fn reset(&self) -> Result<()> {
        let _: InboxResponse = self.post_json("/reset", &serde_json::json!({}))?;
        Ok(())
    }

    pub fn queued(&self) -> Result<usize> {
        Ok(self.get_json::<QueuedResponse>("/stats")?.queued)
    }

    fn post_json<T: Serialize, R: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &T,
    ) -> Result<R> {
        let mut response = ureq::post(&format!("{}{path}", self.base))
            .send_json(body)
            .map_err(|e| Error::Transport(format!("POST {path} failed: {e}")))?;
        response
            .body_mut()
            .read_json::<R>()
            .map_err(|e| Error::Transport(format!("POST {path} response unreadable: {e}")))
    }

    fn get_json<R: serde::de::DeserializeOwned>(&self, path: &str) -> Result<R> {
        let mut response = ureq::get(&format!("{}{path}", self.base))
            .call()
            .map_err(|e| Error::Transport(format!("GET {path} failed: {e}")))?;
        response
            .body_mut()
            .read_json::<R>()
            .map_err(|e| Error::Transport(format!("GET {path} response unreadable: {e}")))
    }
}

#[derive(Debug, Deserialize)]
struct QueuedResponse {
    queued: usize,
}

impl Transport for HttpTransport {
    fn send(&mut self, from: u16, envelope: Envelope) -> Result<()> {
        if envelope.from != from {
            return Err(Error::BadTrustee(format!(
                "trustee {from} tried to send an envelope claiming to be from {}",
                envelope.from
            )));
        }
        let body = SendRequest {
            trustee: from,
            envelopes: vec![envelope],
        };
        let _: InboxResponse = self.post_json("/send", &body)?;
        self.sent += 1;
        Ok(())
    }

    fn inbox(&mut self, trustee: u16) -> Result<Vec<Envelope>> {
        let response: InboxResponse = self.get_json(&format!("/inbox/{trustee}"))?;
        self.delivered += response.envelopes.len();
        Ok(response.envelopes)
    }

    fn kind(&self) -> &'static str {
        "relay"
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
            [3u8; 32],
            MessageKind::DkgRound1,
            from,
            to,
            vec![from as u8],
        )
    }

    fn relay() -> SharedRelay {
        Arc::new(Mutex::new(RelayState::new(vec![1, 2, 3])))
    }

    #[test]
    fn broadcast_is_expanded_by_the_relay() {
        let state = relay();
        let mut s = state.lock().unwrap();
        s.accept(1, vec![env(1, BROADCAST)]).unwrap();
        assert_eq!(s.drain(1).len(), 0);
        assert_eq!(s.drain(2).len(), 1);
        assert_eq!(s.drain(3).len(), 1);
    }

    #[test]
    fn relay_will_not_accept_a_forged_sender() {
        let state = relay();
        let mut s = state.lock().unwrap();
        assert!(s.accept(1, vec![env(2, 3)]).is_err());
    }

    #[test]
    fn round2_packages_are_not_fanned_out() {
        let state = relay();
        let mut s = state.lock().unwrap();
        let pkg = Envelope::new([1u8; 32], MessageKind::DkgRound2, 2, 3, vec![9]);
        s.accept(2, vec![pkg]).unwrap();
        assert_eq!(
            s.drain(1).len(),
            0,
            "a per-recipient package must stay private"
        );
        assert_eq!(s.drain(3).len(), 1);
    }

    #[test]
    fn drain_empties_the_queue() {
        let state = relay();
        let mut s = state.lock().unwrap();
        s.accept(1, vec![env(1, 2)]).unwrap();
        assert_eq!(s.drain(2).len(), 1);
        assert_eq!(s.drain(2).len(), 0);
    }

    #[test]
    fn reset_clears_everything() {
        let state = relay();
        let mut s = state.lock().unwrap();
        s.accept(1, vec![env(1, BROADCAST)]).unwrap();
        assert!(s.queued() > 0);
        s.reset();
        assert_eq!(s.queued(), 0);
    }

    #[test]
    fn session_id_survives_the_relay() {
        let session: SessionId = [0x5a; 32];
        let state = relay();
        let mut s = state.lock().unwrap();
        s.accept(
            1,
            vec![Envelope::new(
                session,
                MessageKind::SigningRound1,
                1,
                2,
                vec![1],
            )],
        )
        .unwrap();
        let got = s.drain(2);
        assert_eq!(got[0].session, session);
    }

    #[test]
    fn base_url_loses_trailing_slashes() {
        let t = HttpTransport::new("http://127.0.0.1:9///");
        assert_eq!(t.base, "http://127.0.0.1:9");
    }
}
