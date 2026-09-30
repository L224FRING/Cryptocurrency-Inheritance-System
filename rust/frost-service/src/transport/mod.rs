pub mod http;
pub mod memory;

use crate::error::Result;
use crate::wire::Envelope;

/// Carries envelopes between trustees who hold no shared memory and, in the
/// deployed case, may not even be in the same process.
///
/// The relay is deliberately untrusted: it routes opaque bytes and is not
/// assumed to be honest. Correctness of the protocol must not depend on it,
/// because FROST's own verification rejects any package that does not belong to
/// the sender it claims. Availability does depend on it, which is why the
/// coordinator detects a stalled session rather than hanging.
pub trait Transport {
    /// Deliver one envelope. A broadcast is expanded to every other trustee.
    fn send(&mut self, from: u16, envelope: Envelope) -> Result<()>;

    /// Take everything queued for `trustee`, oldest first. The relay clears the
    /// queue on read, so a party sees each envelope at most once.
    fn inbox(&mut self, trustee: u16) -> Result<Vec<Envelope>>;

    /// A short label for reports, e.g. "memory" or "relay".
    fn kind(&self) -> &'static str;

    /// Envelopes handed to the transport so far.
    fn sent(&self) -> usize;

    /// Envelopes handed back to parties so far.
    fn delivered(&self) -> usize;
}
