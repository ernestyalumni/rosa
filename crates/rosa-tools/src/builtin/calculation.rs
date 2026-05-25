//! `add` — add two floating-point numbers.
//!
//! This is the simplest possible tool and also serves as the unit-test fixture
//! for the `Tool` trait and `ToolRegistry`.

use async_trait::async_trait;
use schemars::{schema_for, schema::RootSchema, JsonSchema};
use serde::Deserialize;
use serde_json::{json, Value};

use rosa_core::error::Result;

use crate::tool::Tool;

// ---------------------------------------------------------------------------
// Arg struct
// ---------------------------------------------------------------------------

/// Arguments for the `add` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddArgs {
    /// First operand.
    pub a: f64,
    /// Second operand.
    pub b: f64,
}

// ---------------------------------------------------------------------------
// Tool impl
// ---------------------------------------------------------------------------

/// Add two numbers and return the sum.
pub struct AddTool;

#[async_trait]
impl Tool for AddTool {
    fn name(&self) -> &str {
        "add"
    }

    fn description(&self) -> &str {
        "Add two numbers (a and b) and return the sum as a JSON number."
    }

    fn schema(&self) -> RootSchema {
        schema_for!(AddArgs)
    }

    async fn execute(&self, args: Value) -> Result<Value> {
        let a: AddArgs = serde_json::from_value(args)?;
        Ok(json!(a.a + a.b))
    }
}
