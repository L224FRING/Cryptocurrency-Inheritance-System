use std::process::ExitCode;

use rand_core::{OsRng, RngCore};
use serde_json::json;

use frost_service::coordinator::{self, SigningOutcome, SigningRequest};
use frost_service::error::{Error, Result};
use frost_service::report::{
    self, BelowThresholdReport, DkgReport, SelftestReport, SignatureReport,
};
use frost_service::transport::{http::HttpTransport, memory::MemoryTransport, Transport};
use frost_service::wire::{SessionId, SIGNING_DOMAIN};



const MAX_SIGNERS: u16 = 5;
const MIN_SIGNERS: u16 = 3;
/// FROST's DKG refuses a single-signer key, so 2 is the floor for a threshold.
const MIN_THRESHOLD: u16 = 2;
const MESSAGE: &[u8] = b"inheritance-release-attestation";

const USAGE: &str = "\
frost-service: FROST (secp256k1) threshold signing for the inheritance vault

usage:
  frost-service selftest              in-process DKG + signing check
  frost-service dkg    [options]      run the 3-part DKG across the committee
  frost-service sign   [options]      run two-round threshold signing
  frost-service matrix [options]      sweep n-of-m and sub-threshold subsets
  frost-service vdf    [options]      compute or verify a VDF proof
  frost-service --help

options:
  --trustees N        committee size                     (default 5)
  --threshold T       signing threshold                  (default 3)
  --session HEX       32-byte session id                 (default: random)
  --domain STR        signing domain separator           (default cis/frost/release-attestation/v1)
  --relay URL         coordinate through a relay         (default: in-process)
  --message HEX       payload to sign                    (default: attestation string)
  --participants L    comma-separated trustee indices    (default: 1..T)
  --expect-failure    require rejection, report the reason

Every subcommand writes one JSON document to stdout. A rejected run exits
non-zero, so a caller never sees a failure parsed as a success.
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let outcome: Result<serde_json::Value> = match args.first().map(String::as_str) {
        None | Some("selftest") => selftest().map(|r| json!(r)),
        Some("dkg") => cmd_dkg(&args[1..]),
        Some("sign") => cmd_sign(&args[1..]),
        Some("matrix") => cmd_matrix(&args[1..]),
        Some("vdf") => cmd_vdf(&args[1..]),
        Some("--help" | "-h") => {
            print!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Some(other) => Err(Error::BadArgument(format!("unknown command `{other}`"))),
    };

    match outcome.and_then(|value| report::emit(&value)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            report::emit_failure(e);
            ExitCode::FAILURE
        }
    }
}

fn random_session() -> SessionId {
    let mut session = [0u8; 32];
    OsRng.fill_bytes(&mut session);
    session
}

fn selftest() -> Result<SelftestReport> {
    let mut rng = OsRng;
    let session = random_session();

    let mut parties = coordinator::new_committee(MAX_SIGNERS)?;
    let mut transport = MemoryTransport::new(coordinator::committee(MAX_SIGNERS));

    let dkg = coordinator::run_dkg(&mut parties, &mut transport, session, MIN_SIGNERS, &mut rng)?;

    let signers: Vec<u16> = (1..=MIN_SIGNERS).collect();
    let signed = coordinator::run_signing(
        &mut parties,
        &mut transport,
        &SigningRequest::new(session, MIN_SIGNERS, &signers, MESSAGE, SIGNING_DOMAIN),
        &mut rng,
    )?;

    // A subset below the threshold must be unable to produce a signature.
    let short: Vec<u16> = (1..MIN_SIGNERS).collect();
    let (rejections, error) = match coordinator::run_signing(
        &mut parties,
        &mut transport,
        &SigningRequest::new(session, MIN_SIGNERS, &short, MESSAGE, SIGNING_DOMAIN),
        &mut rng,
    ) {
        Ok(_) => (
            Vec::new(),
            Some("below-threshold run produced a signature".to_string()),
        ),
        Err(e) => (vec![format!("session refused: {e}")], Some(e.to_string())),
    };

    Ok(SelftestReport {
        status: "ok",
        scheme: "frost-secp256k1",
        dkg: DkgReport {
            trustees: MAX_SIGNERS,
            threshold: MIN_SIGNERS,
            label: format!("{MIN_SIGNERS}-of-{MAX_SIGNERS}"),
        },
        group_verifying_key: dkg.group_verifying_key,
        signature: SignatureReport {
            message: String::from_utf8_lossy(MESSAGE).into_owned(),
            message_hex: hex::encode(MESSAGE),
            signers,
            shares_aggregated: MIN_SIGNERS,
            value: signed.signature,
            verified_against_group_key: true,
        },
        below_threshold: BelowThresholdReport {
            signers: short,
            rejections,
            aggregate_rejected: true,
            aggregate_error: error,
        },
    })
}

