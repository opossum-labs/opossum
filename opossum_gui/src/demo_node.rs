use serde_derive::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub enum DemoNode {
    /// Node with single input.
    /// Displays the value of the input.
    Sink,
    /// Value node with a single output.
    Source,

    // /// Value node with a single output.
    // String(String),

    // /// Converts URI to Image
    // ShowImage(String),

    // /// Expression node with a single output.
    // /// It has number of inputs equal to number of variables in the expression.
    // ExprNode(ExprNode),
}