//! Log tools: `log_message` (emit) and `read_log` (read + filter file contents).

use std::io::{BufRead, BufReader};

use async_trait::async_trait;
use schemars::{schema_for, schema::RootSchema, JsonSchema};
use serde::Deserialize;
use serde_json::{json, Value};

use rosa_core::error::{Result, RosaError};

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

// ---------------------------------------------------------------------------
// read_log — read a log file with optional level filter and line range
// ---------------------------------------------------------------------------

fn default_start_line() -> usize { 0 }
fn default_end_line()   -> usize { 50 }

/// Arguments for the `read_log` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadLogArgs {
    /// Absolute path to the log file (e.g. a path from `roslog_list`).
    pub file_path: String,

    /// Optional severity keyword to filter lines (case-insensitive substring
    /// match: `"INFO"`, `"WARN"`, `"ERROR"`, `"DEBUG"`).
    /// Omit or pass `""` to return all lines.
    #[serde(default)]
    pub level_filter: String,

    /// First line to return (0-indexed, inclusive). Default 0.
    #[serde(default = "default_start_line")]
    pub start_line: usize,

    /// Last line to return (0-indexed, exclusive). Default 50.
    #[serde(default = "default_end_line")]
    pub end_line: usize,
}

/// Read a log file, optionally filter by severity keyword, and return a line slice.
///
/// Matches the Python `read_log` tool: useful for inspecting ROS log output
/// (paths come from `roslog_list`).
pub struct ReadLogTool;

#[async_trait]
impl Tool for ReadLogTool {
    fn name(&self) -> &str { "read_log" }

    fn description(&self) -> &str {
        "Read lines from a log file. Optionally filter by severity keyword \
         (INFO/WARN/ERROR/DEBUG — case-insensitive substring). \
         Returns the slice [start_line, end_line) of matching lines and the \
         total matching line count. Use `roslog_list` first to find log paths."
    }

    fn schema(&self) -> RootSchema { schema_for!(ReadLogArgs) }

    async fn execute(&self, args: Value) -> Result<Value> {
        let a: ReadLogArgs = serde_json::from_value(args)?;

        let file = std::fs::File::open(&a.file_path).map_err(|e| RosaError::ToolExecution {
            name: self.name().into(),
            message: format!("cannot open '{}': {e}", a.file_path),
        })?;

        let filter = a.level_filter.to_uppercase();
        let lines: Vec<String> = BufReader::new(file)
            .lines()
            .filter_map(|l| l.ok())
            .filter(|l| filter.is_empty() || l.to_uppercase().contains(&filter))
            .collect();

        let total = lines.len();
        let start = a.start_line.min(total);
        let end   = a.end_line.min(total);
        let slice = lines[start..end].to_vec();

        Ok(json!({
            "file_path":    a.file_path,
            "level_filter": a.level_filter,
            "total_lines":  total,
            "start_line":   start,
            "end_line":     end,
            "lines":        slice,
        }))
    }
}
