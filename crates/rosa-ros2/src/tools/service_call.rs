//! `ros2_service_call` — wraps `ros2 service call <service> <type> <yaml>`.
//!
//! This is the primary way to call turtlesim services like `/spawn`, `/kill`,
//! `/turtle1/teleport_absolute`, `/turtle1/set_pen`, etc.
//!
//! # Example
//! ```json
//! {
//!   "service": "/turtlesim/spawn",
//!   "srv_type": "turtlesim/srv/Spawn",
//!   "request": "{x: 2.0, y: 2.0, theta: 0.0, name: 'turtle2'}"
//! }
//! ```

use async_trait::async_trait;
use schemars::{schema_for, schema::RootSchema, JsonSchema};
use serde::Deserialize;
use serde_json::{json, Value};

use rosa_core::error::Result;
use rosa_tools::Tool;

use crate::runner::{SharedRunner, ShellRunner};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ServiceCallArgs {
    /// Fully-qualified service name (e.g. `/turtlesim/spawn`).
    pub service: String,
    /// ROS 2 service type (e.g. `turtlesim/srv/Spawn`).
    pub srv_type: String,
    /// YAML request body (e.g. `"{x: 1.0, y: 1.0, theta: 0.0, name: 'turtle2'}"`).
    /// Omit or use `"{}"` for services with no request fields.
    #[serde(default)]
    pub request: Option<String>,
}

pub struct ServiceCallTool {
    runner: SharedRunner,
}

impl ServiceCallTool {
    pub fn new(_blacklist: Vec<String>) -> Self {
        Self { runner: ShellRunner::shared() }
    }

    #[cfg(test)]
    pub fn with_runner(runner: SharedRunner, _blacklist: Vec<String>) -> Self {
        Self { runner }
    }
}

#[async_trait]
impl Tool for ServiceCallTool {
    fn name(&self) -> &str { "ros2_service_call" }

    fn description(&self) -> &str {
        "Call a ROS 2 service and return its response. \
         Use this to spawn/kill turtles, reset the sim, clear drawings, \
         teleport turtles, set pen colour/width, and more."
    }

    fn schema(&self) -> RootSchema { schema_for!(ServiceCallArgs) }

    async fn execute(&self, args: Value) -> Result<Value> {
        let a: ServiceCallArgs = serde_json::from_value(args)?;
        let request = a.request.as_deref().unwrap_or("{}");

        let raw = self.runner
            .run(&["service", "call", &a.service, &a.srv_type, request])
            .await?;

        Ok(json!({
            "service":  a.service,
            "srv_type": a.srv_type,
            "response": raw.trim()
        }))
    }
}
