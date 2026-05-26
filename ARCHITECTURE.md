# ROSA Rust Rewrite — Architecture

Companion to `ORCHESTRATION.md`. This file pins the technical decisions so per-phase briefs can stay short.

---

## Crate layout

```
rosa/
├── crates/
│   ├── rosa-core/        # agent loop, history, events, errors. No I/O deps.
│   ├── rosa-llm/         # LlmProvider trait + OpenAI/Anthropic/Ollama adapters
│   ├── rosa-tools/       # Tool trait + ToolRegistry + JSON schema derivation
│   ├── rosa-ros2/        # r2r/rclrs bridge: topics, services, params, CLI fallback
│   ├── rosa-isaac/       # Isaac Sim HTTP control tools (timeline, diagnostics, USD)
│   └── rosa-cli/         # binary `rosa`: REPL, streaming render, config loading
├── examples/
│   ├── turtle/           # phase 6: turtle demo as a Rust binary
│   └── starship/         # phase 7: Starship command/control client
├── agent-tasks/          # sub-agent briefs (markdown only)
└── ARCHITECTURE.md       # this file
```

Pre-existing Python (`src/rosa/`, `src/turtle_agent/`) stays in tree during phase 1 marked deprecated, then gets deleted in phase 6's PR.

Reasoning: split crates so the LLM adapters and the ROS bridge can be developed in parallel (phase 3 and 5 unblock independently). `rosa-core` has no I/O so it's trivial to unit-test.

---

## `rosa-core` — agent loop contract

Public surface:

```rust
pub struct Agent { /* ... */ }

impl Agent {
    pub fn builder() -> AgentBuilder;
}

pub struct AgentBuilder {
    // .llm(provider: Arc<dyn LlmProvider>)
    // .tools(registry: ToolRegistry)
    // .system_prompt(s: impl Into<String>)
    // .max_iterations(n: usize)        // default 100
    // .max_context_tokens(n: usize)    // triggers compression
    // .build() -> Result<Agent>
}

impl Agent {
    pub async fn invoke(&self, query: &str) -> Result<String>;
    pub fn stream(&self, query: &str) -> impl Stream<Item = AgentEvent>;
    pub fn clear_history(&self);
}

pub enum AgentEvent {
    Token { content: String },
    ToolStart { name: String, input: serde_json::Value },
    ToolEnd { name: String, output: serde_json::Value },
    Final { content: String },
    Error { message: String },
}
```

Event types mirror what upstream ROSA emits today (`token`, `tool_start`, `tool_end`, `final`, `error`) so the turtle demo's render code maps 1:1.

Loop:

```
loop iterations (cap = max_iterations):
    response = llm.chat(history, tools.as_provider_schema()).await
    for each chunk in response stream:
        emit Token | ToolCallDelta
    if response has tool_calls:
        for each call in parallel (bounded):
            result = tools.dispatch(call.name, call.args).await
            history.push(ToolMessage { id, result })
            emit ToolStart/ToolEnd
        continue loop
    else:
        history.push(AssistantMessage { content })
        emit Final
        break
```

This matches LangChain `AgentExecutor` semantics without depending on it.

---

## `rosa-llm` — provider trait

```rust
#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn chat(
        &self,
        messages: &[Message],
        tools: &[ToolSpec],
        opts: &ChatOptions,
    ) -> Result<BoxStream<'static, ChatChunk>>;
}

pub enum ChatChunk {
    Token { delta: String },
    ToolCall { id: String, name: String, args_delta: String },
    Finish { reason: FinishReason, usage: Option<Usage> },
}
```

Initial adapters (in priority order):

1. **OpenAI / Azure OpenAI / OpenAI-compatible** (covers OpenAI, Together, Groq, llama-cpp-server, vLLM)
2. **Anthropic Messages API** (Claude 4.x — preferred for tool use quality)
3. **Ollama** (local llama / qwen / etc.)

We do NOT depend on `async-openai` (it ships its own retry/error model that conflicts with ours). Hand-roll on `reqwest` + `eventsource-stream` for SSE — total surface is ~400 lines per adapter.

Reference (do not copy verbatim, do read for inspiration):
- `/home/propdev/.openclaw/workspace/workspace2/repos/hermes-agent/agent/anthropic_adapter.py` — how hermes handles streaming, tool deltas, retries
- `/home/propdev/.openclaw/workspace/workspace2/repos/hermes-agent/agent/prompt_caching.py` — prompt cache header handling

---

## `rosa-tools` — tool trait

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn schema(&self) -> schemars::schema::RootSchema;
    async fn execute(&self, args: serde_json::Value) -> Result<serde_json::Value>;
}

pub struct ToolRegistry { /* ... */ }

