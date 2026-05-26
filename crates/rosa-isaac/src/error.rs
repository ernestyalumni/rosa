//! Error type for `rosa-isaac`.

use thiserror::Error;

/// Errors produced by Isaac Sim tool calls.
#[derive(Debug, Error)]
pub enum IsaacError {
    /// HTTP transport error (connection refused, timeout, …)
    #[error("Isaac control server unreachable: {0}")]
    Transport(#[from] reqwest::Error),

    /// Isaac server returned a non-2xx status.
    #[error("Isaac control server returned HTTP {status}: {body}")]
    ServerError { status: u16, body: String },

    /// JSON decode error on the server's response body.
    #[error("bad JSON from Isaac control server: {0}")]
    Json(#[from] serde_json::Error),
}
