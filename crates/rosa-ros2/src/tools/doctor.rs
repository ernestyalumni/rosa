//! `ros2_doctor` — wraps `ros2 doctor --report`.

use async_trait::async_trait;
use schemars::{schema_for, schema::RootSchema, JsonSchema};
use serde::Deserialize;
use serde_json::{json, Value};

use rosa_core::error::Result;
use rosa_tools::Tool;

use crate::runner::{SharedRunner, ShellRunner};

/// Empty args struct — ros2 doctor takes no arguments.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DoctorArgs {}

pub struct DoctorTool {
    runner: SharedRunner,
}

impl DoctorTool {
    pub fn new(_blacklist: Vec<String>) -> Self {
        Self { runner: ShellRunner::shared() }
    }

    #[cfg(test)]
    pub fn with_runner(runner: SharedRunner, _blacklist: Vec<String>) -> Self {
        Self { runner }
    }
}

#[async_trait]
impl Tool for DoctorTool {
    fn name(&self) -> &str { "ros2_doctor" }

    fn description(&self) -> &str {
        "Run `ros2 doctor --report` and return the full diagnostic report. \
         Use this to check ROS 2 system health and DDS configuration."
    }

    fn schema(&self) -> RootSchema { schema_for!(DoctorArgs) }

    async fn execute(&self, args: Value) -> Result<Value> {
        let _: DoctorArgs = serde_json::from_value(args)?;
        let raw = self.runner.run(&["doctor", "--report"]).await?;
        Ok(json!({ "report": raw.trim() }))
    }
}
