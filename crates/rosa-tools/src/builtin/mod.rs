//! Provider-agnostic built-in tools bundled with rosa-tools.
//!
//! | Module        | Tool struct           | name                  | Purpose                        |
//! |---------------|-----------------------|-----------------------|--------------------------------|
//! | `calculation` | [`AddTool`]           | `add`                 | Add two floating-point numbers |
//! | `log`         | [`LogMessageTool`]    | `log_message`         | Emit a structured log event    |
//! | `system`      | [`SystemInfoTool`]    | `system_info`         | Query hostname / uname / date  |
//! | `math`        | [`DegreesToRadians`]  | `degrees_to_radians`  | Convert degrees → radians      |
//! | `math`        | [`RadiansToDegrees`]  | `radians_to_degrees`  | Convert radians → degrees      |
//! | `math`        | [`Sqrt`]              | `sqrt`                | Square root                    |
//! | `math`        | [`Atan2`]             | `atan2`               | Four-quadrant arctangent       |
//! | `math`        | [`Distance2D`]        | `distance_2d`         | 2-D Euclidean distance         |
//! | `math`        | [`HeadingAndDistance`]| `heading_and_distance`| Bearing + distance p1 → p2     |
//! | `math`        | [`Subtract`]          | `subtract`            | a − b                          |
//! | `math`        | [`Multiply`]          | `multiply`            | a × b                          |
//! | `math`        | [`Divide`]            | `divide`              | a ÷ b                          |
//! | `math`        | [`Sin`]               | `sin`                 | sin(radians)                   |
//! | `math`        | [`Cos`]               | `cos`                 | cos(radians)                   |
//! | `math`        | [`Tan`]               | `tan`                 | tan(radians)                   |
//! | `wait`        | [`WaitTool`]          | `wait`                | Pause N seconds (max 60)       |

pub mod calculation;
pub mod log;
pub mod math;
pub mod system;
pub mod wait;

pub use calculation::AddTool;
pub use log::LogMessageTool;
pub use math::{
    Atan2, Cos, DegreesToRadians, Distance2D, Divide,
    HeadingAndDistance, Multiply, RadiansToDegrees, Sin, Sqrt, Subtract, Tan,
};
pub use system::SystemInfoTool;
pub use wait::WaitTool;
