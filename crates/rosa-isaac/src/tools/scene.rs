//! USD scene tools: load_usd, list_usds.

use async_trait::async_trait;
use schemars::{schema_for, schema::RootSchema, JsonSchema};
use serde::Deserialize;
use serde_json::{json, Value};

use rosa_core::error::{Result, RosaError};
use rosa_tools::Tool;

use crate::IsaacClient;

// ── LoadUsdTool ───────────────────────────────────────────────────────────────

/// Arguments for `load_usd`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct LoadUsdArgs {
    /// Absolute path to the `.usd` / `.usda` / `.usdc` file to load inside
    /// the Isaac Sim container (e.g. `/isaac-sim/exts/starship/starship.usd`).
    pub path: String,
}

/// Load a USD scene into Isaac Sim.
///
/// The scene path must be valid inside the Isaac Sim container.  Call
/// `list_usds` first to discover available scenes.  Loading a new scene
/// replaces the current stage.
pub struct LoadUsdTool {
    client: IsaacClient,
}

impl LoadUsdTool {
    pub fn new(client: IsaacClient) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Tool for LoadUsdTool {
    fn name(&self) -> &str {
        "load_usd"
    }

    fn description(&self) -> &str {
        "Load a USD scene into the running Isaac Sim instance.  \
         The `path` must be an absolute path inside the Isaac Sim container \
         (e.g. `/isaac-sim/exts/starship/starship.usd`).  \
         Use `list_usds` to discover available scenes.  \
         Loading a new scene replaces the current stage; run `timeline_start` \
         afterwards to begin the simulation."
    }

    fn schema(&self) -> RootSchema {
        schema_for!(LoadUsdArgs)
    }

    async fn execute(&self, args: Value) -> Result<Value> {
        let a: LoadUsdArgs = serde_json::from_value(args)
            .map_err(|e| RosaError::Other(format!("bad args: {e}")))?;

        self.client
            .post("/scene/load", Some(&json!({ "path": a.path })))
            .await
            .map_err(|e| RosaError::Other(e.to_string()))?;

        Ok(json!({
            "status": "ok",
            "message": format!("Scene load queued: {}", a.path),
            "note": "scene loading is asynchronous — call get_diagnostics after 2s to confirm"
        }))
    }
}

// ── ListUsdsTool ──────────────────────────────────────────────────────────────

/// Arguments for `list_usds` — none required.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListUsdsArgs {}

/// List available USD scenes known to the running Isaac Sim instance.
pub struct ListUsdsTool {
    client: IsaacClient,
}

impl ListUsdsTool {
    pub fn new(client: IsaacClient) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Tool for ListUsdsTool {
    fn name(&self) -> &str {
        "list_usds"
    }

    fn description(&self) -> &str {
        "List the USD scene files available inside the running Isaac Sim instance.  \
         Returns a JSON array of absolute paths.  Pass one of these paths to \
         `load_usd` to open the scene."
    }

    fn schema(&self) -> RootSchema {
        schema_for!(ListUsdsArgs)
    }

    async fn execute(&self, _args: Value) -> Result<Value> {
        self.client
            .get("/scene/list")
            .await
            .map_err(|e| RosaError::Other(e.to_string()))
    }
}
