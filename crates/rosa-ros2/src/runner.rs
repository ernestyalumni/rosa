//! Async ROS 2 command executor.
//!
//! The `Ros2Runner` trait decouples tools from the real `ros2` binary, making
//! unit tests possible without a live ROS 2 install. Inject `MockRunner` in
//! tests via `YourTool::with_runner(runner, blacklist)`.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::time::timeout;

use rosa_core::error::{Result, RosaError};

// ---------------------------------------------------------------------------
// Trait
// ---------------------------------------------------------------------------

/// Execute `ros2 <args>` and return stdout.
///
/// Implementors: [`ShellRunner`] (production) and [`MockRunner`] (tests).
#[async_trait]
pub trait Ros2Runner: Send + Sync {
    async fn run(&self, args: &[&str]) -> Result<String>;
}

/// Shared reference alias.
pub type SharedRunner = Arc<dyn Ros2Runner>;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// POSIX single-quote a string so it is safe to embed inside `bash -ic "…"`.
///
/// Single-quoted strings cannot contain a literal `'`, so we end the quote,
/// insert an escaped `'`, then restart the quote — the classic `'...'\\''...'`
/// trick.
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

// ---------------------------------------------------------------------------
// Production: shell-out
// ---------------------------------------------------------------------------

/// Spawns `ros2 <args>` via `tokio::process::Command` with a timeout.
///
/// If the `ROS_CONTAINER` environment variable is set, prefixes every command
/// with `docker exec <container>`, allowing rosa to run on the host while the
/// ROS 2 tools execute inside the container:
///
/// ```bash
/// ROS_CONTAINER=rosa-ros2 cargo run --example turtle -p rosa-cli
/// ```
pub struct ShellRunner {
    pub timeout_secs: u64,
    /// If Some, use `docker exec <container> ros2 …` instead of `ros2 …`.
    pub ros_container: Option<String>,
}

impl ShellRunner {
    pub fn new() -> Self {
        Self {
            timeout_secs: 5,
            ros_container: std::env::var("ROS_CONTAINER").ok(),
        }
    }

    pub fn shared() -> SharedRunner {
        Arc::new(Self::new())
    }
}

impl Default for ShellRunner {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Ros2Runner for ShellRunner {
    async fn run(&self, args: &[&str]) -> Result<String> {
        // Build the command.
        //
        // Direct mode: `ros2 <args>`
        //
        // Docker exec mode: `docker exec <container> bash -ic "ros2 <args>"`
        //   We must go through `bash -ic` so the container's
        //   `/opt/ros/humble/setup.bash` is sourced (baked into .bashrc by the
        //   Dockerfile). Without it, `ros2` is not in PATH.
        let owned_ros2_cmd: String; // kept alive across the if-else
        let (program, full_args): (&str, Vec<&str>) =
            if let Some(ref container) = self.ros_container {
                // Shell-quote each arg so YAML bodies with spaces/quotes survive
                // being passed through `bash -ic "ros2 <args>"`.
                owned_ros2_cmd = format!(
                    "ros2 {}",
                    args.iter().map(|a| shell_quote(a)).collect::<Vec<_>>().join(" ")
                );
                ("docker", vec!["exec", container.as_str(), "bash", "-ic", &owned_ros2_cmd])
            } else {
                owned_ros2_cmd = String::new(); // unused
                ("ros2", args.to_vec())
            };

        let label = if self.ros_container.is_some() {
            format!("docker exec {} {}", self.ros_container.as_deref().unwrap_or(""), &owned_ros2_cmd)
        } else {
            format!("ros2 {}", args.join(" "))
        };

        let fut = tokio::process::Command::new(program)
            .args(&full_args)
            .output();

        let output = timeout(Duration::from_secs(self.timeout_secs), fut)
            .await
            .map_err(|_| RosaError::ToolExecution {
                name: "ros2".into(),
                message: format!("timed out after {}s: {label}", self.timeout_secs),
            })?
            .map_err(|e| RosaError::ToolExecution {
                name: "ros2".into(),
                message: format!("failed to spawn `{label}`: {e}"),
            })?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).into_owned())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(RosaError::ToolExecution {
                name: "ros2".into(),
                message: format!("`{label}` failed:\n{stderr}"),
            })
        }
    }
}

// ---------------------------------------------------------------------------
// Test: mock
// ---------------------------------------------------------------------------

/// Unit-test mock that returns a fixed string for any command.
///
/// Use with `YourTool::with_runner(MockRunner::new("…"), blacklist)`.
#[cfg(test)]
pub struct MockRunner {
    pub output: String,
}

#[cfg(test)]
impl MockRunner {
    pub fn new(output: impl Into<String>) -> SharedRunner {
        Arc::new(Self {
            output: output.into(),
        })
    }
}

#[cfg(test)]
#[async_trait]
impl Ros2Runner for MockRunner {
    async fn run(&self, _args: &[&str]) -> Result<String> {
        Ok(self.output.clone())
    }
}
