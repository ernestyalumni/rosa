//! Turtle demo — rosa controlling TurtleSim via ROS 2.
//!
//! # Prerequisites
//! 1. Start the ROS 2 container:
//!    ```bash
//!    cd Monoclaw/Deployments/ROS && docker compose up -d
//!    ```
//! 2. In another terminal, start turtlesim (needs X11 forwarding):
//!    ```bash
//!    docker compose exec ros2 bash -ic "ros2 run turtlesim turtlesim_node"
//!    ```
//! 3. Set `ROS_CONTAINER=rosa-ros2` so rosa uses `docker exec` for all ros2 commands.
//!
//! # Run
//! ```bash
//! ROS_CONTAINER=rosa-ros2 ANTHROPIC_API_KEY=... \
//!   cargo run --example turtle -p rosa-cli
//! ```
//! Then try: `Draw a 5-point star using the turtle.`

use std::io::{self, Write};
use std::sync::Arc;

use async_trait::async_trait;
use schemars::{schema_for, schema::RootSchema, JsonSchema};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tokio::time::Duration;

use rosa_core::{
    agent::Agent,
    error::{Result, RosaError},
    events::AgentEvent,
    provider::ChatOptions,
};
use rosa_tools::{Tool, ToolRegistry};
use rosa_ros2::ros2_registry_default;

// ---------------------------------------------------------------------------
// Turtle-specific tools
// ---------------------------------------------------------------------------

/// POSIX single-quote a string so it survives `bash -ic "…"`.
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Run a `ros2` command, optionally via `docker exec ROS_CONTAINER bash -ic`.
///
/// **Why `bash -ic`?** `docker exec container ros2 …` tries to exec the `ros2`
/// binary directly. But `ros2` lives under `/opt/ros/humble/bin` which is only
/// added to PATH when bash sources `.bashrc` (which sources `setup.bash`). Without
/// the shell wrapper ros2 is not found and the command fails with no stderr output.
/// Routing through `bash -ic` mirrors what `ShellRunner` does for all other tools.
async fn ros2_exec(args: &[&str], timeout_secs: u64) -> std::result::Result<String, String> {
    let ros_container = std::env::var("ROS_CONTAINER").ok();

    let owned_cmd: String;
    let (program, full_args): (&str, Vec<&str>) = if let Some(ref c) = ros_container {
        // Shell-quote every arg so YAML bodies with spaces/braces survive bash.
        owned_cmd = format!(
            "ros2 {}",
            args.iter().map(|a| shell_quote(a)).collect::<Vec<_>>().join(" ")
        );
        ("docker", vec!["exec", c.as_str(), "bash", "-ic", &owned_cmd])
    } else {
        owned_cmd = String::new();
        let _ = &owned_cmd;
        ("ros2", args.to_vec())
    };

    let output = tokio::time::timeout(
        Duration::from_secs(timeout_secs),
        tokio::process::Command::new(program)
            .args(&full_args)
            .output(),
    )
    .await
    .map_err(|_| format!("timed out after {timeout_secs}s"))?
    .map_err(|e| format!("spawn failed: {e}"))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_owned())
    }
}

// ── turtle_publish_twist ──────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
struct PublishTwistArgs {
    /// Turtle name, e.g. `turtle1`.
    #[serde(default = "default_turtle")]
    pub name: String,
    /// Forward speed (units/s). Positive = forward.
    pub linear_x: f64,
    /// Rotation speed (rad/s). Positive = counterclockwise.
    pub angular_z: f64,
    /// How long to apply this velocity (seconds).
    pub duration_s: f64,
}

fn default_turtle() -> String { "turtle1".to_owned() }

struct TurtlePublishTwist;

#[async_trait]
impl Tool for TurtlePublishTwist {
    fn name(&self) -> &str { "turtle_publish_twist" }
    fn description(&self) -> &str {
        "Move a turtle by publishing a Twist command for `duration_s` seconds. \
         `linear_x` is forward speed (units/s), `angular_z` is rotation speed (rad/s, \
         positive = counterclockwise). A stop command is sent automatically after."
    }
    fn schema(&self) -> RootSchema { schema_for!(PublishTwistArgs) }

