//! `ros2_topic_info` — wraps `ros2 topic info -v <topic>`.

use async_trait::async_trait;
use schemars::{schema_for, schema::RootSchema, JsonSchema};
use serde::Deserialize;
use serde_json::{json, Value};

use rosa_core::error::Result;
use rosa_tools::Tool;

use crate::runner::{SharedRunner, ShellRunner};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TopicInfoArgs {
    /// Topic to inspect (e.g. `/turtle1/cmd_vel`).
    pub topic: String,
}

pub struct TopicInfoTool {
    runner: SharedRunner,
}

impl TopicInfoTool {
    pub fn new(_blacklist: Vec<String>) -> Self {
        Self { runner: ShellRunner::shared() }
    }

    #[cfg(test)]
    pub fn with_runner(runner: SharedRunner, _blacklist: Vec<String>) -> Self {
        Self { runner }
    }
}

#[async_trait]
impl Tool for TopicInfoTool {
    fn name(&self) -> &str { "ros2_topic_info" }

    fn description(&self) -> &str {
        "Show verbose information about a ROS 2 topic: type, publisher count, \
         subscriber count, and endpoint details."
    }

    fn schema(&self) -> RootSchema { schema_for!(TopicInfoArgs) }

    async fn execute(&self, args: Value) -> Result<Value> {
        let a: TopicInfoArgs = serde_json::from_value(args)?;
        let raw = self.runner.run(&["topic", "info", "-v", &a.topic]).await?;
        Ok(json!({ "info": raw.trim() }))
    }
}
