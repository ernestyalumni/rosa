//! `ros2_service_info` — wraps `ros2 service type <service>` for one or more services.
//!
//! Returns the ROS 2 message type for each requested service name.
//! Equivalent to the Python `ros2_service_info` tool.

use async_trait::async_trait;
use schemars::{schema_for, schema::RootSchema, JsonSchema};
use serde::Deserialize;
use serde_json::{json, Value};

use rosa_core::error::Result;
use rosa_tools::Tool;

use crate::runner::{SharedRunner, ShellRunner};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ServiceInfoArgs {
    /// One or more fully-qualified service names to query,
    /// e.g. `["/turtlesim/spawn", "/turtlesim/kill"]`.
    pub services: Vec<String>,
}

pub struct ServiceInfoTool {
    runner: SharedRunner,
}

impl ServiceInfoTool {
    pub fn new(_blacklist: Vec<String>) -> Self {
        Self { runner: ShellRunner::shared() }
    }

    #[cfg(test)]
    pub fn with_runner(runner: SharedRunner, _blacklist: Vec<String>) -> Self {
        Self { runner }
    }
}

#[async_trait]
impl Tool for ServiceInfoTool {
    fn name(&self) -> &str { "ros2_service_info" }

    fn description(&self) -> &str {
        "Return the ROS 2 service type for one or more service names. \
         Use this to look up what message type a service expects before calling it."
    }

    fn schema(&self) -> RootSchema { schema_for!(ServiceInfoArgs) }

    async fn execute(&self, args: Value) -> Result<Value> {
        let a: ServiceInfoArgs = serde_json::from_value(args)?;
        let mut data = serde_json::Map::new();

        for svc in &a.services {
            match self.runner.run(&["service", "type", svc]).await {
                Ok(out) => {
                    data.insert(svc.clone(), json!(out.trim()));
                }
                Err(e) => {
                    data.insert(svc.clone(), json!({ "error": e.to_string() }));
                }
            }
        }

        Ok(Value::Object(data))
    }
}
