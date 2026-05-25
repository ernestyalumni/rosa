//! Anthropic Claude provider adapter.
//!
//! Converts rosa's provider-agnostic types to Anthropic's Messages API format
//! and parses the SSE stream back into `ChatChunk`s.
//!
//! Reference: <https://docs.anthropic.com/en/api/messages-streaming>
//!
//! ## SSE event → ChatChunk mapping
//!
//! | Anthropic SSE event       | Emits                          |
//! |---------------------------|--------------------------------|
//! | `content_block_start`     | *(records tool_use id/name)*   |
//! | `content_block_delta` (text_delta)       | `Token { delta }`  |
//! | `content_block_delta` (input_json_delta) | `ToolCall { … }`   |
//! | `message_delta`           | `Finish { reason, usage }`     |
//! | everything else           | *(ignored)*                    |

use std::collections::HashMap;

use async_stream::stream;
use async_trait::async_trait;
use futures::stream::{BoxStream, StreamExt};
use serde_json::{json, Value};
use tracing::warn;

use rosa_core::{
    history::Message,
    provider::{ChatChunk, ChatOptions, FinishReason, LlmProvider, ToolSpec, Usage},
    error::{Result, RosaError},
};

use crate::common::sse::sse_events;

const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";
const ANTHROPIC_VERSION: &str = "2023-06-01";

// ---------------------------------------------------------------------------
// Provider struct
// ---------------------------------------------------------------------------

/// HTTP adapter for the Anthropic Claude API.
///
/// ```no_run
/// use rosa_llm::AnthropicProvider;
/// use rosa_core::provider::{ChatOptions, LlmProvider};
///
/// let provider = AnthropicProvider::new(std::env::var("ANTHROPIC_API_KEY").unwrap());
/// ```
pub struct AnthropicProvider {
    client: reqwest::Client,
    api_key: String,
    base_url: String,
}