    async fn execute(&self, args: Value) -> Result<Value> {
        let a: PublishTwistArgs = serde_json::from_value(args)?;
        let rate: f64 = 10.0;
        let times  = (a.duration_s * rate).ceil() as u64;
        let topic  = format!("/{}/cmd_vel", a.name);
        let twist  = format!(
            "{{linear: {{x: {:.4}}}, angular: {{z: {:.4}}}}}",
            a.linear_x, a.angular_z
        );
        let stop   = "{linear: {x: 0.0}, angular: {z: 0.0}}";
        let rate_s = format!("{rate}");
        let times_s = times.to_string();

        // Publish velocity for duration
        ros2_exec(
            &["topic", "pub", "--rate", &rate_s, "--times", &times_s,
              &topic, "geometry_msgs/msg/Twist", &twist],
            a.duration_s as u64 + 5,
        )
        .await
        .map_err(|e| RosaError::ToolExecution {
            name: self.name().into(), message: e,
        })?;

        // Explicit stop (belt-and-suspenders)
        let _ = ros2_exec(
            &["topic", "pub", "--times", "1", &topic, "geometry_msgs/msg/Twist", stop],
            5,
        ).await;

        Ok(json!({
            "moved": true,
            "turtle": a.name,
            "linear_x": a.linear_x,
            "angular_z": a.angular_z,
            "duration_s": a.duration_s,
        }))
    }
}

// ── turtle_get_pose ───────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
struct GetPoseArgs {
    /// Turtle name, e.g. `turtle1`.
    #[serde(default = "default_turtle")]
    pub name: String,
}

struct TurtleGetPose;

#[async_trait]
impl Tool for TurtleGetPose {
    fn name(&self) -> &str { "turtle_get_pose" }
    fn description(&self) -> &str {
        "Read the current pose (x, y, theta) of a turtle from its pose topic."
    }
    fn schema(&self) -> RootSchema { schema_for!(GetPoseArgs) }

    async fn execute(&self, args: Value) -> Result<Value> {
        let a: GetPoseArgs = serde_json::from_value(args)?;
        let topic = format!("/{}/pose", a.name);

        let raw = ros2_exec(
            &["topic", "echo", "--once", &topic, "turtlesim/msg/Pose"],
            10,
        )
        .await
        .map_err(|e| RosaError::ToolExecution { name: self.name().into(), message: e })?;

        // Parse YAML lines like "x: 5.544..."
        let (mut x, mut y, mut theta) = (0.0f64, 0.0f64, 0.0f64);
        for line in raw.lines() {
            if let Some(v) = line.trim().strip_prefix("x:") {
                x = v.trim().parse().unwrap_or(0.0);
            } else if let Some(v) = line.trim().strip_prefix("y:") {
                y = v.trim().parse().unwrap_or(0.0);
            } else if let Some(v) = line.trim().strip_prefix("theta:") {
                theta = v.trim().parse().unwrap_or(0.0);
            }
        }
        Ok(json!({ "turtle": a.name, "x": x, "y": y, "theta": theta }))
    }
}

// ── turtle_teleport_absolute ──────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
struct TeleportArgs {
    #[serde(default = "default_turtle")]
    pub name: String,
    pub x:     f64,
    pub y:     f64,
    #[serde(default)]
    pub theta: f64,
}

struct TurtleTeleportAbsolute;

#[async_trait]
impl Tool for TurtleTeleportAbsolute {
    fn name(&self) -> &str { "turtle_teleport_absolute" }
    fn description(&self) -> &str {
        "Teleport a turtle to an absolute position (x, y, theta) without drawing. \
         Coordinates: x ∈ [0, 11.1], y ∈ [0, 11.1]."
    }
    fn schema(&self) -> RootSchema { schema_for!(TeleportArgs) }

    async fn execute(&self, args: Value) -> Result<Value> {
        let a: TeleportArgs = serde_json::from_value(args)?;
        let svc    = format!("/{}/teleport_absolute", a.name);
        let params = format!("{{x: {:.4}, y: {:.4}, theta: {:.4}}}", a.x, a.y, a.theta);

        ros2_exec(
            &["service", "call", &svc, "turtlesim/srv/TeleportAbsolute", &params],
            5,
        )
        .await
        .map_err(|e| RosaError::ToolExecution { name: self.name().into(), message: e })?;

        Ok(json!({ "teleported": true, "x": a.x, "y": a.y, "theta": a.theta }))
    }
}

