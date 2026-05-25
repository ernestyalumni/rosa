//! `ros2_param_set` — wraps `ros2 param set <node> <name> <value>`.

use async_trait::async_trait;
use schemars::{schema_for, schema::RootSchema, JsonSchema};
use serde::Deserialize;
use serde_json::{json, Value};

use rosa_core::error::Result;
use rosa_tools::Tool;

use crate::runner::{SharedRunner, ShellRunner};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ParamSetArgs {
    /// Node that owns the parameter (e.g. `/turtlesim`).
    pub node: String,
    /// Parameter name (e.g. `background_r`).
    pub name: String,
    /// New value as a string (e.g. `"255"`, `"true"`, `"3.14"`).
    pub value: String,
}

pub struct ParamSetTool {
    runner: SharedRunner,
}

impl ParamSetTool {
    pub fn new(_blacklist: Vec<String>) -> Self {
        Self { runner: ShellRunner::shared() }
    }

    #[cfg(test)]
    pub fn with_runner(runner: SharedRunner, _blacklist: Vec<String>) -> Self {
        Self { runner }
    }
}

#[async_trait]
impl Tool for ParamSetTool {
    fn name(&self) -> &str { "ros2_param_set" }

    fn description(&self) -> &str {
        "Set a ROS 2 parameter on a running node. \
         Supply the node name (e.g. /turtlesim), parameter name, and new value as a string."
    }

    fn schema(&self) -> RootSchema { schema_for!(ParamSetArgs) }

    async fn execute(&self, args: Value) -> Result<Value> {
        let a: ParamSetArgs = serde_json::from_value(args)?;
        let raw = self.runner
            .run(&["param", "set", &a.node, &a.name, &a.value])
            .await?;
        Ok(json!({
            "node":  a.node,
            "name":  a.name,
            "value": a.value,
            "result": raw.trim()
        }))
    }
}
