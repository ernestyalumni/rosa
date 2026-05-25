//! Tool trait, `ToolRegistry`, and provider-agnostic built-in tools.
//!
//! ## Architecture
//!
//! ```text
//! rosa_core::agent::ToolDispatcher   (trait, defined in rosa-core)
//!         ↑
//!  ToolRegistry  ─── holds ──→  Vec<Box<dyn Tool>>
//!                                       ↑
//!                            AddTool / LogMessageTool / SystemInfoTool
//!                            (builtin) + user-defined tools
//! ```
//!
//! ## Quick start
//!
//! ```no_run
//! use rosa_tools::{ToolRegistry, builtin::{AddTool, LogMessageTool, SystemInfoTool}};
//! use rosa_core::agent::ToolDispatcher;
//!
//! #[tokio::main]
//! async fn main() {
//!     let registry = ToolRegistry::new()
//!         .register(AddTool)
//!         .register(LogMessageTool)
//!         .register(SystemInfoTool);
//!
//!     let result = registry
//!         .dispatch("add", serde_json::json!({"a": 1.0, "b": 2.0}))
//!         .await
//!         .unwrap();
//!     assert_eq!(result, serde_json::json!(3.0));
//! }
//! ```

pub mod builtin;
mod registry;
pub mod tool;

#[cfg(test)]
mod tests;

pub use registry::ToolRegistry;
pub use tool::Tool;