// ── turtle_set_pen ────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
struct SetPenArgs {
    #[serde(default = "default_turtle")]
    pub name:  String,
    pub r:     u8,
    pub g:     u8,
    pub b:     u8,
    #[serde(default = "pen_default_width")]
    pub width: u8,
    /// 0 = pen down (draws), 1 = pen up (no drawing).
    #[serde(default)]
    pub off:   u8,
}
fn pen_default_width() -> u8 { 2 }

struct TurtleSetPen;

#[async_trait]
impl Tool for TurtleSetPen {
    fn name(&self) -> &str { "turtle_set_pen" }
    fn description(&self) -> &str {
        "Set the turtle's pen color (r/g/b 0-255), width (pixels), and on/off state."
    }
    fn schema(&self) -> RootSchema { schema_for!(SetPenArgs) }

    async fn execute(&self, args: Value) -> Result<Value> {
        let a: SetPenArgs = serde_json::from_value(args)?;
        let svc    = format!("/{}/set_pen", a.name);
        // "off" must be double-quoted: bare `off` is a YAML 1.1 boolean (= False),
        // causing PyYAML inside the container to parse {off: 0} as {False: 0} →
        // ros2 crashes with "attribute name must be string".
        let params = format!(
            "{{r: {}, g: {}, b: {}, width: {}, \"off\": {}}}",
            a.r, a.g, a.b, a.width, a.off
        );
        ros2_exec(&["service", "call", &svc, "turtlesim/srv/SetPen", &params], 5)
            .await
            .map_err(|e| RosaError::ToolExecution { name: self.name().into(), message: e })?;
        Ok(json!({ "pen_set": true }))
    }
}

// ── turtle_clear ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
struct ClearArgs {}

struct TurtleClear;

#[async_trait]
impl Tool for TurtleClear {
    fn name(&self) -> &str { "turtle_clear" }
    fn description(&self) -> &str {
        "Clear all drawn lines from the TurtleSim canvas (calls /clear service)."
    }
    fn schema(&self) -> RootSchema { schema_for!(ClearArgs) }

    async fn execute(&self, args: Value) -> Result<Value> {
        let _: ClearArgs = serde_json::from_value(args)?;
        ros2_exec(&["service", "call", "/clear", "std_srvs/srv/Empty", "{}"], 5)
            .await
            .map_err(|e| RosaError::ToolExecution { name: self.name().into(), message: e })?;
        Ok(json!({ "cleared": true }))
    }
}

// ── turtle_within_bounds ──────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
struct WithinBoundsArgs {
    pub x: f64,
    pub y: f64,
}

struct TurtleWithinBounds;

#[async_trait]
impl Tool for TurtleWithinBounds {
    fn name(&self) -> &str { "turtle_within_bounds" }
    fn description(&self) -> &str {
        "Check if (x, y) is within TurtleSim's 11.1 × 11.1 canvas. Pure computation, no ROS call."
    }
    fn schema(&self) -> RootSchema { schema_for!(WithinBoundsArgs) }

    async fn execute(&self, args: Value) -> Result<Value> {
        let a: WithinBoundsArgs = serde_json::from_value(args)?;
        const MAX: f64 = 11.1;
        let within = a.x >= 0.0 && a.x <= MAX && a.y >= 0.0 && a.y <= MAX;
        Ok(json!({ "within_bounds": within, "x": a.x, "y": a.y, "canvas_size": MAX }))
    }
}

// ── turtle_reset ──────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
struct ResetArgs {}

struct TurtleReset;

#[async_trait]
impl Tool for TurtleReset {
    fn name(&self) -> &str { "turtle_reset" }
    fn description(&self) -> &str {
        "Reset TurtleSim: removes all extra turtles, clears drawings, and returns \
         turtle1 to the centre. Calls the /reset service."
    }
    fn schema(&self) -> RootSchema { schema_for!(ResetArgs) }

