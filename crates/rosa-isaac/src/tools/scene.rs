//! USD scene tools: load_usd, list_usds, create_starship_stage, starship_stage_status.

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

// ── CreateStarshipStageTool ────────────────────────────────────────────────────

/// Arguments for `create_starship_stage` — none required.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateStarshipStageArgs {}

/// Generate the Starship USD stage (`/isaac-sim/exts/starship/starship.usd`).
///
/// The stage contains a capsule rigid body representing the Starship vehicle,
/// a ground collision plane, and a camera at the nose cone.  This only needs
/// to be called ONCE; after that, use `load_usd` with the stage path.
///
/// USD generation runs inside the Isaac Sim process (pxr bindings require
/// the Kit runtime).  Poll `starship_stage_status` to confirm completion.
pub struct CreateStarshipStageTool {
    client: IsaacClient,
}

impl CreateStarshipStageTool {
    pub fn new(client: IsaacClient) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Tool for CreateStarshipStageTool {
    fn name(&self) -> &str {
        "create_starship_stage"
    }

    fn description(&self) -> &str {
        "Generate the Starship USD stage inside Isaac Sim.  \
         Creates `/isaac-sim/exts/starship/starship.usd` with a capsule \
         rigid body, ground plane, physics scene, and nose-cone camera.  \
         Only needed once; after that load the stage with `load_usd`.  \
         Poll `starship_stage_status` after ~5s to confirm the file exists."
    }

    fn schema(&self) -> RootSchema {
        schema_for!(CreateStarshipStageArgs)
    }

    async fn execute(&self, _args: Value) -> Result<Value> {
        self.client
            .post("/starship/create-stage", None)
            .await
            .map_err(|e| RosaError::Other(e.to_string()))?;

        Ok(json!({
            "status": "queued",
            "output_path": "/isaac-sim/exts/starship/starship.usd",
            "note": "stage creation is asynchronous — call starship_stage_status after 5s"
        }))
    }
}

// ── StarshipStageStatusTool ────────────────────────────────────────────────────

/// Arguments for `starship_stage_status` — none required.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct StarshipStageStatusArgs {}

/// Check whether the Starship USD stage file has been created.
pub struct StarshipStageStatusTool {
    client: IsaacClient,
}

impl StarshipStageStatusTool {
    pub fn new(client: IsaacClient) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Tool for StarshipStageStatusTool {
    fn name(&self) -> &str {
        "starship_stage_status"
    }

    fn description(&self) -> &str {
        "Check whether `/isaac-sim/exts/starship/starship.usd` exists.  \
         Returns `exists: true` and `size_bytes` when the stage is ready.  \
         Use this after `create_starship_stage` before calling `load_usd`."
    }

    fn schema(&self) -> RootSchema {
        schema_for!(StarshipStageStatusArgs)
    }

    async fn execute(&self, _args: Value) -> Result<Value> {
        self.client
            .get("/starship/stage-status")
            .await
            .map_err(|e| RosaError::Other(e.to_string()))
    }
}
