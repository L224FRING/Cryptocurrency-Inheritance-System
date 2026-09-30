use serde::Serialize;

use crate::error::{Error, Result};

#[derive(Serialize)]
pub struct Failure {
    pub status: &'static str,
    pub error: String,
}

#[derive(Serialize)]
pub struct DkgReport {
    pub trustees: u16,
    pub threshold: u16,
    pub label: String,
}

#[derive(Serialize)]
pub struct SignatureReport {
    pub message: String,
    pub message_hex: String,
    pub signers: Vec<u16>,
    pub shares_aggregated: u16,
    pub value: String,
    pub verified_against_group_key: bool,
}

#[derive(Serialize)]
pub struct BelowThresholdReport {
    pub signers: Vec<u16>,
    /// One entry per refusal. A sub-threshold attempt is refused before any
    /// round runs, so this normally holds a single explanation rather than one
    /// line per trustee.
    pub rejections: Vec<String>,
    pub aggregate_rejected: bool,
    pub aggregate_error: Option<String>,
}

#[derive(Serialize)]
pub struct SelftestReport {
    pub status: &'static str,
    pub scheme: &'static str,
    pub dkg: DkgReport,
    pub group_verifying_key: String,
    pub signature: SignatureReport,
    pub below_threshold: BelowThresholdReport,
}

/// Write one JSON document to stdout. Nothing else is ever printed there, so a
/// caller can parse stdout without stripping banners.
pub fn emit(report: &impl Serialize) -> Result<()> {
    let json = serde_json::to_string_pretty(report)
        .map_err(|e| Error::Serde(format!("report serialization failed: {e}")))?;
    println!("{json}");
    Ok(())
}

pub fn emit_failure(error: impl std::fmt::Display) {
    let _ = emit(&Failure {
        status: "error",
        error: error.to_string(),
    });
}