    async fn execute(&self, args: Value) -> Result<Value> {
        let _: ResetArgs = serde_json::from_value(args)?;
        ros2_exec(&["service", "call", "/reset", "std_srvs/srv/Empty", "{}"], 5)
            .await
            .map_err(|e| RosaError::ToolExecution { name: self.name().into(), message: e })?;
        Ok(json!({ "reset": true }))
    }
}

// ── turtle_stop ───────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
struct StopArgs {
    #[serde(default = "default_turtle")]
    pub name: String,
}

struct TurtleStop;

#[async_trait]
impl Tool for TurtleStop {
    fn name(&self) -> &str { "turtle_stop" }
    fn description(&self) -> &str {
        "Send a zero-velocity command to immediately stop a turtle's motion."
    }
    fn schema(&self) -> RootSchema { schema_for!(StopArgs) }

    async fn execute(&self, args: Value) -> Result<Value> {
        let a: StopArgs = serde_json::from_value(args)?;
        let topic = format!("/{}/cmd_vel", a.name);
        let stop  = "{linear: {x: 0.0}, angular: {z: 0.0}}";
        ros2_exec(
            &["topic", "pub", "--times", "1", &topic, "geometry_msgs/msg/Twist", stop],
            5,
        )
        .await
        .map_err(|e| RosaError::ToolExecution { name: self.name().into(), message: e })?;
        Ok(json!({ "stopped": true, "turtle": a.name }))
    }
}

// ── turtle_spawn ──────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
struct SpawnArgs {
    pub x:     f64,
    pub y:     f64,
    #[serde(default)]
    pub theta: f64,
    /// Name for the new turtle (e.g. `turtle2`). Omit to let TurtleSim choose.
    #[serde(default)]
    pub name:  String,
}

struct TurtleSpawn;

#[async_trait]
impl Tool for TurtleSpawn {
    fn name(&self) -> &str { "turtle_spawn" }
    fn description(&self) -> &str {
        "Spawn a new turtle at (x, y, theta). Optionally give it a name. \
         Returns the name assigned by TurtleSim."
    }
    fn schema(&self) -> RootSchema { schema_for!(SpawnArgs) }

    async fn execute(&self, args: Value) -> Result<Value> {
        let a: SpawnArgs = serde_json::from_value(args)?;
        let params = if a.name.is_empty() {
            format!("{{x: {:.4}, y: {:.4}, theta: {:.4}}}", a.x, a.y, a.theta)
        } else {
            // Double-quote the name so it's unambiguously a YAML string and
            // shell_quote doesn't need to escape inner single quotes.
            format!("{{x: {:.4}, y: {:.4}, theta: {:.4}, name: \"{}\"}}", a.x, a.y, a.theta, a.name)
        };
        let raw = ros2_exec(
            &["service", "call", "/spawn", "turtlesim/srv/Spawn", &params],
            10,
        )
        .await
        .map_err(|e| RosaError::ToolExecution { name: self.name().into(), message: e })?;
        Ok(json!({ "spawned": true, "response": raw }))
    }
}

// ── turtle_kill ───────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
struct KillArgs {
    /// Name of the turtle to kill (e.g. `turtle2`).
    pub name: String,
}

struct TurtleKill;

#[async_trait]
impl Tool for TurtleKill {
    fn name(&self) -> &str { "turtle_kill" }
    fn description(&self) -> &str {
        "Remove a turtle from TurtleSim by name. Cannot kill the last remaining turtle."
    }
    fn schema(&self) -> RootSchema { schema_for!(KillArgs) }

    async fn execute(&self, args: Value) -> Result<Value> {
        let a: KillArgs = serde_json::from_value(args)?;
        let params = format!("{{name: \"{}\"}}", a.name);
        ros2_exec(
            &["service", "call", "/kill", "turtlesim/srv/Kill", &params],
            5,
        )
        .await
        .map_err(|e| RosaError::ToolExecution { name: self.name().into(), message: e })?;
        Ok(json!({ "killed": a.name }))
    }
}

