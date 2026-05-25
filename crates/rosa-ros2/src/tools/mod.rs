//! ROS 2 CLI shell-out tools (Phase 5a).
//!
//! All tools wrap `ros2 <subcommand>` via [`crate::runner::ShellRunner`].
//! For unit tests, inject a [`crate::runner::MockRunner`] via `YourTool::with_runner(…)`.
//!
//! ## Quick start
//!
//! ```no_run
//! use rosa_ros2::ros2_registry;
//!
//! // Build a ToolRegistry with all ROS 2 tools and the default blacklist.
//! let registry = ros2_registry(vec!["master".into(), "docker".into()]);
//! ```

pub mod doctor;
pub mod list_nodes;
pub mod list_params;
pub mod list_services;
pub mod list_topics;
pub mod node_info;
pub mod param_get;
pub mod param_set;
pub mod service_call;
pub mod topic_echo;
pub mod topic_info;

pub use doctor::DoctorTool;
pub use list_nodes::ListNodesTool;
pub use list_params::ListParamsTool;
pub use list_services::ListServicesTool;
pub use list_topics::ListTopicsTool;
pub use node_info::NodeInfoTool;
pub use param_get::ParamGetTool;
pub use param_set::ParamSetTool;
pub use service_call::ServiceCallTool;
pub use topic_echo::TopicEchoTool;
pub use topic_info::TopicInfoTool;
