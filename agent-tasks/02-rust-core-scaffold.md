# Task 02 — Rust Core Scaffold

**Owner role:** Rust agent
**Blocked by:** 01 (clean branch)
**Blocks:** 03, 04, 05
**Estimate:** 2–3 hr

## Goal

Stand up the Cargo workspace + the `rosa-core` crate with the agent loop, history, events, and errors as defined in `ARCHITECTURE.md`. No network I/O, no ROS, no LLM provider — those are tasks 03 and 05.

## Context — why

`ARCHITECTURE.md` splits into 5 crates so tasks 03/04/05 can land in parallel. This task creates the workspace and the one crate they all depend on. If the trait/event shapes here are wrong, every downstream task pays for it — so spend the first 30 min reading both `ORCHESTRATION.md` and `ARCHITECTURE.md` end to end before writing code.

## Files to create

```
rosa/
├── Cargo.toml                          # workspace manifest
├── rust-toolchain.toml                 # pin stable channel
├── .cargo/config.toml                  # optional: lld linker on Linux
├── crates/
│   └── rosa-core/
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs                  # re-exports
│           ├── agent.rs                # Agent, AgentBuilder
│           ├── history.rs              # Message enum, History
│           ├── events.rs               # AgentEvent
│           ├── error.rs                # RosaError (thiserror)
│           └── tests/                  # unit tests
```

## Required deps for `rosa-core/Cargo.toml`

```toml
async-trait = "0.1"
futures = "0.3"
tokio = { version = "1", features = ["rt-multi-thread", "sync", "time", "macros"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "1"
tracing = "0.1"
```

No reqwest, no clap, no rclrs in this crate — keep it I/O-free.

## Required type shapes (must match exactly so adapters compile)

```rust
// history.rs
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Message {
    System { content: String },
    User { content: String },
    Assistant { content: Option<String>, tool_calls: Vec<ToolCall> },
    Tool { call_id: String, content: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolCall { pub id: String, pub name: String, pub arguments: serde_json::Value }
```

```rust
// events.rs — mirrors upstream rosa's event types so the demo render maps 1:1
pub enum AgentEvent {
    Token { content: String },
    ToolStart { name: String, input: serde_json::Value },
    ToolEnd { name: String, output: serde_json::Value },
    Final { content: String },
    Error { message: String },
}
```

```rust
// agent.rs — see ARCHITECTURE.md "rosa-core — agent loop contract"
```

The agent loop in this scaffold is allowed to take `Box<dyn LlmProvider>` and `ToolRegistry` as **stub traits** defined inline in `rosa-core` (or behind a `mock` feature), so task 02 can ship and compile before 03/04 land. The real `LlmProvider` and `Tool` traits then move into their dedicated crates in 03/04 and `rosa-core` re-exports them.

## Acceptance criteria

- [ ] `cargo build --workspace` succeeds with zero warnings on stable.
- [ ] `cargo test -p rosa-core` passes; tests cover (a) building a history, (b) the agent loop with a mock provider that returns one tool call then a final message, (c) max-iteration cap fires.
- [ ] No `unwrap()` outside `#[cfg(test)]`.
- [ ] `cargo clippy --workspace -- -D warnings` clean.
- [ ] PR titled `feat(rosa-core): agent loop scaffold` against `feat/rust-rewrite`.

## Out of scope

- Real HTTP / SSE parsing (task 03).
- Real tool implementations (task 04).
- ROS 2 (task 05).
- Streaming render — the `AgentEvent` stream is sufficient; CLI rendering is task 06.

## Reference reading (do not copy)

- `/home/propdev/.openclaw/workspace/workspace2/repos/hermes-agent/run_agent.py` — search for "agent loop" patterns; large file (16k lines), use grep not full read.
- `/home/propdev/.openclaw/workspace/workspace2/repos/hermes-agent/agent/context_engine.py` — short, shows clean history management.
- Upstream rosa's `src/rosa/rosa.py` (deleted in task 01 but still in git history): `invoke()` and `astream()` are the behavioral spec we're matching.
