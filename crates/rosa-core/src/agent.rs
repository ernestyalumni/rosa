use std::sync::Arc;

use futures::StreamExt;
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, info, instrument, warn};

use crate::{
    error::{Result, RosaError},
    events::AgentEvent,
    history::{History, Message, ToolCall},
    provider::{ChatChunk, ChatOptions, FinishReason, LlmProvider, ToolSpec},
};

// ---------------------------------------------------------------------------
// Tool dispatcher trait (implemented by rosa-tools::ToolRegistry)
// ---------------------------------------------------------------------------

#[async_trait::async_trait]
pub trait ToolDispatcher: Send + Sync {
    fn tool_specs(&self) -> Vec<ToolSpec>;
    async fn dispatch(
        &self,
        name: &str,
        args: serde_json::Value,
    ) -> Result<serde_json::Value>;
}

// ---------------------------------------------------------------------------
// Agent
// ---------------------------------------------------------------------------

pub struct Agent {
    llm: Arc<dyn LlmProvider>,
    tools: Arc<dyn ToolDispatcher>,
    system_prompt: String,
    max_iterations: usize,
    max_context_tokens: usize,
    opts: ChatOptions,
    /// Persistent conversation history across multiple REPL turns.
    history: Mutex<History>,
}

impl Agent {
    pub fn builder() -> AgentBuilder {
        AgentBuilder::default()
    }

    /// Clear the conversation history (used by the `/clear` REPL command).
    ///
    /// The system prompt is re-injected automatically on the next turn.
    pub async fn clear_history(&self) {
        let mut h = self.history.lock().await;
        *h = History::new();
    }

    /// Run the agent loop; collect all events and return the final answer.
    #[instrument(skip(self), fields(query = %query))]
    pub async fn invoke(&self, query: &str) -> Result<String> {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let this = self.run_loop(query, tx);
        let collect = async {
            let mut final_answer = String::new();
            while let Some(event) = rx.recv().await {
                if let AgentEvent::Final { content } = event {
                    final_answer = content;
                }
            }
            final_answer
        };
        let (result, answer) = tokio::join!(this, collect);
        result?;
        Ok(answer)
    }

    /// Stream `AgentEvent`s as they are produced.
    pub async fn stream(
        &self,
        query: &str,
        tx: mpsc::UnboundedSender<AgentEvent>,
    ) -> Result<()> {
        self.run_loop(query, tx).await
    }

