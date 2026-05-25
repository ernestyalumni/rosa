//! `log_message` — emit a structured log event at the requested severity.
//!
//! This is useful for agents that need to write audit trails or debug output
//! without having full shell access.

use async_trait::async_trait;
use schemars::{schema_for, schema::RootSchema, JsonSchema};
use serde::Deserialize;
use serde_json::{json, Value};

use rosa_core::error::Result;

use crate::tool::Tool;

// ---------------------------------------------------------------------------
// Arg struct
// ---------------------------------------------------------------------------

fn default_level() -> String {
    "info".to_owned()
}

/// Arguments for the `log_message` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct LogMessageArgs {
    /// The message text to log.
    pub message: String,

    /// Severity level: `trace`, `debug`, `info`, `warn`, or `error`.
    /// Defaults to `"info"` when omitted.
    #[serde(default = "default_level")]
    pub level: String,
}

// ---------------------------------------------------------------------------
// Tool impl
// ---------------------------------------------------------------------------

/// Emit a structured log event using the `tracing` subscriber.
///
/// The agent can use this to write timestamped audit entries without needing
/// shell access. The log event is observable via any `tracing` subscriber
/// (e.g. `tracing-subscriber` pretty-printer or JSON exporter).
pub struct LogMessageTool;

#[async_trait]
impl Tool for LogMessageTool {
    fn name(&self) -> &str {
        "log_message"
    }

    fn description(&self) -> &str {
        "Log a message at the given severity level (trace/debug/info/warn/error). \
         Returns {logged: true, level, message} on success."
    }

    fn schema(&self) -> RootSchema {
        schema_for!(LogMessageArgs)
    }

    async fn execute(&self, args: Value) -> Result<Value> {
        let a: LogMessageArgs = serde_json::from_value(args)?;
        match a.level.as_str() {
            "trace" => tracing::trace!(message = %a.message, "agent log"),
            "debug" => tracing::debug!(message = %a.message, "agent log"),
            "warn"  => tracing::warn!(message  = %a.message, "agent log"),
            "error" => tracing::error!(message = %a.message, "agent log"),
            _       => tracing::info!(message  = %a.message, "agent log"),
        }
        Ok(json!({
            "logged":  true,
            "level":   a.level,
            "message": a.message,
        }))
    }
}
