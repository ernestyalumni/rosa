//! rosa-starship — FSW command-and-control demo for the Starship sim.
//!
//! Works with either:
//! - Full Isaac Sim stack (Monoclaw/Deployments/Stacks/IsaacSim) for physics
//! - Fallback stub (`examples/starship/stub/run_stub.sh`) for quick demos
//!
//! # Run
//! ```bash
//! ROS_CONTAINER=rosa-ros2 ANTHROPIC_API_KEY=... \
//!   cargo run --example starship -p rosa-cli
//! ```
//!
//! # Demo prompts
//! - "What's the current altitude and fuel fraction?"
//! - "Hover at 500 m for 10 seconds."
//! - "Fuel is below 5% — what do you do?"
//! - "Abort."

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
// Shared ros2 helper (same as turtle.rs)
// ---------------------------------------------------------------------------

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

async fn ros2_exec(args: &[&str], timeout_secs: u64) -> std::result::Result<String, String> {
    let ros_container = std::env::var("ROS_CONTAINER").ok();
    let owned_cmd: String;
    let (program, full_args): (&str, Vec<&str>) = if let Some(ref c) = ros_container {
        owned_cmd = format!(
            "ros2 {}",
            args.iter().map(|a| shell_quote(a)).collect::<Vec<_>>().join(" ")
        );
        ("docker", vec!["exec", c.as_str(), "bash", "-ic", owned_cmd.as_str()])
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

// Helper: publish a std_msgs Float32 / Bool / String to a topic
async fn pub_once(topic: &str, msg_type: &str, data_yaml: &str) -> std::result::Result<(), String> {
    ros2_exec(
        &["topic", "pub", "--once", topic, msg_type, data_yaml],
        5,
    )
    .await
    .map(|_| ())
}

// Helper: echo --once from a topic
async fn echo_once(topic: &str, msg_type: &str) -> std::result::Result<String, String> {
    ros2_exec(&["topic", "echo", "--once", topic, msg_type], 10).await
}

// ---------------------------------------------------------------------------
// Starship tools
// ---------------------------------------------------------------------------

// ── starship_get_telemetry ────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
struct GetTelemetryArgs {}

struct StarshipGetTelemetry;

#[async_trait]
impl Tool for StarshipGetTelemetry {
    fn name(&self) -> &str { "starship_get_telemetry" }
    fn description(&self) -> &str {
        "Snapshot all Starship telemetry: pose, altitude, velocity, fuel_fraction, engine_state. \
         Always call this before issuing any command."
    }
    fn schema(&self) -> RootSchema { schema_for!(GetTelemetryArgs) }

    async fn execute(&self, args: Value) -> Result<Value> {
        let _: GetTelemetryArgs = serde_json::from_value(args)?;

        // Read altitude and fuel (lowest latency topics)
        let altitude = echo_once("/starship/altitude", "std_msgs/msg/Float64")
            .await.unwrap_or_else(|e| format!("unavailable: {e}"));
        let fuel = echo_once("/starship/fuel_fraction", "std_msgs/msg/Float32")
            .await.unwrap_or_else(|e| format!("unavailable: {e}"));
        let engine = echo_once("/starship/engine_state", "std_msgs/msg/String")
            .await.unwrap_or_else(|e| format!("unavailable: {e}"));
        let velocity = echo_once("/starship/velocity", "geometry_msgs/msg/Vector3Stamped")
            .await.unwrap_or_else(|e| format!("unavailable: {e}"));

        // Parse simple values
        let alt_m  = parse_float_field(&altitude, "data").unwrap_or(f64::NAN);
        let fuel_f = parse_float_field(&fuel,     "data").unwrap_or(f64::NAN);

        Ok(json!({
            "altitude_m":      alt_m,
            "fuel_fraction":   fuel_f,
            "engine_state":    engine.trim(),
            "velocity_raw":    velocity.trim(),
            "altitude_raw":    altitude.trim(),
        }))
    }
}

// ── starship_set_throttle ─────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
struct SetThrottleArgs {
    /// Normalized throttle 0.0 (off) to 1.0 (100%). Hard limit: ≤ 0.85.
    pub throttle: f32,
}

struct StarshipSetThrottle;

#[async_trait]
impl Tool for StarshipSetThrottle {
    fn name(&self) -> &str { "starship_set_throttle" }
    fn description(&self) -> &str {
        "Set Raptor engine throttle (0.0–0.85, hard FSW limit). \
         0.0 = off, 0.85 = maximum. NEVER exceed 0.85."
    }
    fn schema(&self) -> RootSchema { schema_for!(SetThrottleArgs) }

    async fn execute(&self, args: Value) -> Result<Value> {
        let mut a: SetThrottleArgs = serde_json::from_value(args)?;

        // FSW hard limit
        if a.throttle > 0.85 {
            return Err(RosaError::ToolExecution {
                name: self.name().into(),
                message: format!(
                    "FSW LIMIT: throttle {:.2} exceeds hard limit 0.85. Clamped, not applied.",
                    a.throttle
                ),
            });
        }
        if a.throttle < 0.0 { a.throttle = 0.0; }

        let yaml = format!("{{data: {:.3}}}", a.throttle);
        pub_once("/starship/main_throttle", "std_msgs/msg/Float32", &yaml)
            .await
            .map_err(|e| RosaError::ToolExecution { name: self.name().into(), message: e })?;

        Ok(json!({ "throttle_set": a.throttle }))
    }
}

// ── starship_set_gimbal ───────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
struct SetGimbalArgs {
    /// Pitch angle in radians (−0.26 to +0.26, i.e. ±15°).
    pub pitch: f32,
    /// Yaw angle in radians (−0.26 to +0.26).
    pub yaw: f32,
}

struct StarshipSetGimbal;

#[async_trait]
impl Tool for StarshipSetGimbal {
    fn name(&self) -> &str { "starship_set_gimbal" }
    fn description(&self) -> &str {
        "Set main engine gimbal pitch and yaw (±0.26 rad / ±15°). \
         x=pitch, y=yaw, z=unused."
    }
    fn schema(&self) -> RootSchema { schema_for!(SetGimbalArgs) }

    async fn execute(&self, args: Value) -> Result<Value> {
        let a: SetGimbalArgs = serde_json::from_value(args)?;
        let pitch = a.pitch.clamp(-0.26, 0.26);
        let yaw   = a.yaw  .clamp(-0.26, 0.26);
        let yaml  = format!("{{x: {pitch:.4}, y: {yaw:.4}, z: 0.0}}");
        pub_once("/starship/main_gimbal", "geometry_msgs/msg/Vector3", &yaml)
            .await
            .map_err(|e| RosaError::ToolExecution { name: self.name().into(), message: e })?;
        Ok(json!({ "gimbal_pitch_rad": pitch, "gimbal_yaw_rad": yaw }))
    }
}

// ── starship_fire_rcs ─────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
struct FireRcsArgs {
    /// Which thruster group: `top`, `mid_fwd`, or `mid_aft`.
    pub group: String,
    /// Impulse vector (x, y, z) normalized −1 to +1 per axis.
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

struct StarshipFireRcs;

#[async_trait]
impl Tool for StarshipFireRcs {
    fn name(&self) -> &str { "starship_fire_rcs" }
    fn description(&self) -> &str {
        "Fire an RCS thruster group (`top`, `mid_fwd`, or `mid_aft`) with a \
         unitless impulse vector (x/y/z each −1 to +1)."
    }
    fn schema(&self) -> RootSchema { schema_for!(FireRcsArgs) }

    async fn execute(&self, args: Value) -> Result<Value> {
        let a: FireRcsArgs = serde_json::from_value(args)?;
        let topic = match a.group.as_str() {
            "top"      => "/starship/rcs/top",
            "mid_fwd"  => "/starship/rcs/mid_fwd",
            "mid_aft"  => "/starship/rcs/mid_aft",
            other      => return Err(RosaError::ToolExecution {
                name: self.name().into(),
                message: format!("unknown RCS group '{other}'; use top, mid_fwd, or mid_aft"),
            }),
        };
        let x = a.x.clamp(-1.0, 1.0);
        let y = a.y.clamp(-1.0, 1.0);
        let z = a.z.clamp(-1.0, 1.0);
        let yaml = format!("{{x: {x:.3}, y: {y:.3}, z: {z:.3}}}");
        pub_once(topic, "geometry_msgs/msg/Vector3", &yaml)
            .await
            .map_err(|e| RosaError::ToolExecution { name: self.name().into(), message: e })?;
        Ok(json!({ "rcs_group": a.group, "impulse": { "x": x, "y": y, "z": z } }))
    }
}

// ── starship_safe_mode ────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
struct SafeModeArgs {
    /// Set to true to engage safe mode, false to disengage.
    pub engage: bool,
}

struct StarshipSafeMode;

#[async_trait]
impl Tool for StarshipSafeMode {
    fn name(&self) -> &str { "starship_safe_mode" }
    fn description(&self) -> &str {
        "Engage or disengage vehicle safe mode. In safe mode: \
         throttle=0, gimbals neutral, RCS off, attitude hold only. \
         ALWAYS engage if fuel < 5% or on abort."
    }
    fn schema(&self) -> RootSchema { schema_for!(SafeModeArgs) }

    async fn execute(&self, args: Value) -> Result<Value> {
        let a: SafeModeArgs = serde_json::from_value(args)?;
        let yaml = format!("{{data: {}}}", if a.engage { "true" } else { "false" });
        // Also zero out throttle when engaging
        if a.engage {
            let _ = pub_once("/starship/main_throttle", "std_msgs/msg/Float32", "{data: 0.0}").await;
        }
        pub_once("/starship/safe_mode", "std_msgs/msg/Bool", &yaml)
            .await
            .map_err(|e| RosaError::ToolExecution { name: self.name().into(), message: e })?;
        Ok(json!({ "safe_mode": a.engage, "throttle_zeroed": a.engage }))
    }
}

// ── starship_reset ────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
struct ResetArgs {}

struct StarshipReset;

#[async_trait]
impl Tool for StarshipReset {
    fn name(&self) -> &str { "starship_reset" }
    fn description(&self) -> &str {
        "Reset the Starship simulation to the launch pad configuration \
         (calls /starship/reset service)."
    }
    fn schema(&self) -> RootSchema { schema_for!(ResetArgs) }

    async fn execute(&self, args: Value) -> Result<Value> {
        let _: ResetArgs = serde_json::from_value(args)?;
        ros2_exec(
            &["service", "call", "/starship/reset", "std_srvs/srv/Empty", "{}"],
            5,
        )
        .await
        .map_err(|e| RosaError::ToolExecution { name: self.name().into(), message: e })?;
        Ok(json!({ "reset": true }))
    }
}

// ---------------------------------------------------------------------------
// Helper: parse a `data:` or `x:` etc. field from ROS 2 YAML echo output
// ---------------------------------------------------------------------------

fn parse_float_field(output: &str, field: &str) -> Option<f64> {
    for line in output.lines() {
        if let Some(v) = line.trim().strip_prefix(&format!("{field}:")) {
            return v.trim().parse().ok();
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

fn starship_registry() -> ToolRegistry {
    ros2_registry_default()
        .register(StarshipGetTelemetry)
        .register(StarshipSetThrottle)
        .register(StarshipSetGimbal)
        .register(StarshipFireRcs)
        .register(StarshipSafeMode)
        .register(StarshipReset)
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

    let (provider, model) = detect_provider().unwrap_or_else(|| {
        eprintln!("Set ANTHROPIC_API_KEY, XAI_API_KEY, or OPENAI_API_KEY (or copy .env.example → .env).");
        std::process::exit(1);
    });

    let registry = Arc::new(starship_registry());

    let agent = Agent::builder()
        .llm(provider)
        .tools(registry)
        .system_prompt(STARSHIP_SYSTEM_PROMPT)
        .max_iterations(20)
        .opts(ChatOptions { model: model.clone(), max_tokens: Some(4096), temperature: None })
        .build()
        .expect("failed to build agent");

    // Banner
    println!("┌───────────────────────────────────────────────────────────┐");
    println!("│  rosa-starship  •  FSW command-and-control demo            │");
    println!("│  model: {model:<53}│");
    println!("│  /quit to exit  •  Ctrl-C cancels a turn                  │");
    println!("└───────────────────────────────────────────────────────────┘");
    if let Ok(c) = std::env::var("ROS_CONTAINER") { println!("  ros2 → docker exec {c}"); }
    println!();
    println!("  Try: 'What is the current altitude and fuel fraction?'");
    println!("       'Hover the vehicle at 500 m.'");
    println!("       'Fuel is below 5% — what do you do?'");
    println!("       'Abort.'");
    println!();

    // REPL
    loop {
        print!("\x1b[36m> \x1b[0m"); // cyan for starship
        io::stdout().flush().ok();

        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(0) => { println!("\nAbort sequence complete. Bye!"); break; }
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
// Shared helpers
// ---------------------------------------------------------------------------

fn detect_provider() -> Option<(Arc<dyn rosa_core::provider::LlmProvider>, String)> {
    if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
        let model = std::env::var("ROSA_MODEL").unwrap_or_else(|_| "claude-sonnet-4-6".to_owned());
        return Some((Arc::new(rosa_llm::AnthropicProvider::new(key)), model));
    }
    if let Ok(key) = std::env::var("XAI_API_KEY") {
        let model = std::env::var("ROSA_MODEL").unwrap_or_else(|_| "grok-4.3".to_owned());
        return Some((Arc::new(rosa_llm::OpenAiProvider::xai(key)), model));
    }
    if let Ok(key) = std::env::var("OPENAI_API_KEY") {
        let model = std::env::var("ROSA_MODEL").unwrap_or_else(|_| "gpt-5.5".to_owned());
        return Some((Arc::new(rosa_llm::OpenAiProvider::new(key)), model));
    }
    None
}

async fn run_turn(agent: &Agent, query: &str) {
    let (tx, mut rx) = mpsc::unbounded_channel::<AgentEvent>();
    let render = async {
        while let Some(event) = rx.recv().await {
            match event {
                AgentEvent::Token { content } => { print!("{content}"); io::stdout().flush().ok(); }
                AgentEvent::ToolStart { name, input } => { println!("\n\x1b[2m  ↗ {name}({input})\x1b[0m"); }
                AgentEvent::ToolEnd   { name, output } => {
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
// FSW system prompt
// ---------------------------------------------------------------------------

const STARSHIP_SYSTEM_PROMPT: &str = "\
You are flight-control software (FSW) for a Starship-class vehicle simulated in NVIDIA Isaac Sim \
via ROS 2 topics. You reason in natural language and execute precise ROS 2 tool calls.

## Vehicle spec
- Upper stage: ~50 m tall, 9 m diameter, 1.3 M kg dry mass (illustrative)
- Main engines: Raptor cluster, throttleable 0.0–0.85 (HARD FSW LIMIT)
- Gimbal range: ±15° (±0.26 rad) pitch and yaw
- RCS: three pairs (top / mid_fwd / mid_aft), unitless impulse −1 to +1 per axis
- Propellant: fuel_fraction 0.0 (empty) to 1.0 (full)

## Topic contract
Telemetry IN (read-only):
  /starship/pose           geometry_msgs/PoseStamped   100 Hz
  /starship/imu            sensor_msgs/Imu             200 Hz
  /starship/altitude       std_msgs/Float64             50 Hz  ← metres AGL
  /starship/velocity       geometry_msgs/Vector3Stamped 50 Hz
  /starship/fuel_fraction  std_msgs/Float32              1 Hz  ← 0.0–1.0
  /starship/engine_state   std_msgs/String              10 Hz  ← JSON

Commands OUT (write-only):
  /starship/main_throttle  std_msgs/Float32  0.0–0.85 (FSW HARD LIMIT)
  /starship/main_gimbal    geometry_msgs/Vector3  pitch/yaw rad
  /starship/rcs/top|mid_fwd|mid_aft  geometry_msgs/Vector3  impulse
  /starship/safe_mode      std_msgs/Bool

## FSW rules (MUST follow)
1. ALWAYS call `starship_get_telemetry` before any command sequence.
2. NEVER set throttle > 0.85. Violating this is a fault condition.
3. If fuel_fraction < 0.05: IMMEDIATELY call `starship_safe_mode(engage=true)`.
4. On 'abort': set throttle=0, engage safe_mode, report status.
5. After each command, re-read telemetry to confirm state transition.
6. Report altitude, fuel, and engine state in every response.

## Hovering logic
To hover at altitude H: throttle ≈ (vehicle_mass × g) / max_thrust.
For this vehicle, hover throttle ≈ 0.45–0.55 depending on propellant load.
Use small throttle adjustments (±0.05) to null altitude error.
";
