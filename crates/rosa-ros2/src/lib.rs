//! ROS 2 bridge for rosa — Phase 5a: CLI shell-out tools.
//!
//! ## Tools registered by [`ros2_registry`]
//!
//! | Tool name             | Wraps                          |
//! |-----------------------|--------------------------------|
//! | `ros2_list_nodes`     | `ros2 node list`               |
//! | `ros2_list_topics`    | `ros2 topic list -t`           |
//! | `ros2_list_services`  | `ros2 service list -t`         |
//! | `ros2_list_params`    | `ros2 param list [node]`       |
//! | `ros2_topic_echo`     | `ros2 topic echo --once`       |
//! | `ros2_topic_info`     | `ros2 topic info -v`           |
//! | `ros2_node_info`      | `ros2 node info`               |
//! | `ros2_param_get`      | `ros2 param get`               |
//! | `ros2_doctor`         | `ros2 doctor --report`         |
//!
//! Each tool has a 5-second timeout and applies a blacklist filter to stdout.
//!
//! ## Phase 5b (future — `ros2-native` feature)
//!
//! Native r2r bindings for in-process publish/subscribe are gated behind the
//! `ros2-native` Cargo feature. Enable it only when building inside the ROS 2
//! Docker container (requires a sourced Humble install at build time).

pub mod filter;
pub mod runner;
pub mod tools;

#[cfg(test)]
mod tests;

use rosa_tools::ToolRegistry;

pub use tools::{
    DoctorTool, ListNodesTool, ListParamsTool, ListServicesTool,
    ListTopicsTool, NodeInfoTool, ParamGetTool, TopicEchoTool, TopicInfoTool,
};

/// Default blacklist — matches upstream rosa's `["master", "docker"]` convention.
///
/// Nodes, topics, or services whose names contain any of these strings are
/// stripped from CLI output before returning to the agent.
pub const DEFAULT_BLACKLIST: &[&str] = &["master", "docker"];

/// Build a [`ToolRegistry`] pre-loaded with all ROS 2 CLI tools.
///
/// ```no_run
/// use rosa_ros2::ros2_registry;
/// let registry = ros2_registry(vec!["master".into(), "docker".into()]);
/// ```
pub fn ros2_registry(blacklist: Vec<String>) -> ToolRegistry {
    ToolRegistry::new()
        .register(ListNodesTool::new(blacklist.clone()))
        .register(ListTopicsTool::new(blacklist.clone()))
        .register(ListServicesTool::new(blacklist.clone()))
        .register(ListParamsTool::new(blacklist.clone()))
        .register(TopicEchoTool::new(blacklist.clone()))
        .register(TopicInfoTool::new(blacklist.clone()))
        .register(NodeInfoTool::new(blacklist.clone()))
        .register(ParamGetTool::new(blacklist.clone()))
        .register(DoctorTool::new(blacklist))
}

/// Convenience: build a registry with the default blacklist.
pub fn ros2_registry_default() -> ToolRegistry {
    let bl: Vec<String> = DEFAULT_BLACKLIST.iter().map(|s| s.to_string()).collect();
    ros2_registry(bl)
}
