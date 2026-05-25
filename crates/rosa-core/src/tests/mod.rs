use std::sync::Arc;

use async_trait::async_trait;
use futures::stream::{self, BoxStream};
use tokio::sync::mpsc;

use crate::{
    agent::{Agent, ToolDispatcher},
    error::{Result, RosaError},
    events::AgentEvent,
    history::Message,
    provider::{ChatChunk, ChatOptions, FinishReason, LlmProvider, ToolSpec},
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// A mock LLM that returns a configurable sequence of chunks.
struct MockLlm {
    chunks: Vec<ChatChunk>,
}

#[async_trait]
impl LlmProvider for MockLlm {
    async fn chat(
        &self,
        _messages: &[Message],
        _tools: &[ToolSpec],
        _opts: &ChatOptions,
    ) -> Result<BoxStream<'static, Result<ChatChunk>>> {
        let chunks: Vec<Result<ChatChunk>> = self.chunks.iter().cloned().map(Ok).collect();
        Ok(Box::pin(stream::iter(chunks)))
    }
}

/// A mock tool dispatcher with no tools registered.
struct NoTools;

#[async_trait]
impl ToolDispatcher for NoTools {
    fn tool_specs(&self) -> Vec<ToolSpec> {
        vec![]
    }

    async fn dispatch(&self, name: &str, _args: serde_json::Value) -> Result<serde_json::Value> {
        Err(RosaError::ToolNotFound { name: name.to_owned() })
    }
}

/// A mock tool dispatcher that returns a fixed result for one tool.
struct EchoTool;

#[async_trait]
impl ToolDispatcher for EchoTool {
    fn tool_specs(&self) -> Vec<ToolSpec> {
        vec![ToolSpec {
            name: "echo".into(),
            description: "Echoes the input".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": { "text": { "type": "string" } }
            }),
        }]
    }

    async fn dispatch(&self, _name: &str, args: serde_json::Value) -> Result<serde_json::Value> {
        Ok(args)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_history_push_and_len() {
    use crate::history::History;
    let mut h = History::new();
    assert!(h.is_empty());
    h.push(Message::User { content: "hello".into() });
    assert_eq!(h.len(), 1);
}

#[tokio::test]
async fn test_agent_final_answer_no_tools() {
    let llm = Arc::new(MockLlm {
        chunks: vec![
            ChatChunk::Token { delta: "Hello".into() },
            ChatChunk::Token { delta: " world".into() },
            ChatChunk::Finish { reason: FinishReason::Stop, usage: None },
        ],
    });

    let agent = Agent::builder()
        .llm(llm)
        .tools(Arc::new(NoTools))
        .system_prompt("You are helpful.")
        .build()
        .unwrap();

    let answer = agent.invoke("hi").await.unwrap();
    assert_eq!(answer, "Hello world");
}

#[tokio::test]
async fn test_agent_tool_call_then_final() {
    // First turn: LLM returns a tool call.
    // Second turn: LLM returns a final answer.
    use std::sync::Mutex;

    struct TwoTurnLlm {
        call_count: Mutex<usize>,
    }

    #[async_trait]
    impl LlmProvider for TwoTurnLlm {
        async fn chat(
            &self,
            _messages: &[Message],
            _tools: &[ToolSpec],
            _opts: &ChatOptions,
        ) -> Result<BoxStream<'static, Result<ChatChunk>>> {
            let mut count = self.call_count.lock().unwrap();
            *count += 1;
            let turn = *count;
            drop(count);

            if turn == 1 {
                // Return a tool call
                let chunks = vec![
                    Ok(ChatChunk::ToolCall {
                        id: "call_1".into(),
                        name: "echo".into(),
                        args_delta: r#"{"text":"ping"}"#.into(),
                    }),
                    Ok(ChatChunk::Finish {
                        reason: FinishReason::ToolCalls,
                        usage: None,
                    }),
                ];
                Ok(Box::pin(stream::iter(chunks)))
            } else {
                // Return final answer
                let chunks = vec![
                    Ok(ChatChunk::Token { delta: "pong".into() }),
                    Ok(ChatChunk::Finish { reason: FinishReason::Stop, usage: None }),
                ];
                Ok(Box::pin(stream::iter(chunks)))
            }
        }
    }

    let agent = Agent::builder()
        .llm(Arc::new(TwoTurnLlm { call_count: Mutex::new(0) }))
        .tools(Arc::new(EchoTool))
        .system_prompt("Test.")
        .build()
        .unwrap();

    let answer = agent.invoke("test").await.unwrap();
    assert_eq!(answer, "pong");
}

#[tokio::test]
async fn test_max_iterations_cap() {
    // LLM always returns a tool call → should hit max_iterations
    struct AlwaysToolCall;

    #[async_trait]
    impl LlmProvider for AlwaysToolCall {
        async fn chat(
            &self,
            _messages: &[Message],
            _tools: &[ToolSpec],
            _opts: &ChatOptions,
        ) -> Result<BoxStream<'static, Result<ChatChunk>>> {
            let chunks = vec![
                Ok(ChatChunk::ToolCall {
                    id: "c".into(),
                    name: "echo".into(),
                    args_delta: r#"{}"#.into(),
                }),
                Ok(ChatChunk::Finish {
                    reason: FinishReason::ToolCalls,
                    usage: None,
                }),
            ];
            Ok(Box::pin(stream::iter(chunks)))
        }
    }

    let agent = Agent::builder()
        .llm(Arc::new(AlwaysToolCall))
        .tools(Arc::new(EchoTool))
        .max_iterations(3)
        .build()
        .unwrap();

    let err = agent.invoke("loop forever").await.unwrap_err();
    assert!(matches!(err, RosaError::MaxIterationsReached(3)));
}

#[tokio::test]
async fn test_event_stream() {
    let llm = Arc::new(MockLlm {
        chunks: vec![
            ChatChunk::Token { delta: "hi".into() },
            ChatChunk::Finish { reason: FinishReason::Stop, usage: None },
        ],
    });

    let agent = Agent::builder()
        .llm(llm)
        .tools(Arc::new(NoTools))
        .build()
        .unwrap();

    let (tx, mut rx) = mpsc::unbounded_channel();
    agent.stream("hello", tx).await.unwrap();

    let mut got_token = false;
    let mut got_final = false;
    while let Ok(event) = rx.try_recv() {
        match event {
            AgentEvent::Token { .. } => got_token = true,
            AgentEvent::Final { .. } => got_final = true,
            _ => {}
        }
    }
    assert!(got_token);
    assert!(got_final);
}
