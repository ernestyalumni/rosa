//! OpenAI-compatible provider adapter.
//!
//! Works for: **OpenAI API**, **Ollama** (`/v1` endpoint), and any
//! OpenAI-compatible proxy/gateway.
//!
//! Reference: <https://platform.openai.com/docs/api-reference/streaming>
//!
//! ## SSE event → ChatChunk mapping
//!
//! | OpenAI SSE data field              | Emits                        |
//! |------------------------------------|------------------------------|
//! | `choices[].delta.content`          | `Token { delta }`            |
//! | `choices[].delta.tool_calls[].function.arguments` | `ToolCall { … }` |
//! | `choices[].finish_reason` (non-null) | `Finish { reason, usage }` |
//! | `data: [DONE]`                     | *(stream end, no emit)*      |

use std::collections::HashMap;

use async_stream::stream;
use async_trait::async_trait;
use futures::stream::{BoxStream, StreamExt};
use serde_json::{json, Value};

use rosa_core::{
    history::Message,
    provider::{ChatChunk, ChatOptions, FinishReason, LlmProvider, ToolSpec, Usage},
    error::{Result, RosaError},
};

use crate::common::sse::sse_events;

const OPENAI_BASE_URL: &str  = "https://api.openai.com";
const OLLAMA_BASE_URL: &str  = "http://localhost:11434";

// ---------------------------------------------------------------------------
// Provider struct
// ---------------------------------------------------------------------------

/// HTTP adapter for any OpenAI-compatible chat completions endpoint.
///
/// ```no_run
/// // OpenAI
/// use rosa_llm::OpenAiProvider;
/// let openai  = OpenAiProvider::new(std::env::var("OPENAI_API_KEY").unwrap());
///
/// // Ollama (no key required)
/// let ollama  = OpenAiProvider::ollama();
///
/// // Custom endpoint
/// let custom  = OpenAiProvider::new("key").with_base_url("http://localhost:8080");
/// ```
pub struct OpenAiProvider {
    client: reqwest::Client,
    api_key: String,
    base_url: String,
}