// ── turtle_teleport_relative ──────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
struct TeleportRelativeArgs {
    #[serde(default = "default_turtle")]
    pub name:   String,
    /// Linear distance to move forward (turtle's local +x axis).
    pub linear: f64,
    /// Angle to rotate (radians, positive = counterclockwise).
    pub angular: f64,
}

struct TurtleTeleportRelative;

#[async_trait]
impl Tool for TurtleTeleportRelative {
    fn name(&self) -> &str { "turtle_teleport_relative" }
    fn description(&self) -> &str {
        "Teleport a turtle by a relative offset: rotate by `angular` radians then \
         move `linear` units forward in the turtle's local frame. No drawing occurs."
    }
    fn schema(&self) -> RootSchema { schema_for!(TeleportRelativeArgs) }

    async fn execute(&self, args: Value) -> Result<Value> {
        let a: TeleportRelativeArgs = serde_json::from_value(args)?;
        let svc    = format!("/{}/teleport_relative", a.name);
        let params = format!("{{linear: {:.4}, angular: {:.4}}}", a.linear, a.angular);
        ros2_exec(
            &["service", "call", &svc, "turtlesim/srv/TeleportRelative", &params],
            5,
        )
        .await
        .map_err(|e| RosaError::ToolExecution { name: self.name().into(), message: e })?;
        Ok(json!({ "teleported_relative": true, "linear": a.linear, "angular": a.angular }))
    }
}

// ── turtle_draw_line_to ───────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
struct DrawLineToArgs {
    #[serde(default = "default_turtle")]
    pub name: String,
    /// Target x coordinate in TurtleSim canvas space.
    pub target_x: f64,
    /// Target y coordinate in TurtleSim canvas space.
    pub target_y: f64,
    /// Turtle speed (units/s, default 2.0).
    #[serde(default = "default_speed")]
    pub speed: f64,
}
fn default_speed() -> f64 { 2.0 }

struct TurtleDrawLineTo;

#[async_trait]
impl Tool for TurtleDrawLineTo {
    fn name(&self) -> &str { "turtle_draw_line_to" }
    fn description(&self) -> &str {
        "Draw a straight line from the turtle's current pose to (target_x, target_y). \
         Reads the current pose, rotates to face the target, then drives forward. \
         The pen must be down before calling this."
    }
    fn schema(&self) -> RootSchema { schema_for!(DrawLineToArgs) }

