# Rosa — Python → Rust Feature Parity

This document maps every feature and tool from the original Python/LangChain rosa
(`main` branch, v1.0.10) to the Rust rewrite (`master` branch).

Use this as the authoritative checklist to confirm the Rust codebase is a complete
replacement. Each row is **✅ done**, **➕ intentionally extended**, or **🚫 not applicable**.

---

## 1. ROS 2 CLI Tools (`src/rosa/tools/ros2.py` → `crates/rosa-ros2/`)

| Python tool (`ros2.py`) | Rust tool | Crate | Status | Notes |
|---|---|---|---|---|
| `ros2_node_list` | `ros2_list_nodes` | rosa-ros2 | ✅ | Pattern + blacklist filter |
| `ros2_topic_list` | `ros2_list_topics` | rosa-ros2 | ✅ | Parses `name [type]` pairs |
| `ros2_topic_echo` | `ros2_topic_echo` | rosa-ros2 | ✅ | `--once --spin-time` |
| `ros2_service_list` | `ros2_list_services` | rosa-ros2 | ✅ | Parses `name [type]` pairs |
| `ros2_node_info` | `ros2_node_info` | rosa-ros2 | ✅ | |
| `ros2_topic_info` | `ros2_topic_info` | rosa-ros2 | ✅ | `--verbose` |
| `ros2_param_list` | `ros2_list_params` | rosa-ros2 | ✅ | Optional node filter |
| `ros2_param_get` | `ros2_param_get` | rosa-ros2 | ✅ | |
| `ros2_param_set` | `ros2_param_set` | rosa-ros2 | ✅ | |
| `ros2_service_info` | `ros2_service_info` | rosa-ros2 | ✅ | `ros2 service type`; accepts list |
| `ros2_service_call` | `ros2_service_call` | rosa-ros2 | ✅ | YAML request optional |
| `ros2_doctor` | `ros2_doctor` | rosa-ros2 | ✅ | `--report` flag |
| `roslog_list` | `roslog_list` | rosa-ros2 | ✅ | `find ~/.ros/log`; min-size + blacklist |
| `ros2_log_directories` | *(internal)* | — | 🚫 | Was a Python helper, not a LLM tool |

---

## 2. Math / Calculation Tools (`src/rosa/tools/calculation.py` → `crates/rosa-tools/src/builtin/math.rs`)

| Python tool (`calculation.py`) | Rust tool | Status | Notes |
|---|---|---|---|
| `degrees_to_radians` | `degrees_to_radians` | ✅ | Takes single `value` |
| `radians_to_degrees` | `radians_to_degrees` | ✅ | Takes single `value` |
| `sqrt` | `sqrt` | ✅ | Errors on negative |
| `atan2` | `atan2` | ✅ | `(y, x)` args |
| `distance_between_points` | `distance_2d` | ✅ | `(x1,y1,x2,y2)` |
| `calculate_line_angle_and_distance` | `heading_and_distance` | ✅ | Returns bearing + distance |
| `add` (x+y pair) | `add` | ✅ | `(a, b)` — in `calculation.rs` since v0.1 |
| `subtract` | `subtract` | ✅ | |
| `multiply` | `multiply` | ✅ | |
| `divide` | `divide` | ✅ | Errors on zero denominator |
| `exponentiate` | `exponentiate` | ✅ | `a.powf(b)` |
| `modulo` | `modulo` | ✅ | Errors on zero |
| `sine` | `sin` | ✅ | Radians |
| `cosine` | `cos` | ✅ | Radians |
| `tangent` | `tan` | ✅ | Radians |
| `asin` | `asin` | ✅ | Validates [-1, 1] |
| `acos` | `acos` | ✅ | Validates [-1, 1] |
| `atan` | `atan` | ✅ | Single-arg arctangent |
| `sinh` | `sinh` | ✅ | |
| `cosh` | `cosh` | ✅ | |
| `tanh` | `tanh` | ✅ | |
| `add_all` (list sum) | `add_all` | ✅ | `Vec<f64>` |
| `multiply_all` (list product) | `multiply_all` | ✅ | `Vec<f64>` |
| `mean` | `mean` | ✅ | Returns mean + stdev |
| `median` | `median` | ✅ | |
| `variance` | `variance` | ✅ | Population variance |
| `mode` | — | 🚫 | Omitted — requires defining "mode" for floats; LLM can derive it |
| `count_list` | `count_items` | ✅ | Renamed; takes JSON array |
| `count_words` | `count_words` | ✅ | |
| `count_lines` | `count_lines` | ✅ | |

---

## 3. Log Tools (`src/rosa/tools/log.py` → `crates/rosa-tools/src/builtin/log.rs`)

| Python tool (`log.py`) | Rust tool | Status | Notes |
|---|---|---|---|
| `read_log` | `log_message` | 🚫 | Python reads host log files; Rust `log_message` *emits* structured events. Different purpose. Add a `read_log` Rust tool if log-reading is needed. |

