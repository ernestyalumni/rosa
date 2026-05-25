//! LLM provider adapters for rosa.
//!
//! Implements [`rosa_core::provider::LlmProvider`] for:
//!
//! | Adapter              | Module          | Endpoint                         |
//! |----------------------|-----------------|----------------------------------|
//! | [`AnthropicProvider`]| `anthropic`     | `https://api.anthropic.com`      |
//! | [`OpenAiProvider`]   | `openai`        | `https://api.openai.com` / Ollama|
//!
//! All adapters are hand-rolled on **reqwest + SSE** — no vendor SDK is used.
//! Ollama is supported via `OpenAiProvider::ollama()` (OpenAI-compat `/v1` endpoint).
//!
//! ## Quick start
//!
//! ```no_run
//! use rosa_llm::AnthropicProvider;
//! use rosa_core::provider::{ChatOptions, LlmProvider};
//!
//! let llm  = AnthropicProvider::new(std::env::var("ANTHROPIC_API_KEY").unwrap());
//! let opts = ChatOptions::default(); // claude-sonnet-4-6, 4096 tokens
//! ```

mod common;

pub mod anthropic;
pub mod openai;

pub use anthropic::AnthropicProvider;
pub use openai::OpenAiProvider;
