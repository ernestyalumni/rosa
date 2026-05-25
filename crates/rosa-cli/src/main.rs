//! rosa — Rust-first ROS 2 agent CLI.
//!
//! ## Usage
//!
//! ```bash
//! # Copy .env.example → .env, fill in your key, then:
//! cargo run -p rosa-cli
//!
//! # Or pass keys inline (env vars override .env)
//! ANTHROPIC_API_KEY=sk-ant-... cargo run -p rosa-cli
//! XAI_API_KEY=xai-...         cargo run -p rosa-cli
//! OPENAI_API_KEY=sk-...       cargo run -p rosa-cli
//!
//! # Override model for any provider
//! ANTHROPIC_API_KEY=... ROSA_MODEL=claude-opus-4-7 cargo run -p rosa-cli
//!
//! # Via docker exec into the ROS container
//! ROS_CONTAINER=rosa-ros2 ANTHROPIC_API_KEY=... cargo run -p rosa-cli
//! ```
//!
//! ## Commands
//! - `/quit` or `/exit` — exit the REPL
//! - `/tools` — list available tools
//! - Ctrl-C — cancel the current agent turn

use std::io::{self, Write};
use std::sync::Arc;

use tokio::sync::mpsc;

use rosa_core::{
    agent::Agent,
    events::AgentEvent,
    provider::{ChatOptions, LlmProvider},
};
use rosa_ros2::ros2_registry_default;

const VERSION: &str = env!("CARGO_PKG_VERSION");

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    // Load .env file if present (silently ignore if absent)
    let _ = dotenvy::dotenv();

    // Logging — RUST_LOG=debug for verbose output
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_writer(io::stderr)
        .init();

    // Detect provider and build model config
    let (provider, model): (Arc<dyn LlmProvider>, String) = match detect_provider() {
        Some(p) => p,
        None => {
            eprintln!(
                "rosa: no LLM API key found.\n\
                 Set ANTHROPIC_API_KEY, XAI_API_KEY, or OPENAI_API_KEY.\n\
                 Copy .env.example → .env and fill in your key."
            );
            std::process::exit(1);
        }
    };

    // Build tool registry (ROS 2 CLI tools — honours ROS_CONTAINER env var)
    let registry = Arc::new(ros2_registry_default());

    // Build agent
    let opts = ChatOptions {
        model: model.clone(),
        max_tokens: Some(4096),
        temperature: None,
    };

    let agent = Agent::builder()
        .llm(provider)
        .tools(registry.clone())
        .system_prompt(SYSTEM_PROMPT)
        .max_iterations(20)
        .opts(opts)
        .build()
        .expect("failed to build agent");

    // REPL
    print_banner(&model);
    run_repl(&agent, registry.as_ref()).await;
}

// ---------------------------------------------------------------------------
// Provider detection
// ---------------------------------------------------------------------------

fn detect_provider() -> Option<(Arc<dyn LlmProvider>, String)> {
    // 1. Anthropic (highest priority)
    if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
        let model = std::env::var("ROSA_MODEL")
            .unwrap_or_else(|_| "claude-sonnet-4-6".to_owned());
        let p = rosa_llm::AnthropicProvider::new(key);
        return Some((Arc::new(p), model));
    }

    // 2. xAI Grok
    if let Ok(key) = std::env::var("XAI_API_KEY") {
        let model = std::env::var("ROSA_MODEL")
            .unwrap_or_else(|_| "grok-4.3".to_owned());
        let p = rosa_llm::OpenAiProvider::xai(key);
        return Some((Arc::new(p), model));
    }

    // 3. OpenAI
    if let Ok(key) = std::env::var("OPENAI_API_KEY") {
        let model = std::env::var("ROSA_MODEL")
            .unwrap_or_else(|_| "gpt-5.5".to_owned());
        let p = rosa_llm::OpenAiProvider::new(key);
        return Some((Arc::new(p), model));
    }

    None
}

// ---------------------------------------------------------------------------
// REPL
// ---------------------------------------------------------------------------

