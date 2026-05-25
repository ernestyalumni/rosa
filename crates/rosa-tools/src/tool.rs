//! The `Tool` trait — the single interface every rosa tool must implement.

use async_trait::async_trait;
use schemars::schema::RootSchema;
use serde_json::Value;

use rosa_core::{provider::ToolSpec, error::Result};

// ---------------------------------------------------------------------------
// Core trait
// ---------------------------------------------------------------------------

/// A callable tool the agent loop can dispatch to.
///
/// Implementations must be `Send + Sync` so they can be held behind
/// `Box<dyn Tool>` and used across async await points.
///
/// ## Minimal implementation
/// ```no_run
/// use async_trait::async_trait;
/// use schemars::{schema_for, schema::RootSchema, JsonSchema};
/// use serde::Deserialize;
/// use serde_json::{json, Value};
/// use rosa_tools::Tool;
/// use rosa_core::error::Result;
///
/// #[derive(Deserialize, JsonSchema)]
/// struct AddArgs { a: f64, b: f64 }
///
/// struct AddTool;
///
/// #[async_trait]
/// impl Tool for AddTool {
///     fn name(&self) -> &str { "add" }
///     fn description(&self) -> &str { "Add two numbers." }
///     fn schema(&self) -> RootSchema { schema_for!(AddArgs) }
///     async fn execute(&self, args: Value) -> Result<Value> {
///         let a: AddArgs = serde_json::from_value(args)?;
///         Ok(json!(a.a + a.b))
///     }
/// }
/// ```
#[async_trait]
pub trait Tool: Send + Sync {
    /// Stable, snake_case identifier — must be unique within a registry.
    fn name(&self) -> &str;

    /// Human-readable one-liner; passed verbatim to the LLM.
    fn description(&self) -> &str;

    /// JSON Schema for the tool's input arguments (derived via `schemars`).
    fn schema(&self) -> RootSchema;

    /// Opt-in: receive the caller's blacklist under the `_blacklist` key.
    ///
    /// Upstream rosa injected blacklists into every tool. Here it's opt-in so
    /// provider-agnostic tools don't need to know about ROS node filtering.
    fn requires_blacklist(&self) -> bool {
        false
    }

    /// Execute the tool with the given JSON arguments.
    ///
    /// Return `Err(RosaError::Json(_))` on bad argument types — the registry's
    /// `dispatch` promotes that to `RosaError::ToolBadArgs { name, source }`.
    async fn execute(&self, args: Value) -> Result<Value>;

    /// Convert to the provider-agnostic `ToolSpec` used by the agent loop.
    ///
    /// The default implementation serialises `schema()` into `parameters`;
    /// override only if you need custom schema tweaks.
    fn to_spec(&self) -> ToolSpec {
        ToolSpec {
            name: self.name().to_owned(),
            description: self.description().to_owned(),
            parameters: serde_json::to_value(self.schema())
                .unwrap_or_else(|_| serde_json::json!({ "type": "object" })),
        }
    }
}
