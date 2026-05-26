//! Isaac Sim integration tools for the ROSA agent.
//!
//! Each tool calls the Isaac Sim embedded HTTP control server
//! (`enable_ros2_bridge.py` starts it on `ISAAC_CONTROL_URL`,
//! default `http://localhost:8282`).
//!
//! ## Available tools
//!
//! | Tool name          | HTTP call                          | Effect                          |
//! |--------------------|------------------------------------|---------------------------------|
//! | `timeline_start`   | `POST /timeline/play`              | Start / resume simulation       |
//! | `timeline_stop`    | `POST /timeline/stop`              | Stop simulation (rewind)        |
//! | `timeline_pause`   | `POST /timeline/pause`             | Pause simulation (keep state)   |
//! | `get_diagnostics`  | `GET  /diagnostics`                | Return sim stats JSON           |
//! | `load_usd`         | `POST /scene/load`  `{"path":"…"}` | Load a USD scene file           |
//! | `list_usds`        | `GET  /scene/list`                 | List available USD scenes       |
//!
//! ## Quick start
//!
//! ```no_run
//! use rosa_isaac::{IsaacClient, tools::all_isaac_tools};
//! use rosa_tools::ToolRegistry;
//!
//! let client = IsaacClient::default();
//! let registry = ToolRegistry::new();
//! let registry = all_isaac_tools(registry, client);
//! ```

pub mod client;
pub mod error;
pub mod tools;

pub use client::IsaacClient;
pub use error::IsaacError;