    async fn execute(&self, args: Value) -> Result<Value> {
        let a: DrawLineToArgs = serde_json::from_value(args)?;
        if a.speed <= 0.0 {
            return Err(RosaError::ToolExecution {
                name: self.name().into(),
                message: "speed must be > 0".into(),
            });
        }

        // 1. Read current pose
        let topic = format!("/{}/pose", a.name);
        let raw = ros2_exec(
            &["topic", "echo", "--once", &topic, "turtlesim/msg/Pose"],
            10,
        )
        .await
        .map_err(|e| RosaError::ToolExecution { name: self.name().into(), message: e })?;

        let (mut cx, mut cy, mut ctheta) = (0.0f64, 0.0f64, 0.0f64);
        for line in raw.lines() {
            if let Some(v) = line.trim().strip_prefix("x:") { cx = v.trim().parse().unwrap_or(0.0); }
            else if let Some(v) = line.trim().strip_prefix("y:") { cy = v.trim().parse().unwrap_or(0.0); }
            else if let Some(v) = line.trim().strip_prefix("theta:") { ctheta = v.trim().parse().unwrap_or(0.0); }
        }

        // 2. Compute heading and distance
        let dx = a.target_x - cx;
        let dy = a.target_y - cy;
        let distance = (dx * dx + dy * dy).sqrt();
        if distance < 0.01 {
            return Ok(json!({ "drawn": true, "distance": 0.0, "note": "already at target" }));
        }
        let target_theta = dy.atan2(dx);
        // Minimal angle difference (wrap to [-π, π])
        let mut dtheta = target_theta - ctheta;
        while dtheta > std::f64::consts::PI  { dtheta -= 2.0 * std::f64::consts::PI; }
        while dtheta < -std::f64::consts::PI { dtheta += 2.0 * std::f64::consts::PI; }

        // 3. Rotate to face target (angular_z = 1.5 rad/s)
        let rot_speed = 1.5_f64;
        let rot_dur = dtheta.abs() / rot_speed;
        if rot_dur > 0.05 {
            let rot_sign = if dtheta >= 0.0 { 1.0 } else { -1.0 };
            let cmd_vel_topic = format!("/{}/cmd_vel", a.name);
            let times = (rot_dur * 10.0).ceil() as u64;
            let twist = format!("{{linear: {{x: 0.0}}, angular: {{z: {:.4}}}}}", rot_sign * rot_speed);
            ros2_exec(
                &["topic", "pub", "--rate", "10", "--times", &times.to_string(),
                  &cmd_vel_topic, "geometry_msgs/msg/Twist", &twist],
                (rot_dur as u64) + 5,
            )
            .await
            .map_err(|e| RosaError::ToolExecution { name: self.name().into(), message: e })?;
        }

        // 4. Drive forward for distance / speed seconds
        let drive_dur = distance / a.speed;
        let cmd_vel_topic = format!("/{}/cmd_vel", a.name);
        let times = (drive_dur * 10.0).ceil() as u64;
        let twist = format!("{{linear: {{x: {:.4}}}, angular: {{z: 0.0}}}}", a.speed);
        ros2_exec(
            &["topic", "pub", "--rate", "10", "--times", &times.to_string(),
              &cmd_vel_topic, "geometry_msgs/msg/Twist", &twist],
            (drive_dur as u64) + 10,
        )
        .await
        .map_err(|e| RosaError::ToolExecution { name: self.name().into(), message: e })?;

        Ok(json!({
            "drawn": true,
            "from": { "x": cx, "y": cy },
            "to":   { "x": a.target_x, "y": a.target_y },
            "distance": distance,
        }))
    }
}

// ── turtle_draw_circle ────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
struct DrawCircleArgs {
    #[serde(default = "default_turtle")]
    pub name: String,
    /// Radius of the circle (canvas units, max 5.0).
    pub radius: f64,
    /// Speed (units/s, default 2.0).
    #[serde(default = "default_speed")]
    pub speed: f64,
}

struct TurtleDrawCircle;

#[async_trait]
impl Tool for TurtleDrawCircle {
    fn name(&self) -> &str { "turtle_draw_circle" }
    fn description(&self) -> &str {
        "Draw a circle of the given radius at the turtle's current position. \
         Uses a continuous Twist with matching linear and angular velocities. \
         The pen must be down before calling."
    }
    fn schema(&self) -> RootSchema { schema_for!(DrawCircleArgs) }

    async fn execute(&self, args: Value) -> Result<Value> {
        let a: DrawCircleArgs = serde_json::from_value(args)?;
        if a.radius <= 0.0 || a.radius > 5.5 {
            return Err(RosaError::ToolExecution {
                name: self.name().into(),
                message: "radius must be in (0, 5.5]".into(),
            });
        }
        // circumference / speed = time for one full circle
        let circumference = 2.0 * std::f64::consts::PI * a.radius;
        let duration = circumference / a.speed;
        let angular_z = a.speed / a.radius; // ω = v / r

        let topic  = format!("/{}/cmd_vel", a.name);
        let times  = (duration * 10.0).ceil() as u64;
        let twist  = format!(
            "{{linear: {{x: {:.4}}}, angular: {{z: {:.4}}}}}",
            a.speed, angular_z
        );
        ros2_exec(
            &["topic", "pub", "--rate", "10", "--times", &times.to_string(),
              &topic, "geometry_msgs/msg/Twist", &twist],
            (duration as u64) + 10,
        )
        .await
        .map_err(|e| RosaError::ToolExecution { name: self.name().into(), message: e })?;

        Ok(json!({
            "drawn": true,
            "radius": a.radius,
            "circumference": circumference,
            "duration_s": duration,
        }))
    }
}

// ---------------------------------------------------------------------------
// Build the full turtle registry
// ---------------------------------------------------------------------------

