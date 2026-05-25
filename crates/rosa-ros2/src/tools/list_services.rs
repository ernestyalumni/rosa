//! `ros2_list_services` — wraps `ros2 service list -t`.

use async_trait::async_trait;
use schemars::{schema_for, schema::RootSchema, JsonSchema};
use serde::Deserialize;
use serde_json::{json, Value};

use rosa_core::error::Result;
use rosa_tools::Tool;

use crate::{filter::filter_lines, runner::{SharedRunner, ShellRunner}};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListServicesArgs {
    /// Optional regex pattern applied to service names.
    pub pattern: Option<String>,
}

pub struct ListServicesTool {
    runner:    SharedRunner,
    blacklist: Vec<String>,
}

impl ListServicesTool {
    pub fn new(blacklist: Vec<String>) -> Self {
        Self { runner: ShellRunner::shared(), blacklist }
    }

    #[cfg(test)]
    pub fn with_runner(runner: SharedRunner, blacklist: Vec<String>) -> Self {
        Self { runner, blacklist }
    }
}

#[async_trait]
impl Tool for ListServicesTool {
    fn name(&self) -> &str { "ros2_list_services" }

    fn description(&self) -> &str {
        "List all ROS 2 services with their types. \
         Optionally filter by a regex `pattern` matched against service names."
    }

    fn schema(&self) -> RootSchema { schema_for!(ListServicesArgs) }

    async fn execute(&self, args: Value) -> Result<Value> {
        let a: ListServicesArgs = serde_json::from_value(args)?;
        let raw = self.runner.run(&["service", "list", "-t"]).await?;

        let lines = filter_lines(&raw, &self.blacklist, a.pattern.as_deref());
        let services: Vec<Value> = lines
            .iter()
            .map(|line| {
                if let Some(bracket) = line.find('[') {
                    let name = line[..bracket].trim().to_owned();
                    let typ  = line[bracket + 1..].trim_end_matches(']').trim().to_owned();
                    json!({ "name": name, "type": typ })
                } else {
                    json!({ "name": line, "type": "" })
                }
            })
            .collect();

        Ok(json!({ "services": services }))
    }
}