fn print_banner(model: &str) {
    println!("┌─────────────────────────────────────────────────────┐");
    println!("│  rosa v{VERSION:<45}│");
    println!("│  model: {model:<44}│");
    println!("│  /tools  /clear  /quit                              │");
    println!("└─────────────────────────────────────────────────────┘");
    if std::env::var("ROS_CONTAINER").is_ok() {
        println!("  ros2 → docker exec {}", std::env::var("ROS_CONTAINER").unwrap());
    }
    println!();
}

async fn run_repl(agent: &Agent, registry: &dyn rosa_core::agent::ToolDispatcher) {
    loop {
        // Print prompt
        print!("\x1b[32m> \x1b[0m");
        io::stdout().flush().ok();

        // Read a line
        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(0) => {
                println!("\nBye!");
                break;
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("read error: {e}");
                break;
            }
        }

        let query = input.trim();
        if query.is_empty() {
            continue;
        }
        match query {
            "/quit" | "/exit" => {
                println!("Bye!");
                break;
            }
            "/tools" => {
                print_tools(registry);
                continue;
            }
            "/clear" => {
                agent.clear_history().await;
                println!("  conversation history cleared.");
                continue;
            }
            _ => {}
        }

        // Run the agent with Ctrl-C cancellation
        run_turn(agent, query).await;
        println!(); // blank line after each turn
    }
}

async fn run_turn(agent: &Agent, query: &str) {
    let (tx, mut rx) = mpsc::unbounded_channel::<AgentEvent>();

    // Drain events concurrently with the agent loop
    let render = async {
        while let Some(event) = rx.recv().await {
            render_event(event);
        }
    };

    let run = agent.stream(query, tx);

    // race against Ctrl-C
    tokio::select! {
        // If Ctrl-C fires, just return — the agent task is dropped
        _ = tokio::signal::ctrl_c() => {
            println!("\n\x1b[33m[cancelled]\x1b[0m");
        }
        // Normal: run agent + drain events both to completion
        (run_result, ()) = async { tokio::join!(run, render) } => {
            if let Err(e) = run_result {
                eprintln!("\x1b[31m[agent error] {e}\x1b[0m");
            }
        }
    }
}

fn render_event(event: AgentEvent) {
    match event {
        AgentEvent::Token { content } => {
            print!("{content}");
            io::stdout().flush().ok();
        }
        AgentEvent::ToolStart { name, input } => {
            println!("\n\x1b[2m  ↗ {name}({input})\x1b[0m");
        }
        AgentEvent::ToolEnd { name, output } => {
            // Truncate long outputs in the terminal
            let out_str = output.to_string();
            let display = if out_str.len() > 200 {
                format!("{}…", &out_str[..200])
            } else {
                out_str
            };
            println!("\x1b[2m  ↙ {name} → {display}\x1b[0m");
        }
        AgentEvent::Final { .. } => {
            // Tokens were already printed; just ensure a trailing newline
            println!();
        }
        AgentEvent::Error { message } => {
            eprintln!("\n\x1b[31m[error] {message}\x1b[0m");
        }
    }
}

fn print_tools(registry: &dyn rosa_core::agent::ToolDispatcher) {
    let specs = registry.tool_specs();
    println!("\n  {} tools registered:", specs.len());
    for spec in specs {
        println!("  • {:30} {}", spec.name, spec.description);
    }
    println!();
}

// ---------------------------------------------------------------------------
// System prompt
// ---------------------------------------------------------------------------

const SYSTEM_PROMPT: &str = "\
You are rosa, a Rust-powered ROS 2 robot assistant.

You have access to tools to inspect and interact with a running ROS 2 system.

## Guidelines
- Use `ros2_doctor` when asked to check system health.
- Use `ros2_list_nodes` / `ros2_list_topics` to explore what is running.
- Use `ros2_topic_echo` to read sensor data or state.
- Use `ros2_node_info` to understand a node's interfaces.
- Always explain what you found and what actions you took.
- If a tool fails, report the error and suggest a fix.

## Current ROS 2 setup
- RMW: CycloneDDS
- Domain ID: 0
- Running containers: ROS 2 (Humble), optionally Isaac Sim
";