impl OpenAiProvider {
    /// Connect to the real OpenAI API (`https://api.openai.com`).
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key: api_key.into(),
            base_url: OPENAI_BASE_URL.into(),
        }
    }

    /// Connect to a local Ollama instance (`http://localhost:11434`).
    /// No API key is required.
    pub fn ollama() -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key: String::new(),
            base_url: OLLAMA_BASE_URL.into(),
        }
    }

    /// Override the base URL for proxies or alternative endpoints.
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    // -----------------------------------------------------------------------
    // Message / tool serialisation
    // -----------------------------------------------------------------------

    fn to_openai_messages(messages: &[Message]) -> Vec<Value> {
        messages
            .iter()
            .map(|msg| match msg {
                Message::System { content } => {
                    json!({ "role": "system", "content": content })
                }
                Message::User { content } => {
                    json!({ "role": "user", "content": content })
                }
                Message::Assistant { content, tool_calls } => {
                    let mut obj = json!({
                        "role":    "assistant",
                        "content": content,
                    });
                    if !tool_calls.is_empty() {
                        let tc_arr: Vec<Value> = tool_calls
                            .iter()
                            .map(|tc| json!({
                                "id":   tc.id,
                                "type": "function",
                                "function": {
                                    "name":      tc.name,
                                    // Our ToolCall stores parsed JSON; OpenAI wants a string
                                    "arguments": tc.arguments.to_string(),
                                },
                            }))
                            .collect();
                        obj["tool_calls"] = json!(tc_arr);
                    }
                    obj
                }
                Message::Tool { call_id, content } => {
                    json!({
                        "role":         "tool",
                        "tool_call_id": call_id,
                        "content":      content,
                    })
                }
            })
            .collect()
    }

    fn to_openai_tools(tools: &[ToolSpec]) -> Vec<Value> {
        tools
            .iter()
            .map(|t| json!({
                "type": "function",
                "function": {
                    "name":        t.name,
                    "description": t.description,
                    "parameters":  t.parameters,
                },
            }))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// LlmProvider impl
// ---------------------------------------------------------------------------

#[async_trait]
impl LlmProvider for OpenAiProvider {
    async fn chat(
        &self,
        messages: &[Message],
        tools: &[ToolSpec],
        opts: &ChatOptions,
    ) -> Result<BoxStream<'static, Result<ChatChunk>>> {
        // --- Build request body ------------------------------------------------
        let openai_messages = Self::to_openai_messages(messages);
        let openai_tools    = Self::to_openai_tools(tools);

        let mut body = json!({
            "model":          opts.model,
            "stream":         true,
            // Ask the API to include usage in the final SSE chunk
            "stream_options": { "include_usage": true },
            "messages":       openai_messages,
        });
        if let Some(max_tokens) = opts.max_tokens {
            body["max_tokens"] = json!(max_tokens);
        }
        if let Some(temp) = opts.temperature {
            body["temperature"] = json!(temp);
        }
        if !openai_tools.is_empty() {
            body["tools"]       = json!(openai_tools);
            body["tool_choice"] = json!("auto");
        }

        tracing::debug!(model = %opts.model, base_url = %self.base_url, "OpenAI chat request");

        // --- Send request -------------------------------------------------------
        let url = format!("{}/v1/chat/completions", self.base_url);
        let mut req = self.client.post(&url).json(&body);
        if !self.api_key.is_empty() {
            req = req.bearer_auth(&self.api_key);
        }
        let response = req
            .send()
            .await
            .map_err(|e| RosaError::Provider(format!("OpenAI request failed: {e}")))?;

        if !response.status().is_success() {
            let status   = response.status();
            let err_body = response.text().await.unwrap_or_default();
            return Err(RosaError::Provider(format!(
                "OpenAI API {status}: {err_body}"
            )));
        }

        // --- Parse SSE stream ---------------------------------------------------
        let events = sse_events(response.bytes_stream());

        let chunk_stream = stream! {
            // Accumulate tool call state by index across delta chunks.
            // OpenAI sends id + name only in the first delta for that index.
            // index → (call_id, function_name)
            let mut tool_calls: HashMap<u32, (String, String)> = HashMap::new();
            let mut last_usage: Option<Usage> = None;

            let mut events = Box::pin(events);

            'stream: while let Some((_ev_type, data)) = events.next().await {
                // OpenAI uses the default event type; ev_type is always "message"

                // End-of-stream sentinel
                if data.trim() == "[DONE]" {
                    break 'stream;
                }

                let Ok(v) = serde_json::from_str::<Value>(&data) else { continue };

                // Usage arrives in a separate chunk with empty choices array
                // (sent after [DONE] by some providers — capture it here)
                if let Some(u) = v.get("usage").filter(|u| !u.is_null()) {
                    last_usage = Some(Usage {
                        prompt_tokens:     u["prompt_tokens"]    .as_u64().unwrap_or(0) as u32,
                        completion_tokens: u["completion_tokens"].as_u64().unwrap_or(0) as u32,
                    });
                }

                let choices = match v["choices"].as_array() {
                    Some(c) if !c.is_empty() => c.clone(),
                    _ => continue,
                };

                for choice in &choices {
                    let delta         = &choice["delta"];
                    let finish_reason = choice["finish_reason"].as_str();

                    // ── Text token ──────────────────────────────────────────
                    if let Some(text) = delta["content"].as_str() {
                        if !text.is_empty() {
                            yield Ok(ChatChunk::Token { delta: text.to_owned() });
                        }
                    }

                    // ── Tool call deltas ────────────────────────────────────
                    if let Some(tc_arr) = delta["tool_calls"].as_array() {
                        for tc in tc_arr {
                            let idx = tc["index"].as_u64().unwrap_or(0) as u32;

                            // First delta for this index carries id + function.name
                            if let Some(id) = tc["id"].as_str() {
                                let name = tc["function"]["name"]
                                    .as_str()
                                    .unwrap_or("")
                                    .to_owned();
                                tool_calls.insert(idx, (id.to_owned(), name));
                            }

                            // Arguments arrive on first or subsequent deltas
                            let args_delta = tc["function"]["arguments"]
                                .as_str()
                                .unwrap_or("")
                                .to_owned();
                            if !args_delta.is_empty() {
                                if let Some((id, name)) = tool_calls.get(&idx) {
                                    yield Ok(ChatChunk::ToolCall {
                                        id:         id.clone(),
                                        name:       name.clone(),
                                        args_delta,
                                    });
                                }
                            }
                        }
                    }

                    // ── Finish ──────────────────────────────────────────────
                    if let Some(reason_str) = finish_reason {
                        let reason = match reason_str {
                            "stop"       => FinishReason::Stop,
                            "tool_calls" => FinishReason::ToolCalls,
                            "length"     => FinishReason::MaxTokens,
                            other        => FinishReason::Other(other.to_owned()),
                        };
                        yield Ok(ChatChunk::Finish {
                            reason,
                            usage: last_usage.clone(),
                        });
                    }
                }
            }
        };

        Ok(Box::pin(chunk_stream))
    }
}

