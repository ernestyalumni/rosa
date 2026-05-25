//! Tool registry — holds `Box<dyn Tool>`, dispatches by name, and converts
//! the tool set to provider-specific JSON shapes for request bodies.

use async_trait::async_trait;
use serde_json::{json, Value};

use rosa_core::{
    agent::ToolDispatcher,
    error::{Result, RosaError},
    provider::ToolSpec,
};

use crate::tool::Tool;

// ---------------------------------------------------------------------------
// ToolRegistry
// ---------------------------------------------------------------------------

/// Thread-safe collection of `Tool`s with dispatch-by-name and
/// provider-specific JSON serialisation.
///
/// ## Fluent builder
/// ```no_run
/// use rosa_tools::{ToolRegistry, builtin::{AddTool, LogMessageTool, SystemInfoTool}};
///
/// let registry = ToolRegistry::new()
///     .register(AddTool)
///     .register(LogMessageTool)
///     .register(SystemInfoTool)
///     .with_blacklist(vec!["master".into(), "docker".into()]);
/// ```
pub struct ToolRegistry {
    tools:     Vec<Box<dyn Tool>>,
    blacklist: Vec<String>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            tools:     Vec::new(),
            blacklist: Vec::new(),
        }
    }

    /// Add a tool to the registry (builder pattern — consumes `self`).
    pub fn register(mut self, tool: impl Tool + 'static) -> Self {
        self.tools.push(Box::new(tool));
        self
    }

    /// Set the blacklist injected into tools that opt-in via
    /// `Tool::requires_blacklist()`.
    pub fn with_blacklist(mut self, blacklist: Vec<String>) -> Self {
        self.blacklist = blacklist;
        self
    }

    // -----------------------------------------------------------------------
    // Provider-specific serialisation
    // -----------------------------------------------------------------------

    /// Serialise all tools into OpenAI's `tools` array format.
    ///
    /// ```json
    /// [{ "type": "function", "function": { "name": "…", "description": "…", "parameters": {…} } }]
    /// ```
    pub fn as_openai_tools(&self) -> Vec<Value> {
        self.tools
            .iter()
            .map(|t| {
                let spec = t.to_spec();
                json!({
                    "type": "function",
                    "function": {
                        "name":        spec.name,
                        "description": spec.description,
                        "parameters":  spec.parameters,
                    },
                })
            })
            .collect()
    }

    /// Serialise all tools into Anthropic's `tools` array format.
    ///
    /// ```json
    /// [{ "name": "…", "description": "…", "input_schema": {…} }]
    /// ```
    pub fn as_anthropic_tools(&self) -> Vec<Value> {
        self.tools
            .iter()
            .map(|t| {
                let spec = t.to_spec();
                json!({
                    "name":         spec.name,
                    "description":  spec.description,
                    "input_schema": spec.parameters,
                })
            })
            .collect()
    }

    /// Number of registered tools.
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// Returns `true` if no tools are registered.
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }
}

// ---------------------------------------------------------------------------
// ToolDispatcher impl (used by Agent loop)
// ---------------------------------------------------------------------------

#[async_trait]
impl ToolDispatcher for ToolRegistry {
    fn tool_specs(&self) -> Vec<ToolSpec> {
        self.tools.iter().map(|t| t.to_spec()).collect()
    }

    async fn dispatch(&self, name: &str, mut args: Value) -> Result<Value> {
        let tool = self
            .tools
            .iter()
            .find(|t| t.name() == name)
            .ok_or_else(|| RosaError::ToolNotFound { name: name.to_owned() })?;

        // Inject blacklist for tools that opt in
        if tool.requires_blacklist() && !self.blacklist.is_empty() {
            if let Value::Object(ref mut map) = args {
                map.insert("_blacklist".to_owned(), json!(self.blacklist));
            }
        }

        // Promote Json deserialization errors to ToolBadArgs for better context
        tool.execute(args).await.map_err(|e| match e {
            RosaError::Json(source) => RosaError::ToolBadArgs {
                name:   name.to_owned(),
                source,
            },
            other => other,
        })
    }
}
