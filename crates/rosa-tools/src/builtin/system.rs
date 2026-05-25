//! `system_info` — query basic host information via subprocess.
//!
//! Runs `hostname`, `uname -a`, or `date -u` and returns their output.
//! All subprocesses run non-blocking via `tokio::process::Command`.

use async_trait::async_trait;
use schemars::{schema_for, schema::RootSchema, JsonSchema};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::process::Command;

use rosa_core::error::Result;

use crate::tool::Tool;

// ---------------------------------------------------------------------------
// Arg struct
// ---------------------------------------------------------------------------

fn default_query() -> String {
    "all".to_owned()
}

/// Arguments for the `system_info` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SystemInfoArgs {
    /// Which information to retrieve.
    ///
    /// - `"hostname"` — machine hostname (`hostname`)
    /// - `"uname"`    — kernel/OS details (`uname -a`)
    /// - `"date"`     — current UTC time in ISO-8601 (`date -u +%Y-%m-%dT%H:%M:%SZ`)
    /// - `"all"`      — all three (default)
    #[serde(default = "default_query")]
    pub query: String,
}

// ---------------------------------------------------------------------------
// Tool impl
// ---------------------------------------------------------------------------

/// Return system information (hostname, uname, date) via subprocesses.
pub struct SystemInfoTool;

#[async_trait]
impl Tool for SystemInfoTool {
    fn name(&self) -> &str {
        "system_info"
    }

    fn description(&self) -> &str {
        "Return system information: hostname, kernel version (uname), or current UTC \
         date/time. Pass query='all' (default) for all three."
    }

    fn schema(&self) -> RootSchema {
        schema_for!(SystemInfoArgs)
    }

    async fn execute(&self, args: Value) -> Result<Value> {
        let a: SystemInfoArgs = serde_json::from_value(args)?;

        let queries: Vec<&str> = match a.query.as_str() {
            "all"      => vec!["hostname", "uname", "date"],
            other      => vec![other],
        };

        let mut result = serde_json::Map::new();
        for q in queries {
            let out = run_query(q).await;
            match out {
                Ok(text) => { result.insert(q.to_owned(), json!(text)); }
                Err(e)   => { result.insert(q.to_owned(), json!(format!("error: {e}"))); }
            }
        }

        Ok(Value::Object(result))
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async fn run_query(query: &str) -> std::result::Result<String, String> {
    let (program, args): (&str, &[&str]) = match query {
        "hostname" => ("hostname", &[]),
        "uname"    => ("uname",    &["-a"]),
        "date"     => ("date",     &["-u", "+%Y-%m-%dT%H:%M:%SZ"]),
        other      => return Err(format!("unknown query '{other}' — valid: hostname, uname, date, all")),
    };

    let output = Command::new(program)
        .args(args)
        .output()
        .await
        .map_err(|e| format!("spawn '{program}': {e}"))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_owned())
    }
}
