//! HTTP client wrapper for the Isaac Sim embedded control server.
//!
//! The Isaac Sim container runs `enable_ros2_bridge.py`, which starts a
//! small HTTP server on `ISAAC_CONTROL_PORT` (default `8282`).  This
//! client wraps `reqwest` with Isaac-specific error handling.

use reqwest::Client;
use serde_json::Value;
use std::sync::Arc;

use crate::error::IsaacError;

/// Shared HTTP client for the Isaac Sim control API.
///
/// Clone is O(1) — the underlying `reqwest::Client` is reference-counted.
#[derive(Clone, Debug)]
pub struct IsaacClient {
    inner: Client,
    base_url: Arc<String>,
}

impl Default for IsaacClient {
    fn default() -> Self {
        let url = std::env::var("ISAAC_CONTROL_URL")
            .unwrap_or_else(|_| "http://localhost:8282".to_owned());
        Self::new(url)
    }
}

impl IsaacClient {
    /// Create a client that connects to the given base URL.
    ///
    /// ```
    /// use rosa_isaac::IsaacClient;
    /// let client = IsaacClient::new("http://localhost:8282");
    /// ```
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            inner: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("failed to build reqwest client"),
            base_url: Arc::new(base_url.into()),
        }
    }

    /// `GET /<path>` → JSON body.
    pub async fn get(&self, path: &str) -> Result<Value, IsaacError> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self.inner.get(&url).send().await?;
        self.parse(resp).await
    }

    /// `POST /<path>` with an optional JSON body → JSON response.
    pub async fn post(&self, path: &str, body: Option<&Value>) -> Result<Value, IsaacError> {
        let url = format!("{}{}", self.base_url, path);
        let req = self.inner.post(&url);
        let req = match body {
            Some(b) => req.json(b),
            None    => req.header("Content-Length", "0"),
        };
        let resp = req.send().await?;
        self.parse(resp).await
    }

    // ── Helpers ──────────────────────────────────────────────────────────────

    async fn parse(&self, resp: reqwest::Response) -> Result<Value, IsaacError> {
        let status = resp.status().as_u16();
        let body   = resp.text().await.unwrap_or_default();
        if !(200..300).contains(&(status as usize)) {
            return Err(IsaacError::ServerError { status, body });
        }
        Ok(serde_json::from_str(&body)?)
    }
}
