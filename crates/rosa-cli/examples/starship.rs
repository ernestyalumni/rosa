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
use rosa_isaac::{IsaacClient, tools::all_isaac_tools};


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

// ── starship_set_gravity_body ─────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
struct SetGravityBodyArgs {
    /// Planetary body to simulate: "earth" (9.81), "moon" (1.62), "mars" (3.72), "zero".
    pub body: String,
}

struct StarshipSetGravityBody;

#[async_trait]
impl Tool for StarshipSetGravityBody {
    fn name(&self) -> &str { "starship_set_gravity_body" }
    fn description(&self) -> &str {
        "Switch the simulation gravity to a planetary body. \
         Options: 'earth' (9.81 m/s²), 'moon' (1.62 m/s²), 'mars' (3.72 m/s²), 'zero'. \
         Requires Isaac Sim to be running (ISAAC_CONTROL_URL set). \
         Call get_diagnostics after to confirm the change took effect."
    }
    fn schema(&self) -> RootSchema { schema_for!(SetGravityBodyArgs) }

    async fn execute(&self, args: Value) -> Result<Value> {
        let a: SetGravityBodyArgs = serde_json::from_value(args)?;
        let valid = ["earth", "moon", "mars", "zero"];
        if !valid.contains(&a.body.to_lowercase().as_str()) {
            return Err(RosaError::ToolExecution {
                name: self.name().into(),
                message: format!("unknown body '{}'; use: earth, moon, mars, zero", a.body),
            });
        }
        let isaac_url = std::env::var("ISAAC_CONTROL_URL")
            .unwrap_or_else(|_| "http://localhost:8282".to_owned());
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .map_err(|e| RosaError::ToolExecution { name: self.name().into(), message: e.to_string() })?;
        let resp = client
            .post(format!("{isaac_url}/physics/set_gravity"))
            .json(&serde_json::json!({ "body": a.body.to_lowercase() }))
            .send()
            .await
            .map_err(|e| RosaError::ToolExecution { name: self.name().into(), message: e.to_string() })?;
        let result: Value = resp.json().await
            .map_err(|e| RosaError::ToolExecution { name: self.name().into(), message: e.to_string() })?;
        Ok(json!({
            "gravity_body_set": a.body.to_lowercase(),
            "isaac_response": result,
        }))
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
    let base = ros2_registry_default()
        .register(StarshipGetTelemetry)
        .register(StarshipSetThrottle)
        .register(StarshipSetGimbal)
        .register(StarshipSetGravityBody)
        .register(StarshipFireRcs)
        .register(StarshipSafeMode)
        .register(StarshipReset);

    // Add Isaac Sim timeline / diagnostics / USD tools when an Isaac Sim
    // instance is reachable.  Set ISAAC_CONTROL_URL (default localhost:8282)
    // to enable.  If the env var is absent the tools are simply omitted so
    // the agent can still run against a standalone ROS 2 stub.
    let isaac_url = std::env::var("ISAAC_CONTROL_URL")
        .unwrap_or_else(|_| "http://localhost:8282".to_owned());

    // Probe reachability — don't add tools if server is clearly not running.
    // We do a quick non-blocking check rather than blocking the startup.
    // (Actual tool calls will fail gracefully if Isaac is unavailable.)
    let add_isaac = std::env::var("ISAAC_CONTROL_URL").is_ok()
        || std::net::TcpStream::connect_timeout(
            &"127.0.0.1:8282".parse().unwrap(),
            std::time::Duration::from_millis(200),
        ).is_ok();

    if add_isaac {
        println!("  isaac  → {isaac_url} (timeline + diagnostics + USD tools enabled)");
        all_isaac_tools(base, IsaacClient::new(isaac_url))
    } else {
        println!("  isaac  → not detected at localhost:8282 (timeline tools disabled)");
        println!("           set ISAAC_CONTROL_URL to enable or start Isaac Sim");
        base
    }
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
You are the Flight Control System (FSW) for Starship, a 50-m upper stage currently \
operating in the Martian proximity environment, simulated in NVIDIA Isaac Sim with \
full 6-DOF rigid-body physics. You interface via ROS 2 topics.

## Mission context — Mars proximity ops
- Vehicle spawns at 1,000 m AGL above Martian surface (gravity = 3.72 m/s²)
- Mission: controlled descent / hover / station-keeping near Mars surface
- Hover throttle is fuel-dependent: T_hover = (dry_mass + fuel_mass) × g / MAX_THRUST
  - At full fuel (830 t total): hover throttle ≈ 830000×3.72/14700000 ≈ 0.210
  - At empty (130 t dry):      hover throttle ≈ 130000×3.72/14700000 ≈ 0.033
  - Current: use engine_state fuel_kg to compute T_hover before each maneuver
- Martian atmosphere is negligible in this simulation (no drag modeled)

## Vehicle spec
- Upper stage: 50 m tall, 9 m diameter; dry mass 130,000 kg; propellant cap 700,000 kg
- Main engines: 6x Raptor Vacuum, throttleable 0.0–0.85 (HARD FSW LIMIT = 0.85)
  Max thrust: 14.7 MN. Burn rate: 2,000 kg/s at 100% throttle
- TVC gimbal: ±15° (±0.26 rad) pitch and yaw — thrust vector control
- RCS: three pairs (top / mid_fwd / mid_aft), unitless impulse −1 to +1 per axis
- Propellant: fuel_fraction 0.0 (empty) → 1.0 (full), ~700 t capacity

## Telemetry topics (read)
  /starship/pose           geometry_msgs/PoseStamped   100 Hz — 6-DOF state
  /starship/imu            sensor_msgs/Imu             200 Hz — angular rates + accel
  /starship/altitude       std_msgs/Float64             50 Hz  — metres AGL
  /starship/velocity       geometry_msgs/Vector3Stamped 50 Hz  — m/s
  /starship/fuel_fraction  std_msgs/Float32              1 Hz  — 0.0 to 1.0
  /starship/engine_state   std_msgs/String              10 Hz  — JSON (throttle, gimbal, fuel_kg)

## Command topics (write)
  /starship/main_throttle  std_msgs/Float32    0.0–0.85 HARD FSW LIMIT
  /starship/main_gimbal    geometry_msgs/Vector3   pitch/yaw rad (x=pitch, y=yaw)
  /starship/rcs/top        geometry_msgs/Vector3   impulse vector (x/y/z ∈ [−1,1])
  /starship/rcs/mid_fwd    geometry_msgs/Vector3
  /starship/rcs/mid_aft    geometry_msgs/Vector3
  /starship/safe_mode      std_msgs/Bool

## FSW rules (MUST follow — these are hard constraints)
1. ALWAYS call `starship_get_telemetry` before issuing any command.
2. NEVER set throttle > 0.85. If asked to exceed it, refuse and explain FSW limit.
3. If fuel_fraction < 0.05: IMMEDIATELY `starship_safe_mode(engage=true)` and report.
4. On 'abort' or 'emergency': throttle=0, engage safe_mode, report status, wait for instructions.
5. After EVERY command, re-read telemetry to confirm state transition occurred.
6. Always report altitude_m, fuel_fraction, and engine_state in every response.
7. For descent burns: calculate ΔV budget before committing. At Mars g=3.72 m/s², \
   a 0.1 throttle step ≈ ±(14.7MN×0.1)/130000 kg = 11.3 m/s² net accel.

## GNC guidance
- Hover throttle: T = (dry_mass + fuel_kg) × g_mars / MAX_THRUST
  Example: full fuel → (130000+700000)×3.72/14700000 = 0.210; empty → 0.033
- Descent: throttle slightly below T_hover to allow controlled descent rate.
- Arrest descent: throttle above T_hover briefly, then trim to T_hover for hover.
- Attitude control: use `starship_fire_rcs` for attitude, gimbal for translation authority.
- Dead-band: avoid throttle jitter < 0.005 step size.
- After landing (altitude ≈ 0 m): call `starship_reset` before new maneuvers.

## GNC calculation examples
- Get fuel_kg from engine_state JSON field 'fuel_kg'
- T_hover = (130000 + fuel_kg) * 3.72 / 14700000
- Net accel = throttle * 14.7M / total_mass - 3.72  (positive = climbing)
- At 0.25 throttle, full fuel: a_net = 0.25*14700000/830000 - 3.72 = 4.43-3.72 = +0.71 m/s² (climbing)
";

