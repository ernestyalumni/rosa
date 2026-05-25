//! Provider-agnostic built-in tools bundled with rosa-tools.
//!
//! | Module        | Tool struct       | name            | Purpose                       |
//! |---------------|-------------------|-----------------|-------------------------------|
//! | `calculation` | [`AddTool`]       | `add`           | Add two floating-point numbers|
//! | `log`         | [`LogMessageTool`]| `log_message`   | Emit a structured log event   |
//! | `system`      | [`SystemInfoTool`]| `system_info`   | Query hostname / uname / date |

pub mod calculation;
pub mod log;
pub mod system;

pub use calculation::AddTool;
pub use log::LogMessageTool;
pub use system::SystemInfoTool;
