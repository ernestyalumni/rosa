use serde::{Deserialize, Serialize};

/// A single tool call as returned by the LLM.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

/// The message types that flow through the agent loop.
/// Mirrors OpenAI / Anthropic message conventions so adapters map cleanly.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "snake_case")]
pub enum Message {
    System {
        content: String,
    },
    User {
        content: String,
    },
    Assistant {
        /// Text content (may be None when the turn is pure tool calls)
        content: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        tool_calls: Vec<ToolCall>,
    },
    Tool {
        /// Matches the ToolCall.id this result corresponds to
        call_id: String,
        content: String,
    },
}

/// Ordered conversation history.
#[derive(Clone, Debug, Default)]
pub struct History {
    messages: Vec<Message>,
}

impl History {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, msg: Message) {
        self.messages.push(msg);
    }

    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    pub fn clear(&mut self) {
        self.messages.clear();
    }

    pub fn len(&self) -> usize {
        self.messages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// Rough token estimate: 4 chars ≈ 1 token (good enough for guard rails).
    pub fn estimated_tokens(&self) -> usize {
        self.messages
            .iter()
            .map(|m| serde_json::to_string(m).unwrap_or_default().len() / 4)
            .sum()
    }
}