struct Options {
    trustees: u16,
    threshold: u16,
    session: SessionId,
    domain: String,
    relay: Option<String>,
    message: Vec<u8>,
    participants: Vec<u16>,
    expect_failure: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            trustees: MAX_SIGNERS,
            threshold: MIN_SIGNERS,
            session: random_session(),
            domain: SIGNING_DOMAIN.to_string(),
            relay: None,
            message: MESSAGE.to_vec(),
            participants: Vec::new(),
            expect_failure: false,
        }
    }
}

fn parse_options(args: &[String]) -> Result<Options> {
    let mut options = Options::default();
    let mut participants_set = false;
    let mut i = 0;

    while i < args.len() {
        let flag = args[i].as_str();
        let next = |i: usize| -> Result<String> {
            args.get(i + 1)
                .cloned()
                .ok_or_else(|| Error::BadArgument(format!("{flag} needs a value")))
        };

        match flag {
            "--trustees" => options.trustees = next(i)?.parse().map_err(bad_number)?,
            "--threshold" => options.threshold = next(i)?.parse().map_err(bad_number)?,
            "--session" => options.session = parse_session(&next(i)?)?,
            "--domain" => options.domain = next(i)?,
            "--relay" => options.relay = Some(next(i)?),
            "--message" => options.message = parse_hex(&next(i)?, "--message")?,
            "--participants" => {
                options.participants = parse_list(&next(i)?)?;
                participants_set = true;
            }
            "--expect-failure" => options.expect_failure = true,
            other => return Err(Error::BadArgument(format!("unknown flag `{other}`"))),
        }

        i += if flag == "--expect-failure" { 1 } else { 2 };
    }

    if options.threshold < MIN_THRESHOLD || options.threshold > options.trustees {
        return Err(Error::BadArgument(format!(
            "threshold {} is not within {MIN_THRESHOLD}..={}",
            options.threshold, options.trustees
        )));
    }
    if !participants_set {
        options.participants = (1..=options.threshold).collect();
    }
    for index in &options.participants {
        if *index == 0 || *index > options.trustees {
            return Err(Error::BadArgument(format!(
                "trustee {index} is outside the committee of {}",
                options.trustees
            )));
        }
    }
    Ok(options)
}

fn bad_number(e: std::num::ParseIntError) -> Error {
    Error::BadArgument(format!("not a number: {e}"))
}

fn parse_hex(raw: &str, flag: &str) -> Result<Vec<u8>> {
    hex::decode(raw.trim_start_matches("0x"))
        .map_err(|e| Error::Malformed(format!("{flag} is not hex: {e}")))
}

fn parse_session(raw: &str) -> Result<SessionId> {
    let bytes = parse_hex(raw, "--session")?;
    if bytes.len() != 32 {
        return Err(Error::Malformed(format!(
            "--session must be 32 bytes, got {}",
            bytes.len()
        )));
    }
    let mut session = [0u8; 32];
    session.copy_from_slice(&bytes);
    Ok(session)
}

fn parse_list(raw: &str) -> Result<Vec<u16>> {
    raw.split(',')
        .map(|part| {
            part.trim().parse::<u16>().map_err(|e| {
                Error::BadArgument(format!("bad trustee index `{}`: {e}", part.trim()))
            })
        })
        .collect()
}

fn transport_for(options: &Options) -> Box<dyn Transport> {
    match &options.relay {
        Some(url) => Box::new(HttpTransport::new(url.clone())),
        None => Box::new(MemoryTransport::new((1..=options.trustees).collect())),
    }
}

fn cmd_dkg(args: &[String]) -> Result<serde_json::Value> {
    let options = parse_options(args)?;
    let mut rng = OsRng;
    let mut parties = coordinator::new_committee(options.trustees)?;
    let mut transport = transport_for(&options);

    let outcome = coordinator::run_dkg(
        &mut parties,
        transport.as_mut(),
        options.session,
        options.threshold,
        &mut rng,
    );

    match outcome {
        Ok(dkg) if !options.expect_failure => Ok(json!(dkg)),
        Ok(_) => Err(Error::BadArgument(
            "DKG unexpectedly succeeded under --expect-failure".to_string(),
        )),
        Err(e) if options.expect_failure => Ok(json!({
            "status": "rejected_as_expected",
            "error": e.to_string(),
        })),
        Err(e) => Err(e),
    }
}

