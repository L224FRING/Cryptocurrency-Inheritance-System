use std::fmt;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    /// A trustee index outside the configured committee, or an identifier the
    /// ciphersuite refuses to represent.
    BadTrustee(String),
    /// A caller asked for something impossible: bad flags, a threshold outside
    /// the supported range, an empty participant list.
    BadArgument(String),
    /// A party was asked to act before it had received everything it needs.
    NotReady(String),
    /// A protocol step refused. The payload is the underlying FROST message.
    Rejected(String),
    /// Transport-level failure talking to the relay.
    Transport(String),
    /// A message arrived that this session must not act on.
    UnexpectedEnvelope(String),
    /// Signing nonces are single-use. Seeing them twice means a bug or an attack.
    NoncesReused(String),
    /// A session was driven past the point where it can make progress.
    Stalled(String),
    Malformed(String),
    Json(String),
    Serde(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::BadTrustee(m) => write!(f, "bad trustee: {m}"),
            Error::BadArgument(m) => write!(f, "bad argument: {m}"),
            Error::NotReady(m) => write!(f, "not ready: {m}"),
            Error::Rejected(m) => write!(f, "rejected: {m}"),
            Error::Transport(m) => write!(f, "transport: {m}"),
            Error::UnexpectedEnvelope(m) => write!(f, "unexpected envelope: {m}"),
            Error::NoncesReused(m) => write!(f, "nonce reuse: {m}"),
            Error::Stalled(m) => write!(f, "stalled: {m}"),
            Error::Malformed(m) => write!(f, "malformed: {m}"),
            Error::Json(m) => write!(f, "json: {m}"),
            Error::Serde(m) => write!(f, "serde: {m}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Json(e.to_string())
    }
}

impl From<frost_core::Error<frost_secp256k1::Secp256K1Sha256>> for Error {
    fn from(e: frost_core::Error<frost_secp256k1::Secp256K1Sha256>) -> Self {
        Error::Rejected(e.to_string())
    }
}

pub trait ResultExt<T> {
    fn context(self, f: impl FnOnce() -> String) -> Result<T>;
}

impl<T, E: fmt::Display> ResultExt<T> for std::result::Result<T, E> {
    fn context(self, f: impl FnOnce() -> String) -> Result<T> {
        self.map_err(|e| Error::Rejected(format!("{}: {e}", f())))
    }
}
