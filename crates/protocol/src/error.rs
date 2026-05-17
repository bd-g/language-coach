use thiserror::Error;

/// Errors for JSON media envelopes and PCMU payload handling.
#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("invalid JSON message: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("invalid base64 payload: {0}")]
    InvalidBase64(#[from] base64::DecodeError),
    #[error("unsupported event: {0}")]
    UnsupportedEvent(String),
    #[error("missing media payload")]
    MissingMediaPayload,
}
