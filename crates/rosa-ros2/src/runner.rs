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
// Production: shell-out
// ---------------------------------------------------------------------------

/// Spawns `ros2 <args>` via `tokio::process::Command` with a timeout.
pub struct ShellRunner {
    pub timeout_secs: u64,
}

impl ShellRunner {
    pub fn new() -> Self {
        Self { timeout_secs: 5 }
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
        let label = format!("ros2 {}", args.join(" "));

        let fut = tokio::process::Command::new("ros2").args(args).output();

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
