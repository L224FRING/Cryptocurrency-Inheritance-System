//! Per-trustee integration tests.
//!
//! These exercise the deployed shape: every trustee is a separate process that
//! holds only its own share. The in-process tests in `protocol.rs` prove the
//! FROST math; these prove the wiring that makes the trust boundary real.
//!
//! Two layers are covered:
//! - threaded clients, which drive `client::run_*` concurrently against a live
//!   relay, and
//! - the actual `frost-service dkg-party` / `sign-party` subcommands, so the CLI
//!   flags and file formats are exercised too.

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use frost_service::client;
use frost_service::coordinator;
use frost_service::party::Party;
use frost_service::persistence;
use frost_service::transport::http::HttpTransport;
use frost_service::wire::{SessionId, SIGNING_DOMAIN};
use rand_core::OsRng;

use frost_secp256k1 as frostk;

const MESSAGE: &[u8] = b"inheritance-release-attestation";
const COMMITTEE: u16 = 5;
const THRESHOLD: u16 = 3;
/// Generous enough that slow CI does not fail, short enough to surface a hang.
const TIMEOUT: Duration = Duration::from_secs(30);

fn session_id(seed: u8) -> SessionId {
    let mut s = [0u8; 32];
    s[0] = seed;
    s[31] = seed;
    s
}

fn hex(session: &SessionId) -> String {
    session.iter().map(|b| format!("{b:02x}")).collect()
}

fn verify(group_key_hex: &str, session: &SessionId, signature_hex: &str) -> bool {
    let verifying_key =
        frostk::VerifyingKey::deserialize(&hex_decode(group_key_hex)).expect("group key");
    let signature = frostk::Signature::deserialize(&hex_decode(signature_hex)).expect("signature");
    let message = frost_service::wire::signing_message(session, SIGNING_DOMAIN, MESSAGE);
    verifying_key.verify(&message, &signature).is_ok()
}

fn hex_decode(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex"))
        .collect()
}

/// Start the relay binary and learn the port it actually bound.
struct RunningRelay {
    child: Child,
    base: String,
}

