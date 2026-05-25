//! `ros2_param_get` — wraps `ros2 param get <node> <name>`.

use async_trait::async_trait;
use schemars::{schema_for, schema::RootSchema, JsonSchema};
use serde::Deserialize;
use serde_json::{json, Value};

use rosa_core::error::Result;
use rosa_tools::Tool;

use crate::runner::{SharedRunner, ShellRunner};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ParamGetArgs {
    /// Node that owns the parameter (e.g. `/turtlesim`).
    pub node: String,
    /// Parameter name (e.g. `background_r`).
    pub name: String,
}

pub struct ParamGetTool {
    runner: SharedRunner,
}

impl ParamGetTool {
    pub fn new(_blacklist: Vec<String>) -> Self {
        Self { runner: ShellRunner::shared() }
    }

    #[cfg(test)]
    pub fn with_runner(runner: SharedRunner, _blacklist: Vec<String>) -> Self {
        Self { runner }
    }
}

#[async_trait]
impl Tool for ParamGetTool {
    fn name(&self) -> &str { "ros2_param_get" }

    fn description(&self) -> &str {
        "Get the current value of a ROS 2 parameter from a running node."
    }

    fn schema(&self) -> RootSchema { schema_for!(ParamGetArgs) }

    async fn execute(&self, args: Value) -> Result<Value> {
        let a: ParamGetArgs = serde_json::from_value(args)?;
        let raw = self.runner.run(&["param", "get", &a.node, &a.name]).await?;
        Ok(json!({ "node": a.node, "name": a.name, "value": raw.trim() }))
    }
}
