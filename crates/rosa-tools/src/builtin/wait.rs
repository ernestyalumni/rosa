//! `wait` — pause execution for a specified number of seconds.
//!
//! The agent uses this to let a turtlesim motion settle, wait for an Isaac Sim
//! scene to load, or introduce a deliberate delay between commands.

use async_trait::async_trait;
use schemars::{schema_for, schema::RootSchema, JsonSchema};
use serde::Deserialize;
use serde_json::{json, Value};

use rosa_core::error::{Result, RosaError};
use crate::tool::Tool;

/// Maximum delay the agent may request (avoid runaway waits).
const MAX_WAIT_SECS: f64 = 60.0;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct WaitArgs {
    /// Number of seconds to wait (0 < seconds ≤ 60).
    pub seconds: f64,
}

pub struct WaitTool;

#[async_trait]
impl Tool for WaitTool {
    fn name(&self) -> &str { "wait" }

    fn description(&self) -> &str {
        "Pause for the specified number of seconds (max 60). \
         Use after publishing a velocity command to let the robot move, \
         or to wait for a scene to settle."
    }

    fn schema(&self) -> RootSchema { schema_for!(WaitArgs) }

    async fn execute(&self, args: Value) -> Result<Value> {
        let a: WaitArgs = serde_json::from_value(args)?;
        if a.seconds <= 0.0 || a.seconds > MAX_WAIT_SECS {
            return Err(RosaError::ToolExecution {
                name: "wait".into(),
                message: format!(
                    "seconds must be in (0, {}], got {}",
                    MAX_WAIT_SECS, a.seconds
                ),
            });
        }
        tokio::time::sleep(std::time::Duration::from_secs_f64(a.seconds)).await;
        Ok(json!({ "waited_seconds": a.seconds }))
    }
}