impl RunningRelay {
    fn start() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_frost-relay"))
            .args(["--listen", "127.0.0.1:0", "--trustees", "5"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("relay binary starts");

        let stdout = child.stdout.take().expect("piped stdout");
        let mut line = String::new();
        {
            use std::io::BufRead as _;
            let mut reader = std::io::BufReader::new(stdout);
            reader.read_line(&mut line).expect("relay greeting");
        }
        let addr = line
            .split_whitespace()
            .nth(3)
            .unwrap_or_else(|| panic!("unexpected relay greeting: {line}"))
            .to_string();

        Self {
            child,
            base: format!("http://{addr}"),
        }
    }
}

impl Drop for RunningRelay {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Run 5 concurrent trustee clients through DKG, then 3 of them sign, with the
/// aggregator producing a signature that verifies against the group key.
#[test]
fn separate_trustee_processes_complete_dkg_and_signing() {
    let relay = RunningRelay::start();
    let session = session_id(0x11);

    // DKG: one thread per trustee, each holding only its own Party.
    let dkg_session = session;
    let base = relay.base.clone();
    let handles: Vec<_> = (1..=COMMITTEE)
        .map(|index| {
            let base = base.clone();
            thread::spawn(move || {
                let committee = coordinator::committee(COMMITTEE);
                let mut party = Party::new(index, &committee).expect("party");
                let mut transport = HttpTransport::new(base);
                let group_key = client::run_dkg_party(
                    &mut party,
                    &mut transport,
                    dkg_session,
                    THRESHOLD,
                    COMMITTEE,
                    TIMEOUT,
                    &mut OsRng,
                )
                .unwrap_or_else(|e| panic!("trustee {index} dkg: {e}"));
                let share = party
                    .export_share(&committee, THRESHOLD)
                    .expect("export share");
                (index, group_key, share)
            })
        })
        .collect();

    let mut group_keys = Vec::new();
    let mut shares = Vec::new();
    for handle in handles {
        let (index, group_key, share) = handle.join().expect("dkg thread");
        assert_eq!(share.trustee_id, index);
        group_keys.push(group_key);
        shares.push(share);
    }

    let first = hex_encode(&group_keys[0]);
    for key in &group_keys[1..] {
        assert_eq!(
            hex_encode(key),
            first,
            "every trustee must derive the same group key"
        );
    }

    // Round-trip through disk, as a real client would.
    let dir = temp_dir("dkg");
    for share in &shares {
        let path = format!("{dir}/share-{}.json", share.trustee_id);
        persistence::save_trustee_share(share, &path).expect("save share");
        let loaded = persistence::load_trustee_share(&path).expect("load share");
        let rebuilt = Party::from_share(&loaded).expect("rebuild party");
        assert_eq!(rebuilt.me(), share.trustee_id);
    }

    // Signing: participants 1,2,3; trustee 1 aggregates.
    let sign_session = session_id(0x22);
    let participants = vec![1u16, 2, 3];
    let sign_handles: Vec<_> = participants
        .iter()
        .map(|index| {
            let index = *index;
            let base = base.clone();
            let participants = participants.clone();
            let path = format!("{dir}/share-{index}.json");
            let share = persistence::load_trustee_share(&path).expect("share");
            thread::spawn(move || {
                let mut party = Party::from_share(&share).expect("party");
                let mut transport = HttpTransport::new(base);
                let result = client::run_signing_party(
                    &mut party,
                    &mut transport,
                    sign_session,
                    THRESHOLD,
                    &participants,
                    MESSAGE,
                    SIGNING_DOMAIN,
                    index == 1,
                    TIMEOUT,
                    &mut OsRng,
                )
                .unwrap_or_else(|e| panic!("trustee {index} sign: {e}"));
                (index, result)
            })
        })
        .collect();

    let mut signature = None;
    for handle in sign_handles {
        let (index, result) = handle.join().expect("sign thread");
        if index == 1 {
            signature = result;
        } else {
            assert!(result.is_none(), "non-aggregators must not aggregate");
        }
    }

    let signature = signature.expect("aggregator produced a signature");
    let signature_hex = signature
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    assert!(
        verify(&first, &sign_session, &signature_hex),
        "aggregated signature must verify against the group key"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// Drive the real CLI subcommands as child processes.
#[test]
fn cli_party_commands_run_distributed_dkg_and_signing() {
    let relay = RunningRelay::start();
    let dir = temp_dir("cli");
    let session = session_id(0x33);
    let signer = env!("CARGO_BIN_EXE_frost-service");

    // DKG: 5 children, all with the same session, each writing its own share.
    let dkg_children: Vec<Child> = (1..=COMMITTEE)
        .map(|index| {
            Command::new(signer)
                .args([
                    "dkg-party",
                    "--index",
                    &index.to_string(),
                    "--trustees",
                    "5",
                    "--threshold",
                    "3",
                    "--session",
                    &hex(&session),
                    "--relay",
                    &relay.base,
                    "--out",
                    &format!("{dir}/share-{index}.json"),
                    "--timeout-ms",
                    "30000",
                ])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("spawn dkg-party")
        })
        .collect();

    for mut child in dkg_children {
        let status = child.wait().expect("wait dkg-party");
        assert!(status.success(), "a dkg-party process failed: {status}");
    }

    for index in 1..=COMMITTEE {
        assert!(
            PathBuf::from(format!("{dir}/share-{index}.json")).exists(),
            "share {index} was not written"
        );
    }

    // Signing: 3 children, one aggregating. Capture the aggregator's stdout.
    let sign_session = session_id(0x44);
    let participant = |index: u16, aggregate: bool| {
        let mut args: Vec<String> = vec![
            "sign-party".into(),
            "--index".into(),
            index.to_string(),
            "--share".into(),
            format!("{dir}/share-{index}.json"),
            "--participants".into(),
            "1,2,3".into(),
            "--session".into(),
            hex(&sign_session),
            "--relay".into(),
            relay.base.clone(),
            "--timeout-ms".into(),
            "30000".into(),
        ];
        if aggregate {
            args.push("--aggregate".into());
        }
        Command::new(signer)
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn sign-party")
    };

    let aggregator = participant(1, true);
    let other_a = participant(2, false);
    let other_b = participant(3, false);

    let output = aggregator.wait_with_output().expect("aggregator output");
    assert!(output.status.success(), "aggregator failed");
    for mut child in [other_a, other_b] {
        assert!(child.wait().expect("wait participant").success());
    }

    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("aggregator JSON");
    assert_eq!(report["role"], "aggregator");
    let signature = report["signature"].as_str().expect("signature field");

    // Read the group key from a share to verify against.
    let share = persistence::load_trustee_share(&format!("{dir}/share-1.json")).expect("share");
    assert!(
        verify(&share.group_verifying_key, &sign_session, signature),
        "CLI aggregate signature must verify against the group key"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Unique scratch directory under the system temp dir. Unique per call so tests
/// running in parallel do not collide.
fn temp_dir(label: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("frost-{label}-{nanos}"));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir.to_string_lossy().into_owned()
}
