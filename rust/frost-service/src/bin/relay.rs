use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

use frost_service::transport::http::{InboxResponse, RelayState, SendRequest, SharedRelay};

const MAX_BODY: usize = 8 * 1024 * 1024;

#[derive(serde::Serialize)]
struct Stats {
    queued: usize,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut listen = "127.0.0.1:8477".to_string();
    let mut trustees: u16 = 5;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--listen" => {
                listen = args
                    .get(i + 1)
                    .cloned()
                    .unwrap_or_else(|| usage_and_exit("--listen needs a value"));
                i += 2;
            }
            "--trustees" => {
                let raw = args
                    .get(i + 1)
                    .cloned()
                    .unwrap_or_else(|| usage_and_exit("--trustees needs a value"));
                trustees = raw
                    .parse()
                    .unwrap_or_else(|_| usage_and_exit("--trustees must be a number"));
                i += 2;
            }
            "--help" | "-h" => usage_and_exit(""),
            other => usage_and_exit(&format!("unknown argument `{other}`")),
        }
    }

    let roster: Vec<u16> = (1..=trustees).collect();
    let state: SharedRelay = Arc::new(Mutex::new(RelayState::new(roster)));

    let listener =
        TcpListener::bind(&listen).unwrap_or_else(|e| panic!("cannot bind {listen}: {e}"));

    // Print the resolved address, not the requested one: with `--listen
    // 127.0.0.1:0` the port is only known after binding. Flushed so a caller
    // can read the line to learn where to connect.
    let bound = listener
        .local_addr()
        .unwrap_or_else(|e| panic!("cannot read the bound address: {e}"));
    println!("frost-relay listening on {bound} for trustees 1..={trustees}");
    println!("the relay is untrusted: it routes opaque bytes and verifies nothing");
    use std::io::Write as _;
    let _ = std::io::stdout().flush();

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let state = Arc::clone(&state);
                // A relay for a handful of trustees does not need concurrency.
                // Handling one request at a time keeps ordering obvious.
                if let Err(e) = handle(stream, &state) {
                    eprintln!("request failed: {e}");
                }
            }
            Err(e) => eprintln!("accept failed: {e}"),
        }
    }
}

fn usage_and_exit(message: &str) -> ! {
    if !message.is_empty() {
        eprintln!("error: {message}");
    }
    eprintln!(
        "usage: frost-relay [--listen ADDR] [--trustees N]\n\
         \n\
         Routes FROST envelopes between trustees. Endpoints:\n\
         \x20 POST /send            {{\"trustee\":N,\"envelopes\":[...]}}\n\
         \x20 GET  /inbox/{{n}}       drains trustee n's queue\n\
         \x20 GET  /stats           {{\"queued\":N}}\n\
         \x20 POST /reset           clears all queues"
    );
    std::process::exit(if message.is_empty() { 0 } else { 2 });
}

fn handle(mut stream: TcpStream, state: &SharedRelay) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);

    let mut request_line = String::new();
    if reader.read_line(&mut request_line)? == 0 {
        return Ok(());
    }
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let target = parts.next().unwrap_or_default().to_string();

    let mut content_length = 0usize;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header)? == 0 {
            break;
        }
        let header = header.trim_end();
        if header.is_empty() {
            break;
        }
        if let Some(value) = header
            .strip_prefix("Content-Length:")
            .or_else(|| header.strip_prefix("content-length:"))
        {
            content_length = value.trim().parse().unwrap_or(0);
        }
    }

    if content_length > MAX_BODY {
        return respond(&mut stream, 413, br#"{"error":"body too large"}"#);
    }
    let mut body = vec![0u8; content_length];
    if content_length > 0 {
        reader.read_exact(&mut body)?;
    }

    let path = target.split('?').next().unwrap_or_default().to_string();
    let mut state = state.lock().unwrap_or_else(|e| e.into_inner());

    let (code, payload) = match (method.as_str(), path.as_str()) {
        ("POST", "/send") => match serde_json::from_slice::<SendRequest>(&body) {
            Ok(req) => match state.accept(req.trustee, req.envelopes) {
                Ok(()) => (200, json(&InboxResponse { envelopes: vec![] })),
                Err(e) => (400, json(&serde_json::json!({ "error": e.to_string() }))),
            },
            Err(e) => (400, json(&serde_json::json!({ "error": e.to_string() }))),
        },
        ("GET", p) if p.starts_with("/inbox/") => {
            let trustee: u16 = match p.trim_start_matches("/inbox/").parse() {
                Ok(v) => v,
                Err(_) => {
                    return respond(
                        &mut stream,
                        400,
                        json(&serde_json::json!({ "error": "bad trustee" })).as_bytes(),
                    )
                }
            };
            let envelopes = state.drain(trustee);
            (200, json(&InboxResponse { envelopes }))
        }
        ("GET", "/stats") => (
            200,
            json(&Stats {
                queued: state.queued(),
            }),
        ),
        ("POST", "/reset") => {
            state.reset();
            (200, json(&InboxResponse { envelopes: vec![] }))
        }
        ("GET", "/health") => (200, json(&serde_json::json!({ "ok": true }))),
        _ => (
            404,
            json(&serde_json::json!({ "error": "no such endpoint" })),
        ),
    };

    drop(state);
    respond(&mut stream, code, payload.as_bytes())
}

fn json<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
}

fn respond(stream: &mut TcpStream, code: u16, body: &[u8]) -> std::io::Result<()> {
    let reason = match code {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        413 => "Payload Too Large",
        _ => "Error",
    };
    let head = format!(
        "HTTP/1.1 {code} {reason}\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()
}
