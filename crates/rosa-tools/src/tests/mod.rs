//! Integration tests for ToolRegistry and built-in tools.
//!
//! Acceptance criteria (per agent-tasks/04-rust-tool-trait.md):
//! 1. Register a tool, dispatch by name.
//! 2. Unknown tool name → RosaError::ToolNotFound.
//! 3. Arg deserialize error → RosaError::ToolBadArgs (not a panic).
//! 4. Schema round-trips through OpenAI tool shape.
//! 5. Schema round-trips through Anthropic tool shape.
//! 6. Three builtin tools (add, log_message, system_info) registered + tested.

use serde_json::json;

use rosa_core::agent::ToolDispatcher;
use rosa_core::error::RosaError;

use crate::{
    builtin::{AddTool, LogMessageTool, SystemInfoTool},
    ToolRegistry,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn registry_with_all_builtins() -> ToolRegistry {
    ToolRegistry::new()
        .register(AddTool)
        .register(LogMessageTool)
        .register(SystemInfoTool)
}

// ---------------------------------------------------------------------------
// Registry basics
// ---------------------------------------------------------------------------

#[test]
fn test_registry_len_and_is_empty() {
    let r = ToolRegistry::new();
    assert!(r.is_empty());
    assert_eq!(r.len(), 0);

    let r = r.register(AddTool);
    assert!(!r.is_empty());
    assert_eq!(r.len(), 1);
}

#[test]
fn test_tool_specs_match_names() {
    let r = registry_with_all_builtins();
    let specs = r.tool_specs();
    assert_eq!(specs.len(), 3);
    let names: Vec<&str> = specs.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"add"));
    assert!(names.contains(&"log_message"));
    assert!(names.contains(&"system_info"));
}

// ---------------------------------------------------------------------------
// Dispatch: happy path
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_dispatch_add() {
    let r = registry_with_all_builtins();
    let result = r.dispatch("add", json!({"a": 3.0, "b": 4.0})).await.unwrap();
    assert_eq!(result, json!(7.0));
}

#[tokio::test]
async fn test_dispatch_add_negative() {
    let r = registry_with_all_builtins();
    let result = r.dispatch("add", json!({"a": -1.5, "b": 2.5})).await.unwrap();
    assert_eq!(result, json!(1.0));
}

#[tokio::test]
async fn test_dispatch_log_message() {
    let r = registry_with_all_builtins();
    let result = r
        .dispatch("log_message", json!({"message": "hello rosa", "level": "debug"}))
        .await
        .unwrap();
    assert_eq!(result["logged"], json!(true));
    assert_eq!(result["level"], json!("debug"));
    assert_eq!(result["message"], json!("hello rosa"));
}

#[tokio::test]
async fn test_dispatch_log_message_default_level() {
    let r = registry_with_all_builtins();
    // `level` is optional; default is "info"
    let result = r
        .dispatch("log_message", json!({"message": "default level test"}))
        .await
        .unwrap();
    assert_eq!(result["logged"], json!(true));
    assert_eq!(result["level"], json!("info"));
}

#[tokio::test]
async fn test_dispatch_system_info_hostname() {
    let r = registry_with_all_builtins();
    let result = r
        .dispatch("system_info", json!({"query": "hostname"}))
        .await
        .unwrap();
    // hostname field should be present and non-empty
    let hostname = result["hostname"].as_str().expect("hostname field missing");
    assert!(!hostname.is_empty(), "hostname should not be empty");
}

#[tokio::test]
async fn test_dispatch_system_info_all() {
    let r = registry_with_all_builtins();
    // Default query is "all"
    let result = r
        .dispatch("system_info", json!({}))
        .await
        .unwrap();
    assert!(result["hostname"].is_string(), "hostname key missing");
    assert!(result["uname"].is_string(),    "uname key missing");
    assert!(result["date"].is_string(),     "date key missing");
}

// ---------------------------------------------------------------------------
// Dispatch: error cases
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_dispatch_unknown_tool_returns_tool_not_found() {
    let r = registry_with_all_builtins();
    let err = r
        .dispatch("nonexistent_tool", json!({}))
        .await
        .unwrap_err();
    assert!(
        matches!(err, RosaError::ToolNotFound { ref name } if name == "nonexistent_tool"),
        "expected ToolNotFound, got: {err:?}"
    );
}