    async fn run_loop(
        &self,
        query: &str,
        tx: mpsc::UnboundedSender<AgentEvent>,
    ) -> Result<()> {
        // Lock history for the duration of this turn.
        let mut history = self.history.lock().await;

        // Bootstrap the system prompt if this is the first turn (history empty).
        if history.messages().is_empty() {
            history.push(Message::System {
                content: self.system_prompt.clone(),
            });
        }
        history.push(Message::User {
            content: query.to_owned(),
        });

        let tool_specs = self.tools.tool_specs();

        for iteration in 0..self.max_iterations {
            debug!(iteration, "agent loop tick");

            if history.estimated_tokens() > self.max_context_tokens {
                return Err(RosaError::ContextTooLong(self.max_context_tokens));
            }

            let mut chunk_stream = self
                .llm
                .chat(history.messages(), &tool_specs, &self.opts)
                .await?;

            // Accumulate the full response
            let mut text_acc = String::new();
            let mut tool_call_acc: Vec<(String, String, String)> = Vec::new(); // (id, name, args)
            let mut finish_reason = FinishReason::Stop;

            while let Some(chunk_result) = chunk_stream.next().await {
                match chunk_result? {
                    ChatChunk::Token { delta } => {
                        text_acc.push_str(&delta);
                        let _ = tx.send(AgentEvent::Token { content: delta });
                    }
                    ChatChunk::ToolCall { id, name, args_delta } => {
                        // Accumulate by id
                        if let Some(entry) = tool_call_acc.iter_mut().find(|e| e.0 == id) {
                            entry.2.push_str(&args_delta);
                        } else {
                            tool_call_acc.push((id, name, args_delta));
                        }
                    }
                    ChatChunk::Finish { reason, usage } => {
                        finish_reason = reason;
                        if let Some(u) = usage {
                            info!(
                                prompt_tokens = u.prompt_tokens,
                                completion_tokens = u.completion_tokens,
                                "token usage"
                            );
                            let _ = tx.send(AgentEvent::Usage {
                                prompt_tokens: u.prompt_tokens,
                                completion_tokens: u.completion_tokens,
                            });
                        }
                    }
                }
            }

            if finish_reason == FinishReason::ToolCalls || !tool_call_acc.is_empty() {
                // Build tool calls for history
                let tool_calls: Vec<ToolCall> = tool_call_acc
                    .iter()
                    .map(|(id, name, args)| ToolCall {
                        id: id.clone(),
                        name: name.clone(),
                        arguments: serde_json::from_str(args).unwrap_or(serde_json::Value::Null),
                    })
                    .collect();

                history.push(Message::Assistant {
                    content: if text_acc.is_empty() { None } else { Some(text_acc) },
                    tool_calls: tool_calls.clone(),
                });

                // Dispatch tool calls sequentially — critical for robot control where
                // concurrent actuator commands (e.g. cmd_vel) interfere with each other.
                for tc in tool_calls {
                    let _ = tx.send(AgentEvent::ToolStart {
                        name: tc.name.clone(),
                        input: tc.arguments.clone(),
                    });
                    let result = self.tools.dispatch(&tc.name, tc.arguments.clone()).await;
                    let output = match result {
                        Ok(val) => {
                            let _ = tx.send(AgentEvent::ToolEnd {
                                name: tc.name.clone(),
                                output: val.clone(),
                            });
                            val.to_string()
                        }
                        Err(e) => {
                            warn!(tool = %tc.name, error = %e, "tool error");
                            let _ = tx.send(AgentEvent::ToolEnd {
                                name: tc.name.clone(),
                                output: serde_json::json!({ "error": e.to_string() }),
                            });
                            format!("{{\"error\": \"{e}\"}}")
                        }
                    };
                    history.push(Message::Tool { call_id: tc.id, content: output });
                }

                continue; // next iteration
            }

            // No tool calls → final answer
            let _ = tx.send(AgentEvent::Final {
                content: text_acc.clone(),
            });
            history.push(Message::Assistant {
                content: Some(text_acc),
                tool_calls: vec![],
            });
            return Ok(());
        }

        Err(RosaError::MaxIterationsReached(self.max_iterations))
    }
}

// ---------------------------------------------------------------------------
// Builder
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct AgentBuilder {
    llm: Option<Arc<dyn LlmProvider>>,
    tools: Option<Arc<dyn ToolDispatcher>>,
    system_prompt: Option<String>,
    max_iterations: Option<usize>,
    max_context_tokens: Option<usize>,
    opts: Option<ChatOptions>,
}

impl AgentBuilder {
    pub fn llm(mut self, provider: Arc<dyn LlmProvider>) -> Self {
        self.llm = Some(provider);
        self
    }

    pub fn tools(mut self, dispatcher: Arc<dyn ToolDispatcher>) -> Self {
        self.tools = Some(dispatcher);
        self
    }

    pub fn system_prompt(mut self, s: impl Into<String>) -> Self {
        self.system_prompt = Some(s.into());
        self
    }

    pub fn max_iterations(mut self, n: usize) -> Self {
        self.max_iterations = Some(n);
        self
    }

    pub fn max_context_tokens(mut self, n: usize) -> Self {
        self.max_context_tokens = Some(n);
        self
    }

    pub fn opts(mut self, opts: ChatOptions) -> Self {
        self.opts = Some(opts);
        self
    }

    pub fn build(self) -> Result<Agent> {
        Ok(Agent {
            llm: self.llm.ok_or_else(|| RosaError::Other("llm provider required".into()))?,
            tools: self.tools.ok_or_else(|| RosaError::Other("tool dispatcher required".into()))?,
            system_prompt: self.system_prompt.unwrap_or_default(),
            max_iterations: self.max_iterations.unwrap_or(100),
            max_context_tokens: self.max_context_tokens.unwrap_or(120_000),
            opts: self.opts.unwrap_or_default(),
            history: Mutex::new(History::new()),
        })
    }
}
