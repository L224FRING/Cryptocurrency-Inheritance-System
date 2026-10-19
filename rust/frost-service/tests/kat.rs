//! Deterministic known-answer generator for the on-chain FROST verifier.
//!
//! Runs a real DKG and a real threshold signing with a fixed-seed RNG, then
//! prints every value the Solidity `FROSTVerifier` needs. Run with:
//!
//! ```sh
//! cargo test --test kat -- --nocapture
//! ```
//!
//! The captured output lives in `test/fixtures/frost_kat.json` and is what
//! `test/FROSTOnchain.t.sol` checks against.

use rand_core::{CryptoRng, RngCore};

use frost_service::coordinator::{self, SigningRequest};
use frost_service::transport::memory::MemoryTransport;

/// splitmix64: a tiny, fully specified PRNG. Not for production secrets —
/// only for reproducible test vectors.
struct SplitMix64(u64);

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self(seed)
    }
}

impl RngCore for SplitMix64 {
    fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        for chunk in dest.chunks_mut(8) {
            let bytes = self.next_u64().to_le_bytes();
            let n = chunk.len();
            chunk.copy_from_slice(&bytes[..n]);
        }
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand_core::Error> {
        self.fill_bytes(dest);
        Ok(())
    }
}

impl CryptoRng for SplitMix64 {}

#[test]
fn generate_frost_kat() {
    const TRUSTEES: u16 = 3;
    const THRESHOLD: u16 = 2;
    let session = [0x11u8; 32];
    // The vault's release hash, as raw bytes — this is what the contract signs.
    let payload = hex::decode("a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90")
        .expect("payload hex");

    let mut rng = SplitMix64::new(0xDEADBEEFCAFEF00D);
    let mut parties = coordinator::new_committee(TRUSTEES).expect("committee");
    let mut transport = MemoryTransport::new(coordinator::committee(TRUSTEES));

    let dkg = coordinator::run_dkg(&mut parties, &mut transport, session, THRESHOLD, &mut rng)
        .expect("dkg");

    // Raw mode: the trustees sign the payload bytes verbatim, which is what
    // the vault's `releaseHash` needs.
    let participants: Vec<u16> = (1..=THRESHOLD).collect();
    let outcome = coordinator::run_signing(
        &mut parties,
        &mut transport,
        &SigningRequest::new(session, THRESHOLD, &participants, &payload, "unused").raw(true),
        &mut rng,
    )
    .expect("signing");

    let signature = hex::decode(&outcome.signature).expect("sig hex");
    assert_eq!(signature.len(), 65, "FROST signature is 65 bytes");

    let kat = serde_json::json!({
        "session": hex::encode(session),
        "trustees": TRUSTEES,
        "threshold": THRESHOLD,
        "group_verifying_key": dkg.group_verifying_key,
        "payload": hex::encode(&payload),
        "signed_message": outcome.signed_message,
        "signature": outcome.signature,
        "r_prefix": format!("0x{:02x}", signature[0]),
        "r_x": format!("0x{}", hex::encode(&signature[1..33])),
        "z": format!("0x{}", hex::encode(&signature[33..65])),
    });
    println!("{}", serde_json::to_string_pretty(&kat).expect("json"));

    // The message actually signed must be the raw payload, not a wrapped form.
    assert_eq!(
        outcome.signed_message,
        hex::encode(&payload),
        "raw mode must sign the payload verbatim"
    );

    // Golden values. If these move, either the splitmix64 seed changed or the
    // FROST ciphersuite produced different bytes — either way the Solidity
    // fixture must be regenerated before the on-chain test can be trusted.
    assert_eq!(
        dkg.group_verifying_key,
        "037c2de6c714cc97e1575c17108305e37b6508e55fc16d724b9cceeaa16314a3d8",
        "group key drifted"
    );
    assert_eq!(
        outcome.signature,
        "0352cf9dc6b1fd9ea023fcbd7e1ad627e901c2217d295420fe2773d08eb083fe45fb67cd0a5917e7bc98b047652a5cd84a6462f74923c27be708471b62f5b3696f",
        "signature drifted"
    );
}
