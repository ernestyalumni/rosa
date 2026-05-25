//! Math tools — the agent MUST use these instead of computing inline.
//!
//! Mirrors the tool set from the original Python rosa:
//!
//! | Tool                  | Operation                                      |
//! |-----------------------|------------------------------------------------|
//! | `degrees_to_radians`  | Convert degrees → radians                      |
//! | `radians_to_degrees`  | Convert radians → degrees                      |
//! | `sqrt`                | Square root                                    |
//! | `atan2`               | Four-quadrant arctangent (returns radians)     |
//! | `distance_2d`         | Euclidean distance between two 2-D points      |
//! | `heading_and_distance`| Bearing (rad) + distance from p1 to p2         |
//! | `subtract`            | a − b                                          |
//! | `multiply`            | a × b                                          |
//! | `divide`              | a ÷ b (errors on zero denominator)             |
//! | `sin`                 | sin of angle in radians                        |
//! | `cos`                 | cos of angle in radians                        |
//! | `tan`                 | tan of angle in radians                        |

use async_trait::async_trait;
use schemars::{schema_for, schema::RootSchema, JsonSchema};
use serde::Deserialize;
use serde_json::{json, Value};
use std::f64::consts::PI;

use rosa_core::error::{Result, RosaError};
use crate::tool::Tool;

