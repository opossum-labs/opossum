use opossum::{analyzers::AnalyzerType, optic_ref::OpticRef};
use serde_derive::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub enum EditorNode {
    /// Node representing an optical node.
    OpticRef(OpticRef),
    /// Node representing an analyzer.
    Analyzer(AnalyzerType),
}
