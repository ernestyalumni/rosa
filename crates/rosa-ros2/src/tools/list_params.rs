//! `ros2_list_params` — wraps `ros2 param list [node]`.

use async_trait::async_trait;
use schemars::{schema_for, schema::RootSchema, JsonSchema};
use serde::Deserialize;
use serde_json::{json, Value};

use rosa_core::error::Result;
use rosa_tools::Tool;

use crate::{filter::filter_lines, runner::{SharedRunner, ShellRunner}};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListParamsArgs {
    /// Restrict listing to a specific node (e.g. `/turtlesim`). Lists all nodes when omitted.
    pub node: Option<String>,

    /// Optional regex pattern; only parameter names matching it are returned.
    /// Example: `"background_.*"` to see only colour parameters on `/turtlesim`.
    pub pattern: Option<String>,
}

pub struct ListParamsTool {
    runner:    SharedRunner,
    blacklist: Vec<String>,
}

impl ListParamsTool {
    pub fn new(blacklist: Vec<String>) -> Self {
        Self { runner: ShellRunner::shared(), blacklist }
    }

    #[cfg(test)]
    pub fn with_runner(runner: SharedRunner, blacklist: Vec<String>) -> Self {
        Self { runner, blacklist }
    }
}

#[async_trait]
impl Tool for ListParamsTool {
    fn name(&self) -> &str { "ros2_list_params" }

    fn description(&self) -> &str {
        "List ROS 2 parameters. Optionally restrict to a specific `node` name \
         and/or filter parameter names with a regex `pattern` \
         (e.g. `\"background_.*\"` to see only colour parameters on /turtlesim)."
    }

    fn schema(&self) -> RootSchema { schema_for!(ListParamsArgs) }

    async fn execute(&self, args: Value) -> Result<Value> {
        let a: ListParamsArgs = serde_json::from_value(args)?;

        let cmd: Vec<&str> = if let Some(ref node) = a.node {
            vec!["param", "list", node.as_str()]
        } else {
            vec!["param", "list"]
        };

        let raw = self.runner.run(&cmd).await?;
        let params = filter_lines(&raw, &self.blacklist, a.pattern.as_deref());
        Ok(json!({ "params": params }))
    }
}
