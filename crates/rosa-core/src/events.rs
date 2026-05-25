/// Events emitted by the agent loop as it runs.
/// Mirrors upstream rosa's event types so the turtle demo render maps 1:1.
#[derive(Debug, Clone)]
pub enum AgentEvent {
    /// A text token streamed from the LLM.
    Token { content: String },

    /// A tool call is about to be dispatched.
    ToolStart {
        name: String,
        input: serde_json::Value,
    },

    /// A tool call completed.
    ToolEnd {
        name: String,
        output: serde_json::Value,
    },

    /// The agent produced a final answer (no more tool calls).
    Final { content: String },

    /// An unrecoverable error occurred; the loop will exit.
    Error { message: String },
}
