# Task 04 — Tool Trait + Registry

**Owner role:** Rust agent
**Blocked by:** 02
**Blocks:** 05 (ROS tools depend on this trait), 06
**Parallel with:** 03, 05
**Estimate:** 2–3 hr

## Goal

Implement `crates/rosa-tools/` with the `Tool` trait, `ToolRegistry`, JSON Schema derivation, and a handful of provider-agnostic built-in tools (matching upstream rosa's `calculation`, `log`, `system` — the ones that have nothing to do with ROS).

## Context — why a separate crate

Splitting `rosa-tools` from `rosa-ros2` means an agent without ROS can still use rosa (e.g. for a CLI assistant). It also keeps the trait definition stable while ROS-specific tools evolve.

## Files to create

```
crates/rosa-tools/
├── Cargo.toml
└── src/
    ├── lib.rs              # Tool trait, ToolRegistry, tool! macro (optional)
    ├── builtin/
    │   ├── mod.rs
    │   ├── calculation.rs  # eval safe math expressions
    │   ├── log.rs          # log messages with severity
    │   └── system.rs       # `hostname`, `uname -a`, `date` etc.
    └── tests/
```

## Trait shape

See `ARCHITECTURE.md` "rosa-tools — tool trait". The key contract: `schema()` returns a `RootSchema` from `schemars` derived from the arg struct, and the registry's `as_openai_tools()` / `as_anthropic_tools()` translate it into the provider-specific JSON shape.

## Deps

```toml
rosa-core = { path = "../rosa-core" }
async-trait = "0.1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
schemars = "0.8"
thiserror = "1"
tracing = "0.1"

[dev-dependencies]
tokio = { version = "1", features = ["macros", "rt"] }
```

## Example tool (use as the unit-test fixture)

```rust
#[derive(Deserialize, JsonSchema)]
pub struct AddArgs { pub a: f64, pub b: f64 }

pub struct AddTool;

#[async_trait]
impl Tool for AddTool {
    fn name(&self) -> &str { "add" }
    fn description(&self) -> &str { "Add two numbers and return the sum." }
    fn schema(&self) -> RootSchema { schema_for!(AddArgs) }
    async fn execute(&self, args: Value) -> Result<Value> {
        let args: AddArgs = serde_json::from_value(args)?;
        Ok(json!(args.a + args.b))
    }
}
```

## Blacklist support

Upstream rosa lets the caller pass a blacklist (`["master", "docker"]`) injected into tools that filter ROS entities. Implement this via a `Tool` trait method `fn requires_blacklist() -> bool { false }` and a `ToolRegistry::with_blacklist(Vec<String>)` constructor. Tools that opt in receive the blacklist in their `execute` args under a reserved `_blacklist` key. (This is cleaner than upstream's `inject_blacklist` signature-rewriting decorator — see `src/rosa/tools/__init__.py:22–62` in git history for the gnarly version we're replacing.)

## Acceptance criteria

- [ ] `cargo test -p rosa-tools` passes: (i) register a tool, (ii) dispatch by name, (iii) schema round-trips through OpenAI tool shape and back, (iv) unknown tool name → `RosaError::ToolNotFound`, (v) arg deserialize error surfaces as `RosaError::ToolBadArgs` not a panic.
- [ ] Three builtin tools (`add`/calculation, `log_message`, `system_info`) registered, each with a test.
- [ ] `as_openai_tools()` output validated against a saved JSON fixture matching OpenAI's expected shape (`type: "function"`, `function: { name, description, parameters }`).
- [ ] `as_anthropic_tools()` output validated against Anthropic's shape (`name, description, input_schema`).

## Out of scope

- ROS-specific tools (task 05).
- Tool authorization / sandboxing — defer to a follow-up.
