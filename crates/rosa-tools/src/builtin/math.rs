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
//! | `exponentiate`        | a ^ b                                          |
//! | `modulo`              | a % b                                          |
//! | `sin`                 | sin of angle in radians                        |
//! | `cos`                 | cos of angle in radians                        |
//! | `tan`                 | tan of angle in radians                        |
//! | `asin`                | arcsine → radians                              |
//! | `acos`                | arccosine → radians                            |
//! | `atan`                | arctangent (single-arg) → radians              |
//! | `sinh`                | hyperbolic sine                                |
//! | `cosh`                | hyperbolic cosine                              |
//! | `tanh`                | hyperbolic tangent                             |
//! | `add_all`             | sum of a list of numbers                       |
//! | `multiply_all`        | product of a list of numbers                   |
//! | `mean`                | arithmetic mean + stdev of a list              |
//! | `median`              | median of a list                               |
//! | `variance`            | variance of a list                             |
//! | `count_items`         | count elements in a JSON array                 |
//! | `count_words`         | count whitespace-separated words in a string   |
//! | `count_lines`         | count newline-delimited lines in a string      |

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

// ---------------------------------------------------------------------------
// exponentiate / modulo
// ---------------------------------------------------------------------------

pub struct Exponentiate;

#[async_trait]
impl Tool for Exponentiate {
    fn name(&self) -> &str { "exponentiate" }
    fn description(&self) -> &str { "Return a raised to the power b (a^b)." }
    fn schema(&self) -> RootSchema { schema_for!(TwoF64) }
    async fn execute(&self, args: Value) -> Result<Value> {
        let a: TwoF64 = serde_json::from_value(args)?;
        Ok(json!({ "result": a.a.powf(a.b) }))
    }
}

pub struct Modulo;

#[async_trait]
impl Tool for Modulo {
    fn name(&self) -> &str { "modulo" }
    fn description(&self) -> &str { "Return a % b (floating-point remainder). Errors if b is zero." }
    fn schema(&self) -> RootSchema { schema_for!(TwoF64) }
    async fn execute(&self, args: Value) -> Result<Value> {
        let a: TwoF64 = serde_json::from_value(args)?;
        if a.b == 0.0 {
            return Err(RosaError::ToolExecution {
                name: "modulo".into(),
                message: "modulo by zero".into(),
            });
        }
        Ok(json!({ "result": a.a % a.b }))
    }
}

// ---------------------------------------------------------------------------
// asin / acos / atan (single-argument)
// ---------------------------------------------------------------------------

pub struct Asin;

#[async_trait]
impl Tool for Asin {
    fn name(&self) -> &str { "asin" }
    fn description(&self) -> &str {
        "Return the arcsine of a value in [-1, 1], in radians. Errors outside that range."
    }
    fn schema(&self) -> RootSchema { schema_for!(OneF64) }
    async fn execute(&self, args: Value) -> Result<Value> {
        let a: OneF64 = serde_json::from_value(args)?;
        if !(-1.0..=1.0).contains(&a.value) {
            return Err(RosaError::ToolExecution {
                name: "asin".into(),
                message: format!("asin argument {} is outside [-1, 1]", a.value),
            });
        }
        Ok(json!({ "radians": a.value.asin() }))
    }
}

pub struct Acos;

#[async_trait]
impl Tool for Acos {
    fn name(&self) -> &str { "acos" }
    fn description(&self) -> &str {
        "Return the arccosine of a value in [-1, 1], in radians. Errors outside that range."
    }
    fn schema(&self) -> RootSchema { schema_for!(OneF64) }
    async fn execute(&self, args: Value) -> Result<Value> {
        let a: OneF64 = serde_json::from_value(args)?;
        if !(-1.0..=1.0).contains(&a.value) {
            return Err(RosaError::ToolExecution {
                name: "acos".into(),
                message: format!("acos argument {} is outside [-1, 1]", a.value),
            });
        }
        Ok(json!({ "radians": a.value.acos() }))
    }
}

pub struct Atan;

#[async_trait]
impl Tool for Atan {
    fn name(&self) -> &str { "atan" }
    fn description(&self) -> &str {
        "Return the single-argument arctangent of x, in radians. \
         For the angle between two points use atan2(y, x) instead."
    }
    fn schema(&self) -> RootSchema { schema_for!(OneF64) }
    async fn execute(&self, args: Value) -> Result<Value> {
        let a: OneF64 = serde_json::from_value(args)?;
        Ok(json!({ "radians": a.value.atan() }))
    }
}

// ---------------------------------------------------------------------------
// sinh / cosh / tanh
// ---------------------------------------------------------------------------

pub struct Sinh;

#[async_trait]
impl Tool for Sinh {
    fn name(&self) -> &str { "sinh" }
    fn description(&self) -> &str { "Return the hyperbolic sine of x." }
    fn schema(&self) -> RootSchema { schema_for!(OneF64) }
    async fn execute(&self, args: Value) -> Result<Value> {
        let a: OneF64 = serde_json::from_value(args)?;
        Ok(json!({ "result": a.value.sinh() }))
    }
}

pub struct Cosh;

#[async_trait]
impl Tool for Cosh {
    fn name(&self) -> &str { "cosh" }
    fn description(&self) -> &str { "Return the hyperbolic cosine of x." }
    fn schema(&self) -> RootSchema { schema_for!(OneF64) }
    async fn execute(&self, args: Value) -> Result<Value> {
        let a: OneF64 = serde_json::from_value(args)?;
        Ok(json!({ "result": a.value.cosh() }))
    }
}

pub struct Tanh;