fn turtle_registry() -> ToolRegistry {
    ros2_registry_default()
        .register(TurtlePublishTwist)
        .register(TurtleGetPose)
        .register(TurtleTeleportAbsolute)
        .register(TurtleSetPen)
        .register(TurtleClear)
        .register(TurtleWithinBounds)
        .register(TurtleReset)
        .register(TurtleStop)
        .register(TurtleSpawn)
        .register(TurtleKill)
        .register(TurtleTeleportRelative)
        .register(TurtleDrawLineTo)
        .register(TurtleDrawCircle)
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    let _ = dotenvy::dotenv();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_writer(io::stderr)
        .init();

    // Provider detection (same as main binary)
    let (provider, model) = detect_provider().unwrap_or_else(|| {
        eprintln!("Set ANTHROPIC_API_KEY, XAI_API_KEY, or OPENAI_API_KEY (or copy .env.example → .env).");
        std::process::exit(1);
    });

    let registry = Arc::new(turtle_registry());

    let agent = Agent::builder()
        .llm(provider)
        .tools(registry)
        .system_prompt(TURTLE_SYSTEM_PROMPT)
        .max_iterations(30)
        .opts(ChatOptions { model: model.clone(), max_tokens: Some(4096), temperature: None })
        .build()
        .expect("failed to build agent");

    // Print banner
    println!("┌─────────────────────────────────────────────────────┐");
    println!("│  rosa turtle demo  •  model: {model:<23}│");
    println!("│  /clear  /quit  •  Ctrl-C cancels a turn            │");
    println!("└─────────────────────────────────────────────────────┘");
    if let Ok(c) = std::env::var("ROS_CONTAINER") {
        println!("  ros2 → docker exec {c}");
    }
    println!();
    println!("  Try: 'Draw a 5-point star.'");
    println!("       'Spawn a second turtle at (3, 8) and draw a circle of radius 1.5.'");
    println!("       'Draw a triangle with side length 3.'");
    println!("       'Reset the sim and draw a red square.'");
    println!();

    // REPL
    loop {
        print!("\x1b[32m> \x1b[0m");
        io::stdout().flush().ok();

        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(0) => { println!("\nBye!"); break; }
            Ok(_) => {}
            Err(e) => { eprintln!("read error: {e}"); break; }
        }

        let query = input.trim();
        if query.is_empty() { continue; }
        match query {
            "/quit" | "/exit" => { println!("Bye!"); break; }
            "/clear" => {
                agent.clear_history().await;
                println!("  conversation history cleared.");
                continue;
            }
            _ => {}
        }

        run_turn(&agent, query).await;
        println!();
    }
}

// ---------------------------------------------------------------------------
// Shared helpers (duplicated from main.rs to keep example self-contained)
// ---------------------------------------------------------------------------

fn detect_provider() -> Option<(Arc<dyn rosa_core::provider::LlmProvider>, String)> {
    if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
        let model = std::env::var("ROSA_MODEL")
            .unwrap_or_else(|_| "claude-sonnet-4-6".to_owned());
        return Some((Arc::new(rosa_llm::AnthropicProvider::new(key)), model));
    }
    if let Ok(key) = std::env::var("XAI_API_KEY") {
        let model = std::env::var("ROSA_MODEL")
            .unwrap_or_else(|_| "grok-4.3".to_owned());
        return Some((Arc::new(rosa_llm::OpenAiProvider::xai(key)), model));
    }
    if let Ok(key) = std::env::var("OPENAI_API_KEY") {
        let model = std::env::var("ROSA_MODEL")
            .unwrap_or_else(|_| "gpt-5.5".to_owned());
        return Some((Arc::new(rosa_llm::OpenAiProvider::new(key)), model));
    }
    None
}

