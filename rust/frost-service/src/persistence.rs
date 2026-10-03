//! On-disk representation of one trustee's key share.
//!
//! A share is what makes per-trustee operation possible: each trustee process
//! loads only its own file, so no process ever holds another trustee's secret
//! material. The committee roster and threshold are stored alongside the FROST
//! material so a standalone client can reconstruct its `Party` without being
//! told the shape of the ceremony again.

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TrusteeShare {
    pub trustee_id: u16,
    /// The full committee, so the client knows who the other parties are.
    pub committee: Vec<u16>,
    pub threshold: u16,
    /// Hex-encoded group verifying key. Kept as a checksum: loading a share
    /// whose embedded group key disagrees with this is refused.
    pub group_verifying_key: String,
    /// Serialized `frost_secp256k1::keys::KeyPackage` (the secret share).
    pub key_package: Vec<u8>,
    /// Serialized `frost_secp256k1::keys::PublicKeyPackage`.
    pub public_key_package: Vec<u8>,
}

pub fn save_trustee_share(share: &TrusteeShare, path: &str) -> Result<()> {
    let serialized = serde_json::to_string_pretty(share)
        .map_err(|e| Error::Serde(format!("serialize failed: {e}")))?;
    fs::write(path, serialized).map_err(|e| Error::Serde(format!("write failed: {e}")))?;
    Ok(())
}

pub fn load_trustee_share(path: &str) -> Result<TrusteeShare> {
    if !Path::new(path).exists() {
        return Err(Error::NotReady(format!("share not found: {path}")));
    }
    let data = fs::read_to_string(path).map_err(|e| Error::Serde(format!("read failed: {e}")))?;
    serde_json::from_str(&data).map_err(|e| Error::Serde(format!("deserialize failed: {e}")))
}