// ---------------------------------------------------------------------------
// Tests — message/tool serialisation (no HTTP required)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::OpenAiProvider;
    use rosa_core::{
        history::{Message, ToolCall},
        provider::ToolSpec,
    };
    use serde_json::json;

    fn tool_spec(name: &str) -> ToolSpec {
        ToolSpec {
            name: name.to_owned(),
            description: "a tool".to_owned(),
            parameters: json!({ "type": "object", "properties": {} }),
        }
    }

    // ── to_openai_messages ───────────────────────────────────────────────────

    #[test]
    fn test_system_stays_in_messages_array() {
        // Unlike Anthropic, OpenAI keeps system in the messages array
        let msgs = OpenAiProvider::to_openai_messages(&[
            Message::System { content: "be concise".to_owned() },
        ]);
        assert_eq!(msgs[0]["role"],    json!("system"));
        assert_eq!(msgs[0]["content"], json!("be concise"));
    }

    #[test]
    fn test_user_message_serialized() {
        let msgs = OpenAiProvider::to_openai_messages(&[
            Message::User { content: "hello".to_owned() },
        ]);
        assert_eq!(msgs[0]["role"],    json!("user"));
        assert_eq!(msgs[0]["content"], json!("hello"));
    }

    #[test]
    fn test_tool_result_role_and_tool_call_id() {
        // OpenAI tool results use role "tool" with tool_call_id (not tool_use_id)
        let msgs = OpenAiProvider::to_openai_messages(&[
            Message::Tool { call_id: "call_abc".to_owned(), content: "42".to_owned() },
        ]);
        assert_eq!(msgs[0]["role"],         json!("tool"));
        assert_eq!(msgs[0]["tool_call_id"], json!("call_abc"));
        assert_eq!(msgs[0]["content"],      json!("42"));
    }

    #[test]
    fn test_assistant_with_tool_calls_includes_tool_calls_array() {
        let tc = ToolCall {
            id: "tc1".to_owned(),
            name: "search".to_owned(),
            arguments: json!({ "q": "rust" }),
        };
        let msgs = OpenAiProvider::to_openai_messages(&[
            Message::Assistant { content: None, tool_calls: vec![tc] },
        ]);
        let tc_arr = msgs[0]["tool_calls"].as_array().unwrap();
        assert_eq!(tc_arr[0]["id"],               json!("tc1"));
        assert_eq!(tc_arr[0]["type"],             json!("function"));
        assert_eq!(tc_arr[0]["function"]["name"], json!("search"));
    }

    #[test]
    fn test_assistant_tool_call_arguments_serialized_as_json_string() {
        // OpenAI API contract: arguments must be a JSON-encoded *string*,
        // not a nested object. Wrong encoding causes tool call failures.
        let tc = ToolCall {
            id: "tc2".to_owned(),
            name: "add".to_owned(),
            arguments: json!({ "a": 1, "b": 2 }),
        };
        let msgs = OpenAiProvider::to_openai_messages(&[
            Message::Assistant { content: None, tool_calls: vec![tc] },
        ]);
        let args = &msgs[0]["tool_calls"][0]["function"]["arguments"];
        let args_str = args.as_str().expect("arguments must be a JSON string");

        // Round-trip: the string should deserialise back to the original object
        let parsed: serde_json::Value = serde_json::from_str(args_str).unwrap();
        assert_eq!(parsed["a"], json!(1));
        assert_eq!(parsed["b"], json!(2));
    }

    #[test]
    fn test_assistant_no_tool_calls_omits_tool_calls_field() {
        // When there are no tool calls, the key must be absent (not null, not [])
        // to avoid confusing strict OpenAI parsers.
        let msgs = OpenAiProvider::to_openai_messages(&[
            Message::Assistant { content: Some("plain text".to_owned()), tool_calls: vec![] },
        ]);
        assert!(
            msgs[0].get("tool_calls").is_none(),
            "tool_calls key must be absent when there are no tool calls"
        );
    }

    #[test]
    fn test_message_order_preserved() {
        let input = vec![
            Message::System  { content: "sys".to_owned() },
            Message::User    { content: "q".to_owned() },
            Message::Assistant { content: Some("a".to_owned()), tool_calls: vec![] },
        ];
        let msgs = OpenAiProvider::to_openai_messages(&input);
        assert_eq!(msgs[0]["role"], json!("system"));
        assert_eq!(msgs[1]["role"], json!("user"));
        assert_eq!(msgs[2]["role"], json!("assistant"));
    }

    // ── to_openai_tools ──────────────────────────────────────────────────────

    #[test]
    fn test_tools_use_parameters_not_input_schema() {
        // OpenAI uses "parameters"; Anthropic uses "input_schema". Wrong key
        // causes the API to ignore the schema and accept any input.
        let tools = OpenAiProvider::to_openai_tools(&[tool_spec("echo")]);
        assert_eq!(tools[0]["type"],                    json!("function"));
        assert_eq!(tools[0]["function"]["name"],        json!("echo"));
        assert_eq!(tools[0]["function"]["description"], json!("a tool"));
        assert!(tools[0]["function"]["parameters"].is_object(), "must have parameters");
        assert!(
            tools[0]["function"].get("input_schema").is_none(),
            "must NOT have input_schema key"
        );
    }

    #[test]
    fn test_tools_empty_yields_empty_vec() {
        assert!(OpenAiProvider::to_openai_tools(&[]).is_empty());
    }

    #[test]
    fn test_multiple_tools_preserve_order() {
        let specs = vec![tool_spec("alpha"), tool_spec("beta")];
        let tools = OpenAiProvider::to_openai_tools(&specs);
        assert_eq!(tools[0]["function"]["name"], json!("alpha"));
        assert_eq!(tools[1]["function"]["name"], json!("beta"));
    }
}