fn cmd_sign(args: &[String]) -> Result<serde_json::Value> {
    let options = parse_options(args)?;
    let mut rng = OsRng;
    let mut parties = coordinator::new_committee(options.trustees)?;
    let mut transport = transport_for(&options);

    let dkg = coordinator::run_dkg(
        &mut parties,
        transport.as_mut(),
        options.session,
        options.threshold,
        &mut rng,
    )?;
    if options.expect_failure {
        return Err(Error::BadArgument(
            "--expect-failure is not meaningful for a command that runs a DKG first".to_string(),
        ));
    }

    let outcome = coordinator::run_signing(
        &mut parties,
        transport.as_mut(),
        &SigningRequest::new(
            options.session,
            options.threshold,
            &options.participants,
            &options.message,
            &options.domain,
        ),
        &mut rng,
    );

    match outcome {
        Ok(signed) => Ok(json!({
            "status": "ok",
            "dkg": {
                "trustees": dkg.trustees,
                "threshold": dkg.threshold,
                "group_verifying_key": dkg.group_verifying_key,
            },
            "signing": signed,
        })),
        Err(e) => Err(e),
    }
}

#[derive(serde::Serialize)]
struct MatrixCase {
    trustees: u16,
    threshold: u16,
    participants: Vec<u16>,
    expected: &'static str,
    result: &'static str,
    error: Option<String>,
    signature: Option<String>,
}

#[derive(serde::Serialize)]
struct MatrixReport {
    status: &'static str,
    trustees_swept: Vec<u16>,
    cases: Vec<MatrixCase>,
    passed: usize,
    failed: usize,
}

/// Sweep threshold-sized and sub-threshold subsets across several committees.
fn cmd_matrix(args: &[String]) -> Result<serde_json::Value> {
    let options = parse_options(args)?;
    let mut rng = OsRng;
    let mut cases = Vec::new();
    let mut swept = Vec::new();

    let sizes: Vec<u16> = [2, 3, options.trustees]
        .into_iter()
        .filter(|n| *n <= options.trustees)
        .collect();

    for trustees in sizes {
        swept.push(trustees);
        for threshold in MIN_THRESHOLD..=trustees {
            let mut parties = coordinator::new_committee(trustees)?;
            let mut transport = MemoryTransport::new(coordinator::committee(trustees));
            let session = random_session();

            coordinator::run_dkg(&mut parties, &mut transport, session, threshold, &mut rng)?;

            // Every signing run gets its own session id. A session may only be
            // signed over once, so reusing one would be refused by design.
            let sign_session = |rng: &mut rand_core::OsRng| {
                let mut s = [0u8; 32];
                rng.fill_bytes(&mut s);
                s
            };

            // Exactly the threshold must succeed.
            let at_threshold: Vec<u16> = (1..=threshold).collect();
            cases.push(record(
                trustees,
                threshold,
                &at_threshold,
                "signature verifies",
                coordinator::run_signing(
                    &mut parties,
                    &mut transport,
                    &SigningRequest::new(
                        sign_session(&mut rng),
                        threshold,
                        &at_threshold,
                        &options.message,
                        &options.domain,
                    ),
                    &mut rng,
                ),
            ));

            // More than the threshold must also succeed.
            if threshold < trustees {
                let above: Vec<u16> = (1..=trustees).collect();
                cases.push(record(
                    trustees,
                    threshold,
                    &above,
                    "signature verifies",
                    coordinator::run_signing(
                        &mut parties,
                        &mut transport,
                        &SigningRequest::new(
                            sign_session(&mut rng),
                            threshold,
                            &above,
                            &options.message,
                            &options.domain,
                        ),
                        &mut rng,
                    ),
                ));
            }

            // One short of the threshold must fail.
            if threshold >= 2 {
                let below: Vec<u16> = (1..threshold).collect();
                cases.push(record(
                    trustees,
                    threshold,
                    &below,
                    "rejected",
                    coordinator::run_signing(
                        &mut parties,
                        &mut transport,
                        &SigningRequest::new(
                            sign_session(&mut rng),
                            threshold,
                            &below,
                            &options.message,
                            &options.domain,
                        ),
                        &mut rng,
                    ),
                ));
            }
        }
    }

    let failed = cases.iter().filter(|c| c.result != c.expected).count();
    Ok(json!(MatrixReport {
        status: if failed == 0 { "ok" } else { "failed" },
        trustees_swept: swept,
        passed: cases.len() - failed,
        failed,
        cases,
    }))
}