// ---------------------------------------------------------------------------
// Shared arg structs
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct OneF64 {
    /// Input value.
    pub value: f64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TwoF64 {
    /// First operand.
    pub a: f64,
    /// Second operand.
    pub b: f64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TwoPoints {
    /// X-coordinate of the first point.
    pub x1: f64,
    /// Y-coordinate of the first point.
    pub y1: f64,
    /// X-coordinate of the second point.
    pub x2: f64,
    /// Y-coordinate of the second point.
    pub y2: f64,
}

// ---------------------------------------------------------------------------
// degrees_to_radians
// ---------------------------------------------------------------------------

pub struct DegreesToRadians;

#[async_trait]
impl Tool for DegreesToRadians {
    fn name(&self) -> &str { "degrees_to_radians" }
    fn description(&self) -> &str { "Convert an angle from degrees to radians." }
    fn schema(&self) -> RootSchema { schema_for!(OneF64) }
    async fn execute(&self, args: Value) -> Result<Value> {
        let a: OneF64 = serde_json::from_value(args)?;
        Ok(json!({ "radians": a.value * PI / 180.0 }))
    }
}

// ---------------------------------------------------------------------------
// radians_to_degrees
// ---------------------------------------------------------------------------

pub struct RadiansToDegrees;

#[async_trait]
impl Tool for RadiansToDegrees {
    fn name(&self) -> &str { "radians_to_degrees" }
    fn description(&self) -> &str { "Convert an angle from radians to degrees." }
    fn schema(&self) -> RootSchema { schema_for!(OneF64) }
    async fn execute(&self, args: Value) -> Result<Value> {
        let a: OneF64 = serde_json::from_value(args)?;
        Ok(json!({ "degrees": a.value * 180.0 / PI }))
    }
}

// ---------------------------------------------------------------------------
// sqrt
// ---------------------------------------------------------------------------

pub struct Sqrt;

#[async_trait]
impl Tool for Sqrt {
    fn name(&self) -> &str { "sqrt" }
    fn description(&self) -> &str { "Return the square root of a non-negative number." }
    fn schema(&self) -> RootSchema { schema_for!(OneF64) }
    async fn execute(&self, args: Value) -> Result<Value> {
        let a: OneF64 = serde_json::from_value(args)?;
        if a.value < 0.0 {
            return Err(RosaError::ToolExecution {
                name: "sqrt".into(),
                message: "cannot take square root of a negative number".into(),
            });
        }
        Ok(json!({ "result": a.value.sqrt() }))
    }
}

// ---------------------------------------------------------------------------
// atan2
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct Atan2Args {
    /// The y component (numerator).
    pub y: f64,
    /// The x component (denominator).
    pub x: f64,
}

pub struct Atan2;

#[async_trait]
impl Tool for Atan2 {
    fn name(&self) -> &str { "atan2" }
    fn description(&self) -> &str {
        "Return atan2(y, x) in radians — the four-quadrant arctangent. \
         Use this instead of computing angle differences inline."
    }
    fn schema(&self) -> RootSchema { schema_for!(Atan2Args) }
    async fn execute(&self, args: Value) -> Result<Value> {
        let a: Atan2Args = serde_json::from_value(args)?;
        Ok(json!({ "radians": a.y.atan2(a.x) }))
    }
}

// ---------------------------------------------------------------------------
// distance_2d
// ---------------------------------------------------------------------------

pub struct Distance2D;

#[async_trait]
impl Tool for Distance2D {
    fn name(&self) -> &str { "distance_2d" }
    fn description(&self) -> &str {
        "Return the Euclidean distance between two 2-D points (x1,y1) and (x2,y2)."
    }
    fn schema(&self) -> RootSchema { schema_for!(TwoPoints) }
    async fn execute(&self, args: Value) -> Result<Value> {
        let p: TwoPoints = serde_json::from_value(args)?;
        let dx = p.x2 - p.x1;
        let dy = p.y2 - p.y1;
        Ok(json!({ "distance": (dx * dx + dy * dy).sqrt() }))
    }
}

// ---------------------------------------------------------------------------
// heading_and_distance
// ---------------------------------------------------------------------------

pub struct HeadingAndDistance;

#[async_trait]
impl Tool for HeadingAndDistance {
    fn name(&self) -> &str { "heading_and_distance" }
    fn description(&self) -> &str {
        "Return the bearing (radians) and Euclidean distance from point (x1,y1) to (x2,y2). \
         Bearing is atan2(dy, dx) measured from the +x axis."
    }
    fn schema(&self) -> RootSchema { schema_for!(TwoPoints) }
    async fn execute(&self, args: Value) -> Result<Value> {
        let p: TwoPoints = serde_json::from_value(args)?;
        let dx = p.x2 - p.x1;
        let dy = p.y2 - p.y1;
        let distance = (dx * dx + dy * dy).sqrt();
        let heading = dy.atan2(dx);
        Ok(json!({ "heading_radians": heading, "distance": distance }))
    }
}

// ---------------------------------------------------------------------------
// subtract
// ---------------------------------------------------------------------------

pub struct Subtract;

#[async_trait]
impl Tool for Subtract {
    fn name(&self) -> &str { "subtract" }
    fn description(&self) -> &str { "Return a − b." }
    fn schema(&self) -> RootSchema { schema_for!(TwoF64) }
    async fn execute(&self, args: Value) -> Result<Value> {
        let a: TwoF64 = serde_json::from_value(args)?;
        Ok(json!({ "result": a.a - a.b }))
    }
}

// ---------------------------------------------------------------------------
// multiply
// ---------------------------------------------------------------------------

pub struct Multiply;

#[async_trait]
impl Tool for Multiply {
    fn name(&self) -> &str { "multiply" }
    fn description(&self) -> &str { "Return a × b." }
    fn schema(&self) -> RootSchema { schema_for!(TwoF64) }
    async fn execute(&self, args: Value) -> Result<Value> {
        let a: TwoF64 = serde_json::from_value(args)?;
        Ok(json!({ "result": a.a * a.b }))
    }
}

// ---------------------------------------------------------------------------
// divide
// ---------------------------------------------------------------------------

pub struct Divide;

#[async_trait]
impl Tool for Divide {
    fn name(&self) -> &str { "divide" }
    fn description(&self) -> &str { "Return a ÷ b. Errors if b is zero." }
    fn schema(&self) -> RootSchema { schema_for!(TwoF64) }
    async fn execute(&self, args: Value) -> Result<Value> {
        let a: TwoF64 = serde_json::from_value(args)?;
        if a.b == 0.0 {
            return Err(RosaError::ToolExecution {
                name: "divide".into(),
                message: "division by zero".into(),
            });
        }
        Ok(json!({ "result": a.a / a.b }))
    }
}

// ---------------------------------------------------------------------------
// sin / cos / tan
// ---------------------------------------------------------------------------

pub struct Sin;

#[async_trait]
impl Tool for Sin {
    fn name(&self) -> &str { "sin" }
    fn description(&self) -> &str { "Return sin of an angle in radians." }
    fn schema(&self) -> RootSchema { schema_for!(OneF64) }
    async fn execute(&self, args: Value) -> Result<Value> {
        let a: OneF64 = serde_json::from_value(args)?;
        Ok(json!({ "result": a.value.sin() }))
    }
}

pub struct Cos;

#[async_trait]
impl Tool for Cos {
    fn name(&self) -> &str { "cos" }
    fn description(&self) -> &str { "Return cos of an angle in radians." }
    fn schema(&self) -> RootSchema { schema_for!(OneF64) }
    async fn execute(&self, args: Value) -> Result<Value> {
        let a: OneF64 = serde_json::from_value(args)?;
        Ok(json!({ "result": a.value.cos() }))
    }
}

pub struct Tan;

#[async_trait]
impl Tool for Tan {
    fn name(&self) -> &str { "tan" }
    fn description(&self) -> &str { "Return tan of an angle in radians." }
    fn schema(&self) -> RootSchema { schema_for!(OneF64) }
    async fn execute(&self, args: Value) -> Result<Value> {
        let a: OneF64 = serde_json::from_value(args)?;
        Ok(json!({ "result": a.value.tan() }))
    }
}