async fn run_turn(agent: &Agent, query: &str) {
    let (tx, mut rx) = mpsc::unbounded_channel::<AgentEvent>();
    let render = async {
        while let Some(event) = rx.recv().await {
            match event {
                AgentEvent::Token { content } => {
                    print!("{content}");
                    io::stdout().flush().ok();
                }
                AgentEvent::ToolStart { name, input } => {
                    println!("\n\x1b[2m  ↗ {name}({input})\x1b[0m");
                }
                AgentEvent::ToolEnd { name, output } => {
                    let s = output.to_string();
                    let d = if s.len() > 120 { format!("{}…", &s[..120]) } else { s };
                    println!("\x1b[2m  ↙ {name} → {d}\x1b[0m");
                }
                AgentEvent::Final { .. } => { println!(); }
                AgentEvent::Usage { prompt_tokens, completion_tokens } => {
                    eprintln!(
                        "\x1b[2m  [tokens] prompt={prompt_tokens} completion={completion_tokens} \
                         total={}\x1b[0m",
                        prompt_tokens + completion_tokens
                    );
                }
                AgentEvent::Error { message } => { eprintln!("\n\x1b[31m[error] {message}\x1b[0m"); }
            }
        }
    };
    tokio::select! {
        _ = tokio::signal::ctrl_c() => { println!("\n\x1b[33m[cancelled]\x1b[0m"); }
        (result, ()) = async { tokio::join!(agent.stream(query, tx), render) } => {
            if let Err(e) = result { eprintln!("\x1b[31m[agent error] {e}\x1b[0m"); }
        }
    }
}

// ---------------------------------------------------------------------------
// Turtle system prompt
// ---------------------------------------------------------------------------

const TURTLE_SYSTEM_PROMPT: &str = "\
You are rosa, a robot operator controlling a TurtleSim in ROS 2.

## TurtleSim Coordinate System
- Origin (0,0) is at the **bottom-left**
- Default spawn: approximately (5.54, 5.54), facing right (theta = 0)
- Canvas: x ∈ [0.0, 11.1], y ∈ [0.0, 11.1]
- theta = 0 → facing right (+x); theta = π/2 → facing up (+y)
- `linear_x > 0` → move forward; `angular_z > 0` → turn counterclockwise

## Available Tools
| Tool                       | Purpose                                           |
|----------------------------|---------------------------------------------------|
| `turtle_publish_twist`     | Move/rotate for N seconds                        |
| `turtle_get_pose`          | Read current (x, y, theta)                       |
| `turtle_teleport_absolute` | Jump to (x, y, theta) without drawing            |
| `turtle_teleport_relative` | Relative rotate + forward jump without drawing   |
| `turtle_set_pen`           | Set pen colour (r,g,b), width, on/off            |
| `turtle_clear`             | Clear drawings (keeps turtles)                   |
| `turtle_reset`             | Full reset: remove extra turtles, clear canvas   |
| `turtle_stop`              | Send zero-velocity to stop immediately           |
| `turtle_spawn`             | Spawn a new turtle at (x, y, theta)              |
| `turtle_kill`              | Remove a turtle by name                          |
| `turtle_draw_line_to`      | Draw line from current pose to (target_x, target_y)|
| `turtle_draw_circle`       | Draw circle of given radius at current position  |
| `turtle_within_bounds`     | Check if (x, y) is inside the canvas            |

Plus all ROS 2 inspection tools: `ros2_list_nodes`, `ros2_list_topics`, `ros2_service_call`, etc.

## Drawing Strategy

**Straight lines:** use `turtle_draw_line_to` — it auto-rotates and drives.

**Circles:** use `turtle_draw_circle` — it computes angular velocity from v/r.

**Polygons (star, square, triangle):**
1. `turtle_teleport_absolute` to start corner (pen up first with `turtle_set_pen off=1`)
2. `turtle_set_pen off=0` to put pen down, choose colour
3. Loop N sides: `turtle_publish_twist` (forward) → `turtle_publish_twist` (turn)
   - Turn angle for N-gon: 360°/N exterior angle
   - 5-point star turn: 144° = 2.5133 rad
4. `turtle_stop` to ensure motion has ceased

**Multi-turtle:** spawn with `turtle_spawn`, address each by `name`.

## Important Rules
- Never compute trig yourself — use `atan2`, `sin`, `cos`, `degrees_to_radians`, etc.
- Always verify bounds with `turtle_within_bounds` before large moves.
- After `turtle_publish_twist`, the tool auto-sends a stop — no need to call `turtle_stop` again
  unless you need an explicit halt mid-sequence.
- For precise shapes: time = distance / speed or time = angle / angular_rate.
";