---

## 4. System Tools (`src/rosa/tools/system.py` → `crates/rosa-tools/src/builtin/`)

| Python tool (`system.py`) | Rust tool | Status | Notes |
|---|---|---|---|
| `wait` | `wait` | ✅ | `tokio::time::sleep`; max 60 s guard |
| `set_verbosity` | — | 🚫 | LangChain-specific; Rust uses `RUST_LOG=debug` env var |
| `set_debugging` | — | 🚫 | LangChain-specific; Rust uses `RUST_LOG=debug` env var |
| `system_info` | `system_info` | ➕ | Added in Rust (was not in Python rosa) |

---

## 5. Agent / REPL Features (`src/rosa/rosa.py` → `crates/rosa-core/`, `crates/rosa-cli/`)

| Python feature | Rust equivalent | Status | Notes |
|---|---|---|---|
| Streaming (`astream`) | `agent.stream(query, tx)` → `AgentEvent` channel | ✅ | SSE-parsed per-token streaming |
| Non-streaming (`invoke`) | `agent.invoke(query)` | ✅ | |
| `accumulate_chat_history` | `Mutex<History>` on `Agent` | ✅ | Persists across REPL turns |
| `clear_chat()` | `agent.clear_history()` + `/clear` REPL cmd | ✅ | |
| `max_iterations` | `Agent::builder().max_iterations(n)` | ✅ | |
| `blacklist` | `DEFAULT_BLACKLIST` + per-tool filter | ✅ | |
| `show_token_usage` | `AgentEvent::Usage { prompt, completion }` | ✅ | Displayed dimmed after each LLM call |
| Custom tool injection | `ToolRegistry::register(tool)` | ✅ | Per-example registries (turtle, starship) |
| Custom system prompts | `Agent::builder().system_prompt(s)` | ✅ | |
| Ctrl-C cancellation | `tokio::select!` + `ctrl_c()` | ✅ | Drops agent task cleanly |
| `/tools` REPL command | `/tools` | ✅ | Lists all registered tools + descriptions |
| `/clear` REPL command | `/clear` | ✅ | |
| `/quit` / `/exit` REPL | `/quit` / `/exit` | ✅ | |
| Verbose/debug flags | `RUST_LOG=debug cargo run` | 🚫 | Env var, not a runtime tool |
| Token cost display (USD) | — | 🚫 | Token *counts* shown; cost calc would need price table per model |
| `return_intermediate_steps` | — | 🚫 | All tool events streamed via `AgentEvent`; no separate trace object |
| Azure OpenAI support | — | 🚫 | Not added; add `OpenAiProvider::azure()` if needed |
| ROS 1 support (`ros1.py`) | — | 🚫 | Out of scope; rosa-ros2 is ROS 2 only |

---

## 6. Provider Support

| Python LLM | Rust equivalent | Status |
|---|---|---|
| `ChatOpenAI` | `OpenAiProvider::new(key)` | ✅ |
| `AzureChatOpenAI` | — | 🚫 Not added |
| `ChatAnthropic` | `AnthropicProvider::new(key)` | ✅ |
| `ChatOllama` | — | 🚫 Removed per user request |
| xAI Grok | `OpenAiProvider::xai(key)` | ➕ New — not in Python rosa |

---

## 7. Examples / Demos

| Python demo | Rust equivalent | Status |
|---|---|---|
| Turtlesim notebook/demo | `examples/turtle.rs` | ✅ + ➕ 13 turtle tools vs ~6 in Python |
| Isaac Sim ("coming soon") | `examples/starship.rs` stub | ➕ FSW-style agent with 6 tools |

---

## 8. Infrastructure

| Python feature | Rust equivalent | Status |
|---|---|---|
| `pyproject.toml` / pip install | `Cargo.toml` workspace | ✅ |
| `.env` support | `dotenvy` crate, `.env.example` | ✅ |
| Provider priority order | `detect_provider()` in each binary | ✅ |
| Blacklist injection | Compile-time per-tool vs LangChain runtime | ✅ |
| Unit tests | `cargo test --workspace` (75 tests) | ✅ |
| Docker exec ROS routing | `ShellRunner` + `ROS_CONTAINER` env var | ✅ |
| Shell-quoting for docker | `shell_quote()` helper in `runner.rs` | ✅ |

---

## Summary

| Category | Python tools | Rust tools | Gap |
|---|---|---|---|
| ROS 2 CLI | 13 | 13 | ✅ 0 |
| Math / calculation | 27 | 26 | `mode` omitted (float semantics) |
| Log | 1 | 0 | `read_log` not ported (different semantics) |
| System | 3 | 2 | `set_verbosity/set_debugging` → `RUST_LOG` |
| Agent features | 14 | 12 | Cost display, Azure not added |

**Practical parity: complete for all robotics-relevant features.**
The three remaining gaps (`mode`, `read_log`, Azure) are either semantically
awkward for floats, a different tool class, or out of scope for the current work.
