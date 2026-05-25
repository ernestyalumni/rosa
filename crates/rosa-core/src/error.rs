use thiserror::Error;

#[derive(Debug, Error)]
pub enum RosaError {
    #[error("LLM provider error: {0}")]
    Provider(String),

    #[error("Tool not found: {name}")]
    ToolNotFound { name: String },

    #[error("Tool argument error for '{name}': {source}")]
    ToolBadArgs {
        name: String,
        #[source]
        source: serde_json::Error,
    },

    #[error("Tool execution error in '{name}': {message}")]
    ToolExecution { name: String, message: String },

    #[error("Agent reached maximum iterations ({0}) without a final answer")]
    MaxIterationsReached(usize),

    #[error("Context window exceeded max tokens ({0})")]
    ContextTooLong(usize),

    #[error("Cancelled")]
    Cancelled,

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, RosaError>;
