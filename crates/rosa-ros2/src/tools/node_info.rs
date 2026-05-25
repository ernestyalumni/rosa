//! `ros2_node_info` — wraps `ros2 node info <node>`.

use async_trait::async_trait;
use schemars::{schema_for, schema::RootSchema, JsonSchema};
use serde::Deserialize;
use serde_json::{json, Value};

use rosa_core::error::Result;
use rosa_tools::Tool;

use crate::runner::{SharedRunner, ShellRunner};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct NodeInfoArgs {
    /// Full node name to inspect (e.g. `/turtlesim`).
    pub node: String,
}

pub struct NodeInfoTool {
    runner: SharedRunner,
}

impl NodeInfoTool {
    pub fn new(_blacklist: Vec<String>) -> Self {
        Self { runner: ShellRunner::shared() }
    }

    #[cfg(test)]
    pub fn with_runner(runner: SharedRunner, _blacklist: Vec<String>) -> Self {
        Self { runner }
    }
}

#[async_trait]
impl Tool for NodeInfoTool {
    fn name(&self) -> &str { "ros2_node_info" }

    fn description(&self) -> &str {
        "Show information about a ROS 2 node: subscribers, publishers, \
         service servers, and service clients."
    }

    fn schema(&self) -> RootSchema { schema_for!(NodeInfoArgs) }

    async fn execute(&self, args: Value) -> Result<Value> {
        let a: NodeInfoArgs = serde_json::from_value(args)?;
        let raw = self.runner.run(&["node", "info", &a.node]).await?;
        Ok(json!({ "info": raw.trim() }))
    }
}
