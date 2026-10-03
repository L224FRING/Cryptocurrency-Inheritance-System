use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TrusteeShare {
    pub trustee_id: u16,
    pub key_package: Vec<u8>,
    pub public_key_package: Vec<u8>,
}

pub fn save_trustee_share(share: &TrusteeShare, path: &str) -> Result<()> {
    let serialized = serde_json::to_string_pretty(share)
        .map_err(|e| Error::Serde(format!("serialize failed: {e}")))?;
    fs::write(path, serialized)
        .map_err(|e| Error::Serde(format!("write failed: {e}")))?;
    Ok(())
}

pub fn load_trustee_share(path: &str) -> Result<TrusteeShare> {
    if !Path::new(path).exists() {
        return Err(Error::NotReady(format!("share not found: {path}")));
    }
    let data = fs::read_to_string(path)
        .map_err(|e| Error::Serde(format!("read failed: {e}")))?;
    serde_json::from_str(&data)
        .map_err(|e| Error::Serde(format!("deserialize failed: {e}")))
}
