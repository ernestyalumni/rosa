//! `ros2_list_nodes` — wraps `ros2 node list`.

use async_trait::async_trait;
use schemars::{schema_for, schema::RootSchema, JsonSchema};
use serde::Deserialize;
use serde_json::{json, Value};

use rosa_core::error::Result;
use rosa_tools::Tool;

use crate::{filter::filter_lines, runner::{SharedRunner, ShellRunner}};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListNodesArgs {
    /// Optional regex pattern; only node names matching it are returned.
    pub pattern: Option<String>,
}

pub struct ListNodesTool {
    runner:    SharedRunner,
    blacklist: Vec<String>,
}

impl ListNodesTool {
    pub fn new(blacklist: Vec<String>) -> Self {
        Self { runner: ShellRunner::shared(), blacklist }
    }

    #[cfg(test)]
    pub fn with_runner(runner: SharedRunner, blacklist: Vec<String>) -> Self {
        Self { runner, blacklist }
    }
}

#[async_trait]
impl Tool for ListNodesTool {
    fn name(&self) -> &str { "ros2_list_nodes" }

    fn description(&self) -> &str {
        "List all running ROS 2 nodes. Optionally filter by a regex `pattern`."
    }

    fn schema(&self) -> RootSchema { schema_for!(ListNodesArgs) }

    async fn execute(&self, args: Value) -> Result<Value> {
        let a: ListNodesArgs = serde_json::from_value(args)?;
        let raw = self.runner.run(&["node", "list"]).await?;
        let nodes = filter_lines(&raw, &self.blacklist, a.pattern.as_deref());
        Ok(json!({ "nodes": nodes }))
    }
}
