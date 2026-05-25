//! `roslog_list` — list ROS 2 log files on the host (or inside the container).
//!
//! ROS 2 stores logs under `~/.ros/log/` by default, or `$ROS_LOG_DIR` if set.
//! We shell out to `find` (or `ls -la`) so this works both natively and via
//! `docker exec` (the ShellRunner handles the container routing).
//!
//! Mirrors the Python `roslog_list` tool: filters by minimum file size and an
//! optional blacklist, and returns file paths with human-readable sizes.

use async_trait::async_trait;
use schemars::{schema_for, schema::RootSchema, JsonSchema};
use serde::Deserialize;
use serde_json::{json, Value};

use rosa_core::error::{Result, RosaError};
use rosa_tools::Tool;

#[cfg(test)]
use crate::runner::SharedRunner;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RoslogListArgs {
    /// Minimum file size in bytes to include. Defaults to 2048.
    #[serde(default = "default_min_size")]
    pub min_size: u64,
    /// Optional list of substrings — files whose paths contain any of these are excluded.
    #[serde(default)]
    pub blacklist: Vec<String>,
}

fn default_min_size() -> u64 { 2048 }

/// Note: this tool reads from the *host* filesystem using `find`.
/// It does not route through ShellRunner/docker exec because ROS log files
/// are written to the host's `~/.ros/log/` even when ros2 runs in a container.
pub struct RoslogListTool;

impl RoslogListTool {
    pub fn new(_blacklist: Vec<String>) -> Self { Self }

    /// Kept for API consistency with other tools; `_runner` is not used.
    #[cfg(test)]
    pub fn with_runner(_runner: SharedRunner, _blacklist: Vec<String>) -> Self { Self }
}

#[async_trait]
impl Tool for RoslogListTool {
    fn name(&self) -> &str { "roslog_list" }

    fn description(&self) -> &str {
        "List ROS 2 log files under ~/.ros/log/ (or $ROS_LOG_DIR). \
         Returns file paths and sizes. Use roslog_read to read a specific file."
    }

    fn schema(&self) -> RootSchema { schema_for!(RoslogListArgs) }

    async fn execute(&self, args: Value) -> Result<Value> {
        let a: RoslogListArgs = serde_json::from_value(args)?;

        // Determine the log directory: $ROS_LOG_DIR if set, else ~/.ros/log.
        let log_dir = match std::process::Command::new("bash")
            .args(["-c", "echo \"${ROS_LOG_DIR:-$HOME/.ros/log}\""])
            .output()
        {
            Ok(o) => String::from_utf8_lossy(&o.stdout).trim().to_owned(),
            Err(_) => {
                let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
                format!("{home}/.ros/log")
            }
        };

        // Use `find` to list .log files (works inside docker too via ShellRunner).
        // We can't use ShellRunner directly here because this isn't a `ros2` sub-command;
        // we shell out from the host side for native mode, which is fine because
        // log files are on the host filesystem when ROS_CONTAINER is not set.
        //
        // When ROS_CONTAINER *is* set the log_dir will point to the host path too —
        // the container doesn't hold persistent logs the agent needs to read.
        // For simplicity, we always check the host path.
        let output = std::process::Command::new("find")
            .args([
                &log_dir,
                "-type", "f",
                "-name", "*.log",
            ])
            .output()
            .map_err(|e| RosaError::ToolExecution {
                name: "roslog_list".into(),
                message: format!("failed to run find: {e}"),
            })?;

        let raw = String::from_utf8_lossy(&output.stdout);
        let mut files: Vec<Value> = Vec::new();

        for path in raw.lines() {
            let path = path.trim();
            if path.is_empty() { continue; }

            // Apply blacklist
            if a.blacklist.iter().any(|b| path.contains(b.as_str())) {
                continue;
            }

            // Check size
            let size = std::fs::metadata(path)
                .map(|m| m.len())
                .unwrap_or(0);

            if size < a.min_size { continue; }

            let size_str = if size >= 1024 * 1024 {
                format!("{:.2} MB", size as f64 / (1024.0 * 1024.0))
            } else {
                format!("{:.2} KB", size as f64 / 1024.0)
            };

            files.push(json!({ "path": path, "size": size_str, "bytes": size }));
        }

        Ok(json!({
            "log_directory": log_dir,
            "total": files.len(),
            "files": files,
        }))
    }
}
