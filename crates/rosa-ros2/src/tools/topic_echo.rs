//! `ros2_topic_echo` — wraps `ros2 topic echo --once <topic> <msg_type>`.

use async_trait::async_trait;
use schemars::{schema_for, schema::RootSchema, JsonSchema};
use serde::Deserialize;
use serde_json::{json, Value};

use rosa_core::error::Result;
use rosa_tools::Tool;

use crate::runner::{SharedRunner, ShellRunner};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TopicEchoArgs {
    /// Topic to echo (e.g. `/turtle1/pose`).
    pub topic: String,
    /// Message type (e.g. `turtlesim/msg/Pose`). Required by `ros2 topic echo`.
    pub msg_type: String,
}

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
        "Echo the next message published on a ROS 2 topic (--once). \
         Returns the message as a YAML string."
    }

    fn schema(&self) -> RootSchema { schema_for!(TopicEchoArgs) }

    async fn execute(&self, args: Value) -> Result<Value> {
        let a: TopicEchoArgs = serde_json::from_value(args)?;
        let raw = self
            .runner
            .run(&["topic", "echo", "--once", &a.topic, &a.msg_type])
            .await?;
        Ok(json!({ "message": raw.trim() }))
    }
}
