//! Trait definitions for LLM providers and tool specs.
//! Concrete adapters live in `rosa-llm`; concrete tools in `rosa-tools`.
//! Defined here so `rosa-core` (no I/O) can compile standalone.

use async_trait::async_trait;
use futures::stream::BoxStream;
use serde::{Deserialize, Serialize};

use crate::history::Message;
use crate::error::Result;

// ---------------------------------------------------------------------------
// Tool spec (provider-agnostic description used by the agent loop)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    /// JSON Schema for the tool's input arguments.
    pub parameters: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Chat chunk — what streams back from the LLM
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum ChatChunk {
    /// A text delta.
    Token { delta: String },
    /// A tool call delta (accumulate args_delta until Finish).
    ToolCall {
        id: String,
        name: String,
        args_delta: String,
    },
    /// The stream is done.
    Finish {
        reason: FinishReason,
        usage: Option<Usage>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum FinishReason {
    Stop,
    ToolCalls,
    MaxTokens,
    Other(String),
}

#[derive(Debug, Clone, Default)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
}

// ---------------------------------------------------------------------------
// Chat options
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct ChatOptions {
    pub model: String,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
}

impl Default for ChatOptions {
    fn default() -> Self {
        Self {
            model: "claude-sonnet-4-6".into(),
            max_tokens: Some(4096),
            temperature: None,
        }
    }
}

// ---------------------------------------------------------------------------
// LlmProvider trait
// ---------------------------------------------------------------------------

#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Send `messages` with `tools` available and stream back `ChatChunk`s.
    async fn chat(
        &self,
        messages: &[Message],
        tools: &[ToolSpec],
        opts: &ChatOptions,
    ) -> Result<BoxStream<'static, Result<ChatChunk>>>;
}