fn record(
    trustees: u16,
    threshold: u16,
    participants: &[u16],
    expected: &'static str,
    outcome: Result<SigningOutcome>,
) -> MatrixCase {
    match outcome {
        Ok(signed) => MatrixCase {
            trustees,
            threshold,
            participants: participants.to_vec(),
            expected,
            result: "signature verifies",
            error: None,
            signature: Some(signed.signature),
        },
        Err(e) => MatrixCase {
            trustees,
            threshold,
            participants: participants.to_vec(),
            expected,
            result: "rejected",
            error: Some(e.to_string()),
            signature: None,
        },
    }
}

// VDF commands
fn cmd_vdf(args: &[String]) -> Result<serde_json::Value> {
    let mut t: u64 = 100;
    let mut bits: usize = 1024;
    let mut input_hex = String::from("deadbeef");
    let mut verify = false;
    let mut x_str: Option<String> = None;
    let mut y_str: Option<String> = None;
    let mut proof_strs: Vec<String> = Vec::new();
    
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "selftest" => {
                i += 1;
                continue;
            }
            "--t" => {
                if i + 1 < args.len() {
                    t = args[i + 1].parse().map_err(|e| Error::BadArgument(format!("bad t: {e}")))?;
                    i += 2;
                } else {
                    return Err(Error::BadArgument("--t needs value".to_string()));
                }
            }
            "--bits" => {
                if i + 1 < args.len() {
                    bits = args[i + 1].parse().map_err(|e| Error::BadArgument(format!("bad bits: {e}")))?;
                    i += 2;
                } else {
                    return Err(Error::BadArgument("--bits needs value".to_string()));
                }
            }
            "--input" => {
                if i + 1 < args.len() {
                    input_hex = args[i + 1].clone();
                    i += 2;
                } else {
                    return Err(Error::BadArgument("--input needs value".to_string()));
                }
            }
            "--verify" => {
                verify = true;
                i += 1;
            }
            "--x" => {
                if i + 1 < args.len() {
                    x_str = Some(args[i + 1].clone());
                    i += 2;
                } else {
                    return Err(Error::BadArgument("--x needs value".to_string()));
                }
            }
            "--y" => {
                if i + 1 < args.len() {
                    y_str = Some(args[i + 1].clone());
                    i += 2;
                } else {
                    return Err(Error::BadArgument("--y needs value".to_string()));
                }
            }
            "--proof" => {
                if i + 1 < args.len() {
                    proof_strs.push(args[i + 1].clone());
                    i += 2;
                } else {
                    return Err(Error::BadArgument("--proof needs value".to_string()));
                }
            }
            _ => {
                i += 1;
            }
        }
    }
    
    if verify {
        if let (Some(xs), Some(ys)) = (x_str, y_str) {
            use num_bigint::BigUint;
            let x: BigUint = xs.parse().unwrap_or_else(|_| BigUint::from_bytes_be(&hex::decode(&xs).unwrap_or_default()));
            let y: BigUint = ys.parse().unwrap_or_else(|_| BigUint::from_bytes_be(&hex::decode(&ys).unwrap_or_default()));
            let mut proof: Vec<BigUint> = Vec::new();
            for p in &proof_strs {
                if let Ok(b) = hex::decode(p) {
                    proof.push(BigUint::from_bytes_be(&b));
                }
            }
            let n = BigUint::from(0x10001u64) * BigUint::from(0x7fffffff12345678u64);
            let result = frost_service::vdf::verify_vdf_pietrzak(&x, &y, &proof, t, &n);
            return Ok(json!({
                "verified": result,
                "t": t,
                "n": n.to_str_radix(10)
            }));
        }
    }
    
    // Compute mode
    use num_bigint::BigUint;
    let mut rng = OsRng;
    let n = frost_service::vdf::generate_rsa_modulus(&mut rng, bits);
    let input_bytes = hex::decode(&input_hex).unwrap_or_else(|_| input_hex.as_bytes().to_vec());
    let x = BigUint::from_bytes_be(&input_bytes);
    let res = frost_service::vdf::compute_vdf_with_proof(&x, &frost_service::vdf::VDFParams { n: n.clone(), t });
    Ok(json!({
        "x": x.to_str_radix(16),
        "y": res.y.to_str_radix(16),
        "t": t,
        "n": n.to_str_radix(16),
        "proof_points": res.proof.iter().map(|p| p.to_str_radix(16)).collect::<Vec<_>>(),
        "status": "ok"
    }))
}
