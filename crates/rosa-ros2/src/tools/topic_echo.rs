//! `ros2_topic_echo` — wraps `ros2 topic echo --once <topic> [<msg_type>]`.
//!
//! ## Design choices
//!
//! **`msg_type` is optional.**  The `ros2 topic echo` CLI discovers the message
//! type automatically from topic metadata, so passing it is never required.
//! Keeping it optional removes the need for a prior `ros2_topic_info` call and
//! lets the LLM echo any topic with a single tool call.
//!
//! **`count` uses sequential `--once` calls.**  There is no `--times N` flag on
//! `ros2 topic echo`.  Rather than leave a continuous-echo process running (which
//! would be killed by the runner timeout and leak a child process), we issue
//! multiple `--once` calls.  For the 1–10 message range this is clean and safe.

use async_trait::async_trait;
use schemars::{schema_for, schema::RootSchema, JsonSchema};
use serde::Deserialize;
use serde_json::{json, Value};

use rosa_core::error::Result;
use rosa_tools::Tool;

use crate::runner::{SharedRunner, ShellRunner};

// ---------------------------------------------------------------------------
// Args
// ---------------------------------------------------------------------------

fn default_count() -> u8 { 1 }

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TopicEchoArgs {
    /// Topic to echo (e.g. `/turtle1/pose`).
    pub topic: String,

    /// Message type (e.g. `turtlesim/msg/Pose`).
    /// **Optional** — ROS 2 discovers the type automatically from topic metadata.
    /// Provide it only when you already know the type and want to skip the
    /// automatic discovery step.
    #[serde(default)]
    pub msg_type: Option<String>,

    /// Number of messages to capture (1–10, default 1).
    /// Each message is fetched with a separate `--once` invocation so there
    /// is no open-ended process to manage.
    #[serde(default = "default_count")]
    pub count: u8,
}

// ---------------------------------------------------------------------------
// Tool
// ---------------------------------------------------------------------------

pub struct TopicEchoTool {
    runner: SharedRunner,
}

impl TopicEchoTool {
    pub fn new(_blacklist: Vec<String>) -> Self {
        Self { runner: ShellRunner::shared() }
    }

    #[cfg(test)]
    pub fn with_runner(runner: SharedRunner, _blacklist: Vec<String>) -> Self {
        Self { runner }
    }
}

#[async_trait]
impl Tool for TopicEchoTool {
    fn name(&self) -> &str { "ros2_topic_echo" }

    fn description(&self) -> &str {
        "Echo messages published on a ROS 2 topic. \
         `msg_type` is optional — ROS 2 discovers the type automatically. \
         `count` (1–10, default 1) controls how many messages to capture; \
         each is fetched with `--once`. \
         Returns `message` (string) for count=1 or a `messages` array for count>1."
    }

    fn schema(&self) -> RootSchema { schema_for!(TopicEchoArgs) }

    async fn execute(&self, args: Value) -> Result<Value> {
        let a: TopicEchoArgs = serde_json::from_value(args)?;
        let count = a.count.clamp(1, 10);

        // Build the base arg list; msg_type is appended only when provided.
        // Use owned Strings so we can borrow them as &str below.
        let mut cmd_base: Vec<String> = vec![
            "topic".into(),
            "echo".into(),
            "--once".into(),
            a.topic.clone(),
        ];
        if let Some(ref t) = a.msg_type {
            cmd_base.push(t.clone());
        }

        if count == 1 {
            let refs: Vec<&str> = cmd_base.iter().map(String::as_str).collect();
            let raw = self.runner.run(&refs).await?;
            return Ok(json!({
                "topic":   a.topic,
                "message": raw.trim(),
                "count":   1,
            }));
        }

        // For count > 1: call --once sequentially N times.
        // Errors on individual messages are captured as strings so a transient
        // drop doesn't abort the whole sequence.
        let mut messages: Vec<String> = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let refs: Vec<&str> = cmd_base.iter().map(String::as_str).collect();
            match self.runner.run(&refs).await {
                Ok(raw) => messages.push(raw.trim().to_owned()),
                Err(e)  => messages.push(format!("error: {e}")),
            }
        }

        Ok(json!({
            "topic":    a.topic,
            "messages": messages,
            "count":    messages.len(),
        }))
    }
}