impl ToolRegistry {
    pub fn register<T: Tool + 'static>(&mut self, tool: T);
    pub fn dispatch(&self, name: &str, args: serde_json::Value) -> impl Future<...>;
    pub fn as_openai_tools(&self) -> Vec<serde_json::Value>;
    pub fn as_anthropic_tools(&self) -> Vec<serde_json::Value>;
}
```

Convention: each ROS tool is a struct implementing `Tool`. Arg structs derive `Deserialize + JsonSchema` so `schema()` is one line. Blacklist support (matching upstream rosa) lives as a constructor arg on tools that need it.

---

## `rosa-isaac` — Isaac Sim control tools

Calls the HTTP control API embedded inside `enable_ros2_bridge.py` (runs in the
`isaac-sim` Docker container at port 8282, or wherever `ISAAC_CONTROL_URL` points).

```rust
// One-liner to get all Isaac tools into a registry:
use rosa_isaac::{IsaacClient, tools::all_isaac_tools};
let registry = all_isaac_tools(ToolRegistry::new(), IsaacClient::default());
```

| Tool             | HTTP             | Description                             |
|------------------|------------------|-----------------------------------------|
| `timeline_start` | `POST /timeline/play`  | Start simulation, /clock begins ticking |
| `timeline_stop`  | `POST /timeline/stop`  | Stop + rewind to t=0                    |
| `timeline_pause` | `POST /timeline/pause` | Freeze (keep state)                     |
| `get_diagnostics`| `GET  /diagnostics`    | fps, sim_time, running, physics_dt      |
| `load_usd`       | `POST /scene/load`     | Load a USD scene file                   |
| `list_usds`      | `GET  /scene/list`     | Discover available USD scenes           |

`IsaacClient` wraps `reqwest::Client` with a 30s timeout. `ISAAC_CONTROL_URL`
env var overrides the default `http://localhost:8282`.

---

## `rosa-ros2` — ROS 2 bridge

Two layers:

1. **CLI shell-out** (parity with what upstream rosa does today via `subprocess.check_output("ros2 ...")`): `Command::new("ros2").args([...])`. Quick to ship, covers `ros2 topic list`, `ros2 node list`, `ros2 service list`, `ros2 param list`, `ros2 doctor`. Implemented in phase 5a.
2. **Native rclient** via `r2r` (preferred) or `rclrs` (official, less mature). For publishing `geometry_msgs/Twist`, subscribing to `sensor_msgs/Imu`, calling services like `turtlesim/Spawn`. Implemented in phase 5b.

Decision: ship phase 5a first so phase 6 (turtle demo) isn't blocked on r2r mastery; backfill 5b after.

The container in `Monoclaw/Deployments/Stacks/ROS/` provides the ROS 2 environment; `rosa-cli` connects to it via shared host-network DDS (no separate transport).

---

## `rosa-cli` — binary

- Config: `~/.config/rosa/config.toml` for provider keys, default model, ROS 2 domain ID.
- Interactive REPL using `rustyline` (history file at `~/.local/share/rosa/history`).
- Streaming output rendered with `crossterm` (markdown via `termimad` if needed).
- Subcommands:
  - `rosa` → REPL
  - `rosa ask "..."` → one-shot
  - `rosa tools` → list registered tools
  - `rosa doctor` → check ROS 2 reachability + provider auth

---

## Hardware budget for Isaac Sim (phase 7)

Verified on this host (2026-05-24):

| GPU                 | VRAM   | Compute | Isaac Sim 4.5+ | Notes                                  |
|---------------------|--------|---------|----------------|----------------------------------------|
| GTX 980 Ti (GPU 0)  | 6 GB   | 5.2     | ❌ no           | No RT cores; below Turing minimum      |
| RTX 3060 (GPU 1)    | 12 GB  | 8.6     | ✅ yes          | Good for Starship MVP; pin via GPU_ID=1|
| RTX 3070 (laptop)   | 8 GB   | 8.6     | ✅ yes (tight)  | Fine for turtle; tight for full sim    |

Compose pin: `device_ids: ["1"]` on the desktop, default `"0"` on the laptop. Host RAM 32 GB ≥ Isaac Sim's 32 GB recommendation. Driver 580.x ≥ required 535.x.

---

## Cross-cutting

- **Errors:** one `RosaError` enum in `rosa-core`, `thiserror`-derived; provider/tool errors implement `Into<RosaError>`. No `unwrap()` in non-test code.
- **Tracing:** `tracing` + `tracing-subscriber`; respect `RUST_LOG`. Spans on agent iterations and tool dispatches.
- **Config:** `figment` (TOML + env overrides). Provider keys ONLY from env or OS keychain, never config file.
- **Tests:** unit per crate; an integration test in `rosa-ros2/tests/` that spins up the docker compose ROS stack and asserts a topic list.