impl AnthropicProvider {
    /// Construct a provider pointed at the real Anthropic API.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key: api_key.into(),
            base_url: DEFAULT_BASE_URL.into(),
        }
    }

    /// Override the base URL (useful for proxies or unit-test mock servers).
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    // -----------------------------------------------------------------------
    // Message / tool serialisation
    // -----------------------------------------------------------------------

    /// Split our `Message` slice into an optional system string (Anthropic
    /// takes this as a top-level field) and the messages array.
    fn to_anthropic_messages(messages: &[Message]) -> (Option<String>, Vec<Value>) {
        let mut system: Option<String> = None;
        let mut out: Vec<Value> = Vec::new();

        for msg in messages {
            match msg {
                Message::System { content } => {
                    system = Some(content.clone());
                }
                Message::User { content } => {
                    out.push(json!({ "role": "user", "content": content }));
                }
                Message::Assistant { content, tool_calls } => {
                    let mut blocks: Vec<Value> = Vec::new();
                    if let Some(text) = content {
                        if !text.is_empty() {
                            blocks.push(json!({ "type": "text", "text": text }));
                        }
                    }
                    for tc in tool_calls {
                        blocks.push(json!({
                            "type":  "tool_use",
                            "id":    tc.id,
                            "name":  tc.name,
                            "input": tc.arguments,
                        }));
                    }
                    // Anthropic requires at least one block
                    if blocks.is_empty() {
                        blocks.push(json!({ "type": "text", "text": "" }));
                    }
                    out.push(json!({ "role": "assistant", "content": blocks }));
                }
                Message::Tool { call_id, content } => {
                    // Tool results are sent as a user message with tool_result blocks
                    out.push(json!({
                        "role": "user",
                        "content": [{
                            "type":        "tool_result",
                            "tool_use_id": call_id,
                            "content":     content,
                        }],
                    }));
                }
            }
        }

        (system, out)
    }

    /// Convert our provider-agnostic `ToolSpec`s into Anthropic's tool schema.
    fn to_anthropic_tools(tools: &[ToolSpec]) -> Vec<Value> {
        tools
            .iter()
            .map(|t| json!({
                "name":         t.name,
                "description":  t.description,
                "input_schema": t.parameters,
            }))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// LlmProvider impl
// ---------------------------------------------------------------------------

#[async_trait]
impl LlmProvider for AnthropicProvider {
    async fn chat(
        &self,
        messages: &[Message],
        tools: &[ToolSpec],
        opts: &ChatOptions,
    ) -> Result<BoxStream<'static, Result<ChatChunk>>> {
        // --- Build request body ------------------------------------------------
        let (system, anthropic_messages) = Self::to_anthropic_messages(messages);
        let anthropic_tools = Self::to_anthropic_tools(tools);

        let mut body = json!({
            "model":      opts.model,
            "max_tokens": opts.max_tokens.unwrap_or(4096),
            "stream":     true,
            "messages":   anthropic_messages,
        });
        if let Some(sys) = system {
            body["system"] = json!(sys);
        }
        if let Some(temp) = opts.temperature {
            body["temperature"] = json!(temp);
        }
        if !anthropic_tools.is_empty() {
            body["tools"] = json!(anthropic_tools);
        }

        tracing::debug!(model = %opts.model, "Anthropic chat request");

        // --- Send request -------------------------------------------------------
        let url = format!("{}/v1/messages", self.base_url);
        let response = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| RosaError::Provider(format!("Anthropic request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let err_body = response.text().await.unwrap_or_default();
            return Err(RosaError::Provider(format!(
                "Anthropic API {status}: {err_body}"
            )));
        }

        // --- Parse SSE stream ---------------------------------------------------
        let events = sse_events(response.bytes_stream());

        // `stream!` lets us hold mutable state (tool_blocks map, input_token count)
        // across yield points without `Arc<Mutex<…>>`.
        let chunk_stream = stream! {
            // content_block index → (tool_use_id, tool_name)
            let mut tool_blocks: HashMap<u32, (String, String)> = HashMap::new();
            // input_tokens comes from message_start, not message_delta
            let mut input_tokens: u32 = 0;

            let mut events = Box::pin(events);

            while let Some((ev_type, data)) = events.next().await {
                match ev_type.as_str() {
                    // Capture input token count for the final Usage struct
                    "message_start" => {
                        if let Ok(v) = serde_json::from_str::<Value>(&data) {
                            input_tokens = v["message"]["usage"]["input_tokens"]
                                .as_u64()
                                .unwrap_or(0) as u32;
                        }
                    }

                    // Record tool_use block metadata so we can label later deltas
                    "content_block_start" => {
                        let Ok(v) = serde_json::from_str::<Value>(&data) else { continue };
                        let idx = v["index"].as_u64().unwrap_or(0) as u32;
                        if v["content_block"]["type"].as_str() == Some("tool_use") {
                            let id   = v["content_block"]["id"]  .as_str().unwrap_or("").to_owned();
                            let name = v["content_block"]["name"].as_str().unwrap_or("").to_owned();
                            tool_blocks.insert(idx, (id, name));
                        }
                    }

                    // Incremental content — text token or tool arg fragment
                    "content_block_delta" => {
                        let Ok(v) = serde_json::from_str::<Value>(&data) else { continue };
                        let idx = v["index"].as_u64().unwrap_or(0) as u32;

                        match v["delta"]["type"].as_str() {
                            Some("text_delta") => {
                                let text = v["delta"]["text"].as_str().unwrap_or("").to_owned();
                                if !text.is_empty() {
                                    yield Ok(ChatChunk::Token { delta: text });
                                }
                            }
                            Some("input_json_delta") => {
                                let partial = v["delta"]["partial_json"]
                                    .as_str()
                                    .unwrap_or("")
                                    .to_owned();
                                if let Some((id, name)) = tool_blocks.get(&idx) {
                                    yield Ok(ChatChunk::ToolCall {
                                        id:         id.clone(),
                                        name:       name.clone(),
                                        args_delta: partial,
                                    });
                                }
                            }
                            other => {
                                warn!("Unknown Anthropic delta type: {:?}", other);
                            }
                        }
                    }

                    // Message complete — emit Finish with accumulated usage
                    "message_delta" => {
                        let Ok(v) = serde_json::from_str::<Value>(&data) else { continue };
                        let reason = match v["delta"]["stop_reason"].as_str() {
                            Some("end_turn")   => FinishReason::Stop,
                            Some("tool_use")   => FinishReason::ToolCalls,
                            Some("max_tokens") => FinishReason::MaxTokens,
                            Some(r)            => FinishReason::Other(r.to_owned()),
                            None               => FinishReason::Stop,
                        };
                        let completion_tokens = v["usage"]["output_tokens"]
                            .as_u64()
                            .unwrap_or(0) as u32;
                        let usage = Some(Usage {
                            prompt_tokens: input_tokens,
                            completion_tokens,
                        });
                        yield Ok(ChatChunk::Finish { reason, usage });
                    }

                    // Ignore: content_block_stop, message_stop, ping
                    _ => {}
                }
            }
        };

        Ok(Box::pin(chunk_stream))
    }
}
