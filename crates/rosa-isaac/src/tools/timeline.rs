//! Timeline control tools: start, stop, pause.

use async_trait::async_trait;
use schemars::{schema_for, schema::RootSchema, JsonSchema};
use serde::Deserialize;
use serde_json::{json, Value};

use rosa_core::error::{Result, RosaError};
use rosa_tools::Tool;

use crate::IsaacClient;

// ── Shared helper ─────────────────────────────────────────────────────────────

/// Shared args struct: none — all timeline commands take no arguments.
///
/// Having a named empty struct lets schemars emit `{"type":"object","properties":{}}`
/// instead of a bare `null`, which some LLM providers require.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct NoArgs {}

// ── TimelineStartTool ─────────────────────────────────────────────────────────

/// Start (play) the Isaac Sim simulation timeline.
pub struct TimelineStartTool {
    client: IsaacClient,
}

impl TimelineStartTool {
    pub fn new(client: IsaacClient) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Tool for TimelineStartTool {
    fn name(&self) -> &str {
        "timeline_start"
    }

    fn description(&self) -> &str {
        "Start (play) the Isaac Sim simulation timeline.  \
         After this call the simulation clock begins ticking and /clock messages \
         are published on the ROS 2 network.  \
         Call this before issuing any robot movement commands."
    }

    fn schema(&self) -> RootSchema {
        schema_for!(NoArgs)
    }

    async fn execute(&self, _args: Value) -> Result<Value> {
        self.client
            .post("/timeline/play", None)
            .await
            .map_err(|e| RosaError::Other(e.to_string()))?;
        Ok(json!({
            "status": "ok",
            "action": "play",
            "message": "Simulation timeline started — /clock is now publishing"
        }))
    }
}

// ── TimelineStopTool ──────────────────────────────────────────────────────────

/// Stop the Isaac Sim simulation timeline and reset to t=0.
pub struct TimelineStopTool {
    client: IsaacClient,
}

impl TimelineStopTool {
    pub fn new(client: IsaacClient) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Tool for TimelineStopTool {
    fn name(&self) -> &str {
        "timeline_stop"
    }

    fn description(&self) -> &str {
        "Stop the Isaac Sim simulation timeline and rewind to t=0.  \
         All robot state is reset to its initial configuration.  \
         Use timeline_pause instead if you want to freeze without resetting."
    }

    fn schema(&self) -> RootSchema {
        schema_for!(NoArgs)
    }

    async fn execute(&self, _args: Value) -> Result<Value> {
        self.client
            .post("/timeline/stop", None)
            .await
            .map_err(|e| RosaError::Other(e.to_string()))?;
        Ok(json!({
            "status": "ok",
            "action": "stop",
            "message": "Simulation timeline stopped and rewound to t=0"
        }))
    }
}

// ── TimelinePauseTool ─────────────────────────────────────────────────────────

/// Pause the Isaac Sim simulation timeline (freeze without resetting).
pub struct TimelinePauseTool {
    client: IsaacClient,
}

impl TimelinePauseTool {
    pub fn new(client: IsaacClient) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Tool for TimelinePauseTool {
    fn name(&self) -> &str {
        "timeline_pause"
    }

    fn description(&self) -> &str {
        "Pause the Isaac Sim simulation timeline.  \
         The simulation freezes at the current time step; all robot state is preserved.  \
         Resume with timeline_start."
    }

    fn schema(&self) -> RootSchema {
        schema_for!(NoArgs)
    }

    async fn execute(&self, _args: Value) -> Result<Value> {
        self.client
            .post("/timeline/pause", None)
            .await
            .map_err(|e| RosaError::Other(e.to_string()))?;
        Ok(json!({
            "status": "ok",
            "action": "pause",
            "message": "Simulation timeline paused — resume with timeline_start"
        }))
    }
}
