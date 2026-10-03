//! Off-chain FROST (secp256k1) threshold signing for the inheritance vault.
//!
//! The crate is split into a protocol core and the things that move bytes for
//! it:
//!
//! - [`party`] is one trustee. It holds only its own key share and never
//!   assembles a group secret, at setup or at signing.
//! - [`wire`] is the message format. Every message is bound to a [`wire::SessionId`]
//!   so nothing from a previous session can be replayed into a new one.
//! - [`transport`] carries messages between parties. The relay is untrusted and
//!   verifies nothing; correctness rests on FROST's own package verification.
//! - [`coordinator`] drives a session by pumping envelopes until quiet.
//!
//! Setup is a three-part DKG over the full committee. Signing is the two-round
//! FROST protocol over any subset of at least the threshold size.

pub mod coordinator;
pub mod error;
pub mod party;
pub mod report;
pub mod transport;
pub mod wire;

pub use error::{Error, Result};
pub use party::{Party, TrusteeId};
pub use wire::{Envelope, MessageKind, SessionId};

pub use frost_core;
pub use frost_secp256k1;
pub use rand_core;

pub mod vdf;
pub use vdf::{VDFParams, VDFProof, VDFResult, compute_vdf, compute_vdf_with_proof, verify_vdf_pietrzak, generate_rsa_modulus};
pub mod persistence;
pub mod attestation;
