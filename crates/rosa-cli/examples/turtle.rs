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

/// Run a `ros2` command, optionally via `docker exec ROS_CONTAINER`.
async fn ros2_exec(args: &[&str], timeout_secs: u64) -> std::result::Result<String, String> {
    let ros_container = std::env::var("ROS_CONTAINER").ok();

    let (program, full_args): (&str, Vec<&str>) = if let Some(ref c) = ros_container {
        let mut v: Vec<&str> = vec!["exec", c.as_str(), "ros2"];
        v.extend_from_slice(args);
        ("docker", v)
    } else {
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
        let params = format!(
            "{{r: {}, g: {}, b: {}, width: {}, off: {}}}",
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
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_writer(io::stderr)
        .init();

    // Provider detection (same as main binary)
    let (provider, model) = detect_provider().unwrap_or_else(|| {
        eprintln!("Set ANTHROPIC_API_KEY, OPENAI_API_KEY, or OLLAMA_MODEL to run the turtle demo.");
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
    println!("│  type /quit to exit  •  Ctrl-C cancels a turn       │");
    println!("└─────────────────────────────────────────────────────┘");
    if let Ok(c) = std::env::var("ROS_CONTAINER") {
        println!("  ros2 → docker exec {c}");
    }
    println!();
    println!("  Try: 'Draw a 5-point star using the turtle.'");
    println!("       'Move turtle1 forward 3 units and back.'");
    println!("       'What topics are available?'");
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
        if matches!(query, "/quit" | "/exit") { println!("Bye!"); break; }

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
            .unwrap_or_else(|_| "grok-3-mini".to_owned());
        return Some((Arc::new(rosa_llm::OpenAiProvider::xai(key)), model));
    }
    if let Ok(key) = std::env::var("OPENAI_API_KEY") {
        let model = std::env::var("ROSA_MODEL")
            .unwrap_or_else(|_| "gpt-4o-mini".to_owned());
        return Some((Arc::new(rosa_llm::OpenAiProvider::new(key)), model));
    }
    if let Ok(model) = std::env::var("OLLAMA_MODEL") {
        let base = std::env::var("OLLAMA_BASE_URL")
            .unwrap_or_else(|_| "http://localhost:11434".to_owned());
        let p = rosa_llm::OpenAiProvider::ollama().with_base_url(base);
        return Some((Arc::new(p), model));
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

## Drawing a 5-point Star
A 5-point star: forward, turn 144° (2.5133 rad), repeat 5 times.
- Use `turtle_teleport_absolute` to position, `turtle_set_pen` to choose color.
- Each side: ~2.5 units at linear_x = 1.5 units/s → duration_s = 1.67s
- Each turn: angular_z = 2.0 rad/s, turn 144° = 2.5133 rad → duration_s = 1.26s
- After drawing, send a zero-velocity to stop.

## Tool Usage Order
1. `turtle_clear` — reset canvas
2. `turtle_teleport_absolute` — move to start position (pen-up: use set_pen off=1 first)
3. `turtle_set_pen` — choose pen color (off=0 to resume drawing)
4. Loop: `turtle_publish_twist` (forward) → `turtle_publish_twist` (turn)
5. `turtle_get_pose` — verify position if needed
6. `turtle_within_bounds` — sanity check before large moves

## Important
- Always stop after each movement (send angular_z=0, linear_x=0 or rely on the tool's auto-stop).
- For precise shapes, calculate durations from speed × time = distance/angle.
- A complete 5-point star visits all 5 points in order by turning 144° between legs.
";
