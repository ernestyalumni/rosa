//! `ros2_list_topics` — wraps `ros2 topic list -t`.
//!
//! Output format from `ros2 topic list -t`:
//! ```text
//! /clock [std_msgs/msg/Time]
//! /parameter_events [rcl_interfaces/msg/ParameterEvent]
//! ```
//! Parsed into `{ "topics": [{ "name": "…", "type": "…" }] }`.

use async_trait::async_trait;
use schemars::{schema_for, schema::RootSchema, JsonSchema};
use serde::Deserialize;
use serde_json::{json, Value};

use rosa_core::error::Result;
use rosa_tools::Tool;

use crate::{filter::filter_lines, runner::{SharedRunner, ShellRunner}};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListTopicsArgs {
    /// Optional regex pattern applied to topic names.
    pub pattern: Option<String>,
}

pub struct ListTopicsTool {
    runner:    SharedRunner,
    blacklist: Vec<String>,
}

impl ListTopicsTool {
    pub fn new(blacklist: Vec<String>) -> Self {
        Self { runner: ShellRunner::shared(), blacklist }
    }

    #[cfg(test)]
    pub fn with_runner(runner: SharedRunner, blacklist: Vec<String>) -> Self {
        Self { runner, blacklist }
    }
}

#[async_trait]
impl Tool for ListTopicsTool {
    fn name(&self) -> &str { "ros2_list_topics" }

    fn description(&self) -> &str {
        "List all ROS 2 topics with their types. \
         Optionally filter by a regex `pattern` matched against topic names."
    }

    fn schema(&self) -> RootSchema { schema_for!(ListTopicsArgs) }

    async fn execute(&self, args: Value) -> Result<Value> {
        let a: ListTopicsArgs = serde_json::from_value(args)?;
        let raw = self.runner.run(&["topic", "list", "-t"]).await?;

        // Parse "name [type]" lines
        let lines = filter_lines(&raw, &self.blacklist, a.pattern.as_deref());
        let topics: Vec<Value> = lines
            .iter()
            .map(|line| {
                // Split at the first '[' to extract name and type
                if let Some(bracket) = line.find('[') {
                    let name = line[..bracket].trim().to_owned();
                    let typ  = line[bracket + 1..].trim_end_matches(']').trim().to_owned();
                    json!({ "name": name, "type": typ })
                } else {
                    json!({ "name": line, "type": "" })
                }
            })
            .collect();

        Ok(json!({ "topics": topics }))
    }
}