#[tokio::test]
async fn test_dispatch_bad_args_returns_tool_bad_args() {
    let r = registry_with_all_builtins();
    // `a` and `b` must be numbers, not strings
    let err = r
        .dispatch("add", json!({"a": "not-a-number", "b": 1.0}))
        .await
        .unwrap_err();
    assert!(
        matches!(err, RosaError::ToolBadArgs { ref name, .. } if name == "add"),
        "expected ToolBadArgs, got: {err:?}"
    );
}

// ---------------------------------------------------------------------------
// Schema serialisation — OpenAI shape
// ---------------------------------------------------------------------------

#[test]
fn test_as_openai_tools_shape() {
    let r = ToolRegistry::new().register(AddTool);
    let tools = r.as_openai_tools();
    assert_eq!(tools.len(), 1);

    let t = &tools[0];
    assert_eq!(t["type"], json!("function"));
    assert_eq!(t["function"]["name"], json!("add"));
    assert!(t["function"]["description"].is_string());

    let params = &t["function"]["parameters"];
    assert_eq!(params["type"], json!("object"));

    // Both `a` and `b` must appear in properties
    assert!(params["properties"]["a"].is_object(), "property 'a' missing");
    assert!(params["properties"]["b"].is_object(), "property 'b' missing");
}

// ---------------------------------------------------------------------------
// Schema serialisation — Anthropic shape
// ---------------------------------------------------------------------------

#[test]
fn test_as_anthropic_tools_shape() {
    let r = ToolRegistry::new().register(AddTool);
    let tools = r.as_anthropic_tools();
    assert_eq!(tools.len(), 1);

    let t = &tools[0];
    assert_eq!(t["name"], json!("add"));
    assert!(t["description"].is_string());

    let schema = &t["input_schema"];
    assert_eq!(schema["type"], json!("object"));
    assert!(schema["properties"]["a"].is_object(), "property 'a' missing in input_schema");
    assert!(schema["properties"]["b"].is_object(), "property 'b' missing in input_schema");
}

// ---------------------------------------------------------------------------
// Blacklist injection
// ---------------------------------------------------------------------------

/// A toy tool that opts into the blacklist so we can test injection.
mod blacklist_test_helpers {
    use async_trait::async_trait;
    use schemars::{schema_for, schema::RootSchema, JsonSchema};
    use serde::Deserialize;
    use serde_json::{json, Value};

    use rosa_core::error::Result;
    use crate::tool::Tool;

    #[derive(Deserialize, JsonSchema)]
    pub struct EchoArgs {
        #[allow(dead_code)]
        pub message: String,
    }

    pub struct BlacklistEchoTool;

    #[async_trait]
    impl Tool for BlacklistEchoTool {
        fn name(&self) -> &str { "blacklist_echo" }
        fn description(&self) -> &str { "Echo the blacklist." }
        fn schema(&self) -> RootSchema { schema_for!(EchoArgs) }
        fn requires_blacklist(&self) -> bool { true }

        async fn execute(&self, args: Value) -> Result<Value> {
            // Return the injected blacklist so the test can inspect it
            Ok(json!({
                "blacklist": args["_blacklist"],
                "message":   args["message"],
            }))
        }
    }
}

#[tokio::test]
async fn test_blacklist_injected_when_tool_opts_in() {
    use blacklist_test_helpers::BlacklistEchoTool;

    let r = ToolRegistry::new()
        .register(BlacklistEchoTool)
        .with_blacklist(vec!["master".into(), "docker".into()]);

    let result = r
        .dispatch("blacklist_echo", json!({"message": "hi"}))
        .await
        .unwrap();

    assert_eq!(result["blacklist"], json!(["master", "docker"]));
    assert_eq!(result["message"],   json!("hi"));
}

#[tokio::test]
async fn test_blacklist_not_injected_when_empty() {
    use blacklist_test_helpers::BlacklistEchoTool;

    // No blacklist set
    let r = ToolRegistry::new().register(BlacklistEchoTool);

    let result = r
        .dispatch("blacklist_echo", json!({"message": "hi"}))
        .await
        .unwrap();

    // _blacklist key should NOT be present
    assert!(result["blacklist"].is_null(), "_blacklist should be null when no blacklist set");
}