#[async_trait]
impl Tool for Tanh {
    fn name(&self) -> &str { "tanh" }
    fn description(&self) -> &str { "Return the hyperbolic tangent of x." }
    fn schema(&self) -> RootSchema { schema_for!(OneF64) }
    async fn execute(&self, args: Value) -> Result<Value> {
        let a: OneF64 = serde_json::from_value(args)?;
        Ok(json!({ "result": a.value.tanh() }))
    }
}

// ---------------------------------------------------------------------------
// List aggregation: add_all, multiply_all
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct NumberList {
    /// List of numbers to aggregate.
    pub numbers: Vec<f64>,
}

pub struct AddAll;

#[async_trait]
impl Tool for AddAll {
    fn name(&self) -> &str { "add_all" }
    fn description(&self) -> &str { "Return the sum of a list of numbers." }
    fn schema(&self) -> RootSchema { schema_for!(NumberList) }
    async fn execute(&self, args: Value) -> Result<Value> {
        let a: NumberList = serde_json::from_value(args)?;
        Ok(json!({ "result": a.numbers.iter().sum::<f64>() }))
    }
}

pub struct MultiplyAll;

#[async_trait]
impl Tool for MultiplyAll {
    fn name(&self) -> &str { "multiply_all" }
    fn description(&self) -> &str { "Return the product of a list of numbers." }
    fn schema(&self) -> RootSchema { schema_for!(NumberList) }
    async fn execute(&self, args: Value) -> Result<Value> {
        let a: NumberList = serde_json::from_value(args)?;
        Ok(json!({ "result": a.numbers.iter().product::<f64>() }))
    }
}

// ---------------------------------------------------------------------------
// Statistics: mean, median, variance
// ---------------------------------------------------------------------------

pub struct Mean;

#[async_trait]
impl Tool for Mean {
    fn name(&self) -> &str { "mean" }
    fn description(&self) -> &str {
        "Return the arithmetic mean and standard deviation of a list of numbers."
    }
    fn schema(&self) -> RootSchema { schema_for!(NumberList) }
    async fn execute(&self, args: Value) -> Result<Value> {
        let a: NumberList = serde_json::from_value(args)?;
        if a.numbers.is_empty() {
            return Err(RosaError::ToolExecution {
                name: "mean".into(),
                message: "cannot compute mean of an empty list".into(),
            });
        }
        let n = a.numbers.len() as f64;
        let mean = a.numbers.iter().sum::<f64>() / n;
        let variance = a.numbers.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n;
        let stdev = variance.sqrt();
        Ok(json!({ "mean": mean, "stdev": stdev }))
    }
}

pub struct Median;

#[async_trait]
impl Tool for Median {
    fn name(&self) -> &str { "median" }
    fn description(&self) -> &str { "Return the median of a list of numbers." }
    fn schema(&self) -> RootSchema { schema_for!(NumberList) }
    async fn execute(&self, args: Value) -> Result<Value> {
        let a: NumberList = serde_json::from_value(args)?;
        if a.numbers.is_empty() {
            return Err(RosaError::ToolExecution {
                name: "median".into(),
                message: "cannot compute median of an empty list".into(),
            });
        }
        let mut sorted = a.numbers.clone();
        sorted.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
        let mid = sorted.len() / 2;
        let median = if sorted.len() % 2 == 0 {
            (sorted[mid - 1] + sorted[mid]) / 2.0
        } else {
            sorted[mid]
        };
        Ok(json!({ "median": median }))
    }
}

pub struct Variance;

#[async_trait]
impl Tool for Variance {
    fn name(&self) -> &str { "variance" }
    fn description(&self) -> &str {
        "Return the population variance of a list of numbers."
    }
    fn schema(&self) -> RootSchema { schema_for!(NumberList) }
    async fn execute(&self, args: Value) -> Result<Value> {
        let a: NumberList = serde_json::from_value(args)?;
        if a.numbers.len() < 2 {
            return Err(RosaError::ToolExecution {
                name: "variance".into(),
                message: "need at least 2 numbers to compute variance".into(),
            });
        }
        let n = a.numbers.len() as f64;
        let mean = a.numbers.iter().sum::<f64>() / n;
        let variance = a.numbers.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n;
        Ok(json!({ "variance": variance }))
    }
}

// ---------------------------------------------------------------------------
// Count helpers: count_items, count_words, count_lines
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CountItemsArgs {
    /// A JSON array whose elements will be counted.
    pub items: Vec<serde_json::Value>,
}

pub struct CountItems;

#[async_trait]
impl Tool for CountItems {
    fn name(&self) -> &str { "count_items" }
    fn description(&self) -> &str { "Return the number of elements in a JSON array." }
    fn schema(&self) -> RootSchema { schema_for!(CountItemsArgs) }
    async fn execute(&self, args: Value) -> Result<Value> {
        let a: CountItemsArgs = serde_json::from_value(args)?;
        Ok(json!({ "count": a.items.len() }))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TextArg {
    /// The text to analyse.
    pub text: String,
}

pub struct CountWords;

#[async_trait]
impl Tool for CountWords {
    fn name(&self) -> &str { "count_words" }
    fn description(&self) -> &str { "Return the number of whitespace-separated words in a string." }
    fn schema(&self) -> RootSchema { schema_for!(TextArg) }
    async fn execute(&self, args: Value) -> Result<Value> {
        let a: TextArg = serde_json::from_value(args)?;
        Ok(json!({ "count": a.text.split_whitespace().count() }))
    }
}

pub struct CountLines;

#[async_trait]
impl Tool for CountLines {
    fn name(&self) -> &str { "count_lines" }
    fn description(&self) -> &str { "Return the number of newline-delimited lines in a string." }
    fn schema(&self) -> RootSchema { schema_for!(TextArg) }
    async fn execute(&self, args: Value) -> Result<Value> {
        let a: TextArg = serde_json::from_value(args)?;
        Ok(json!({ "count": a.text.lines().count() }))
    }
}
