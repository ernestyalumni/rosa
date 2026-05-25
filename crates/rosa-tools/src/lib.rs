//! Tool trait, ToolRegistry, and built-in tools.
//!
//! Task 04 implements this fully. This stub compiles immediately.

// TODO(task-04): implement Tool trait, ToolRegistry, builtin tools

use rosa_core::{RosaError, ToolSpec};
use rosa_core::agent::ToolDispatcher;
use async_trait::async_trait;

/// Placeholder registry; task 04 replaces this with the full implementation.
pub struct ToolRegistry;

#[async_trait]
impl ToolDispatcher for ToolRegistry {
    fn tool_specs(&self) -> Vec<ToolSpec> {
        vec![]
    }

    async fn dispatch(
        &self,
        name: &str,
        _args: serde_json::Value,
    ) -> rosa_core::Result<serde_json::Value> {
        Err(RosaError::ToolNotFound { name: name.to_owned() })
    }
}
