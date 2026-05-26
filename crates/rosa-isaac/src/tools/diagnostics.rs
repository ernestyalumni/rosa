//! Diagnostics tool — query Isaac Sim internal stats.

use async_trait::async_trait;
use schemars::{schema_for, schema::RootSchema, JsonSchema};
use serde::Deserialize;
use serde_json::Value;

use rosa_core::error::{Result, RosaError};
use rosa_tools::Tool;

use crate::IsaacClient;

/// Arguments for `get_diagnostics` — none required.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DiagnosticsArgs {}

/// Query Isaac Sim's current simulation diagnostics.
///
/// Returns a JSON object with fields like `fps`, `sim_time`, `running`, and
/// `physics_dt`.  Use this to verify the simulation is healthy before issuing
/// movement or sensor commands.
pub struct GetDiagnosticsTool {
    client: IsaacClient,
}

impl GetDiagnosticsTool {
    pub fn new(client: IsaacClient) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Tool for GetDiagnosticsTool {
    fn name(&self) -> &str {
        "get_diagnostics"
    }

    fn description(&self) -> &str {
        "Query Isaac Sim's current simulation diagnostics.  \
         Returns a JSON object with: \
         `running` (bool) — whether the timeline is playing; \
         `sim_time` (float) — elapsed simulation seconds; \
         `fps` (float) — rendering / physics frames per second; \
         `physics_dt` (float) — physics time step in seconds.  \
         Interpret these stats to confirm the simulation is healthy before \
         issuing robot commands.  A typical healthy sim runs at 60 fps with \
         physics_dt=0.01667 s."
    }

    fn schema(&self) -> RootSchema {
        schema_for!(DiagnosticsArgs)
    }

    async fn execute(&self, _args: Value) -> Result<Value> {
        self.client
            .get("/diagnostics")
            .await
            .map_err(|e| RosaError::Other(e.to_string()))
    }
}
